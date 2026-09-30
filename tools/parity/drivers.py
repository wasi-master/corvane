"""Drivers for the two apps under comparison.

GitHub Desktop (Electron) is driven over the Chrome DevTools Protocol: a
private instance runs with its own `--user-data-dir` (the user's real GHD and
its settings are never touched) and `--remote-debugging-port`. Input goes
through `Input.dispatch*Event`, which reaches the renderer without window
focus, and `Page.captureScreenshot` renders the page offscreen.

Corvane is driven through its `CORVANE_CONTROL` socket (crates/corvane/src/
parity_control.rs, `--features snapshots`), which injects the same input into
GPUI's event dispatch and renders frames offscreen.

Both expose the same interface: move / down / up / click / drag / scroll /
key / type / menu / popup / snap.
"""

from __future__ import annotations

import base64
import json
import os
import shlex
import signal
import socket
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

import websocket

IS_MAC = sys.platform == "darwin"

# PARITY_GHD_APP overrides; on Linux, a GitHub Desktop 3.6.6 build (`yarn
# build:prod` → dist/desktop-linux-x64/desktop) or a packaged github-desktop
GHD_APP = Path(
    os.environ.get("PARITY_GHD_APP")
    or ("/Applications/GitHub Desktop.app/Contents/MacOS/GitHub Desktop" if IS_MAC else "/usr/bin/github-desktop")
)
# Retina on the Macs the harness grew up on; X11 under Xvfb is 1x
DEFAULT_SCALE = 2.0 if IS_MAC else 1.0


def platform_keys(spec: str) -> str:
    """Scenario chords say `cmd` for GHD's CmdOrCtrl: Ctrl off macOS."""
    if IS_MAC or not spec:
        return spec
    return " ".join(
        "-".join("ctrl" if part in ("cmd", "meta") else part for part in chord.split("-"))
        if chord not in ("-",) else chord
        for chord in spec.split(" ")
    )

# GHD menu-event names (app/src/main-process/menu/menu-event.ts) → Corvane
# actions (crates/corvane-ui/src/actions.rs). GHD menu accelerators live in the
# main process and never see CDP key events, so shortcuts are replayed as the
# menu events they trigger.
MENU_ACTIONS = {
    "show-changes": "corvane::ShowChanges",
    "show-history": "corvane::ShowHistory",
    "choose-repository": "corvane::ShowRepositoryList",
    "show-branches": "corvane::ShowBranchesList",
    "show-worktrees": "corvane::ShowWorktreesList",
    "create-worktree": "corvane::NewWorktree",
    "go-to-commit-message": "corvane::GoToSummary",
    "show-stashed-changes": "corvane::ToggleStashedChanges",
    "hide-stashed-changes": "corvane::ToggleStashedChanges",
    "toggle-changes-filter": "corvane::ToggleChangesFilter",
    "show-preferences": "corvane::OpenSettings",
    "show-about": "corvane::About",
    "add-local-repository": "corvane::AddLocalRepository",
    "create-repository": "corvane::NewRepository",
    "clone-repository": "corvane::CloneRepository",
    "create-branch": "corvane::NewBranch",
    "rename-branch": "corvane::RenameBranch",
    "delete-branch": "corvane::DeleteBranch",
    "discard-all-changes": "corvane::DiscardAllChanges",
    "stash-all-changes": "corvane::StashAllChanges",
    "update-branch-with-contribution-target-branch": "corvane::UpdateFromDefaultBranch",
    "compare-to-branch": "corvane::CompareToBranch",
    "merge-branch": "corvane::MergeIntoCurrentBranch",
    "squash-and-merge-branch": "corvane::SquashAndMergeIntoCurrentBranch",
    "rebase-branch": "corvane::RebaseCurrentBranch",
    "show-repository-settings": "corvane::RepositorySettings",
    "remove-repository": "corvane::RemoveRepository",
    "push": "corvane::Push",
    "pull": "corvane::Pull",
    "fetch": "corvane::Fetch",
    "preview-pull-request": "corvane::PreviewPullRequest",
    "open-pull-request": "corvane::CreatePullRequest",
    "find-text": "corvane::Find",
    "select-all": "corvane::SelectAll",
    "increase-active-resizable-width": "corvane::ExpandActiveResizable",
    "decrease-active-resizable-width": "corvane::ContractActiveResizable",
}

# GPUI key name → (DOM key, DOM code, Windows virtual key code, mac editing command)
_KEYS = {
    "enter": ("Enter", "Enter", 13, "insertNewline"),
    "escape": ("Escape", "Escape", 27, None),
    "tab": ("Tab", "Tab", 9, None),
    "backspace": ("Backspace", "Backspace", 8, "deleteBackward"),
    "delete": ("Delete", "Delete", 46, "deleteForward"),
    "space": (" ", "Space", 32, None),
    "up": ("ArrowUp", "ArrowUp", 38, "moveUp"),
    "down": ("ArrowDown", "ArrowDown", 40, "moveDown"),
    "left": ("ArrowLeft", "ArrowLeft", 37, "moveLeft"),
    "right": ("ArrowRight", "ArrowRight", 39, "moveRight"),
    "home": ("Home", "Home", 36, "moveToBeginningOfLine"),
    "end": ("End", "End", 35, "moveToEndOfLine"),
    "pageup": ("PageUp", "PageUp", 33, "scrollPageUp"),
    "pagedown": ("PageDown", "PageDown", 34, "scrollPageDown"),
    "f2": ("F2", "F2", 113, None),
}
_CMD_COMMANDS = {"a": "selectAll", "c": "copy", "v": "paste", "x": "cut", "z": "undo"}


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def parse_mods(spec: str) -> dict:
    spec = platform_keys(spec)
    parts = set(spec.replace("+", "-").split("-")) if spec else set()
    return {
        "cmd": bool(parts & {"cmd", "meta"}),
        "shift": "shift" in parts,
        "alt": bool(parts & {"alt", "option"}),
        "ctrl": bool(parts & {"ctrl", "control"}),
    }


def _cdp_mods(m: dict) -> int:
    return (1 if m["alt"] else 0) | (2 if m["ctrl"] else 0) | (4 if m["cmd"] else 0) | (8 if m["shift"] else 0)


class Ghd:
    """A private GitHub Desktop instance under CDP control."""

    name = "ghd"

    def __init__(self, profile: Path, log: Path, env: dict | None = None):
        self.profile = profile
        self.port = free_port()
        self.log = log
        # scenario `ghd_env`, e.g. GITHUB_DESKTOP_PREVIEW_FEATURES=1 for the
        # `test-*` popups (enableTestMenuItems)
        self.env = env or {}
        self.proc: subprocess.Popen | None = None
        self.ws = None
        self._id = 0
        self.scale = DEFAULT_SCALE

    # -- process -----------------------------------------------------------
    def start(self, timeout: float = 30):
        self.profile.mkdir(parents=True, exist_ok=True)
        with open(self.log, "ab") as log:
            self.proc = subprocess.Popen(
                [
                    str(GHD_APP),
                    f"--remote-debugging-port={self.port}",
                    f"--user-data-dir={self.profile}",
                    # captures in sRGB, like Corvane's render_to_image; without it
                    # Chromium converts to the display profile (#1d2125 → #16191c)
                    "--force-color-profile=srgb",
                    # Chromium refuses to run as root with its sandbox
                    *(["--no-sandbox"] if not IS_MAC and os.geteuid() == 0 else []),
                    # extra switches, e.g. `--proxy-server=127.0.0.1:9` to keep
                    # GHD offline where its network would fail differently
                    # from Corvane's (an intercepting proxy Chromium does not
                    # trust opens an "Untrusted server" dialog)
                    *shlex.split(os.environ.get("PARITY_GHD_ARGS", "")),
                ],
                stdout=log,
                stderr=log,
                start_new_session=True,
                env={**os.environ, **{k: str(v) for k, v in self.env.items()}},
            )
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{self.port}/json", timeout=1))
                pages = [t for t in tabs if t.get("type") == "page" and "index.html" in t.get("url", "")]
                if pages:
                    self.ws = websocket.create_connection(pages[0]["webSocketDebuggerUrl"], suppress_origin=True, timeout=60)
                    break
            except OSError:
                pass
            time.sleep(0.25)
        else:
            raise RuntimeError("GitHub Desktop did not expose a DevTools page")
        self.wait_for("document.readyState === 'complete' && !!document.querySelector('#desktop-app-container, #desktop-app')", timeout)

    def stop(self):
        if self.ws:
            try:
                self.ws.close()
            except Exception:
                pass
            self.ws = None
        if self.proc and self.proc.poll() is None:
            # Chromium shuts down cleanly on SIGTERM (IndexedDB / localStorage flushed)
            os.killpg(self.proc.pid, signal.SIGTERM)
            try:
                self.proc.wait(8)
            except subprocess.TimeoutExpired:
                os.killpg(self.proc.pid, signal.SIGKILL)
        self.proc = None

    # -- protocol ----------------------------------------------------------
    def call(self, method: str, **params):
        self._id += 1
        self.ws.send(json.dumps({"id": self._id, "method": method, "params": params}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == self._id:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error']}")
                return msg.get("result", {})

    def eval(self, expression: str):
        r = self.call("Runtime.evaluate", expression=expression, returnByValue=True, awaitPromise=True)
        if "exceptionDetails" in r:
            raise RuntimeError(f"eval failed: {r['exceptionDetails'].get('exception', {}).get('description', r['exceptionDetails'])}")
        return r.get("result", {}).get("value")

    def wait_for(self, expression: str, timeout: float = 15, interval: float = 0.15) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                if self.eval(f"!!({expression})"):
                    return True
            except RuntimeError:
                pass
            time.sleep(interval)
        return False

    def emit(self, channel: str, payload) -> None:
        """Deliver an IPC message to the renderer as if the main process sent it."""
        self.eval(f"require('electron').ipcRenderer.emit({json.dumps(channel)}, {{}}, {json.dumps(payload)})")

    # -- setup -------------------------------------------------------------
    def configure(self, width: int, height: int, scale: float, local_storage: dict, freeze: bool):
        """Viewport, focus emulation and localStorage (then reload so GHD reads it)."""
        self.scale = scale
        if local_storage:
            items = ";".join(f"localStorage.setItem({json.dumps(k)}, {json.dumps(str(v))})" for k, v in local_storage.items())
            self.eval(items)
            self.call("Page.reload", ignoreCache=False)
            time.sleep(0.5)
            self.wait_for("document.readyState === 'complete' && !!document.querySelector('#desktop-app-container, #desktop-app')", 30)
        self.resize(width, height)
        self.hook_context_menus()
        self.call("Emulation.setFocusEmulationEnabled", enabled=True)
        self.emit("focus", None)
        if freeze:
            # transitions finish instantly so a capture never lands mid-animation
            self.eval(
                "(()=>{const s=document.createElement('style');s.id='parity-freeze';"
                "s.textContent='*,*::before,*::after{transition-duration:0s!important;transition-delay:0s!important;"
                "animation-duration:0s!important;animation-delay:0s!important}';document.head.appendChild(s)})()"
            )

    def resize(self, width: int, height: int):
        """Resize the real window (hidden title bar: window size == content size).

        Electron has no `Browser.setWindowBounds`, but `window.resizeTo` on the
        main frame resizes the BrowserWindow. When the screen is too small for
        it, the viewport is emulated at the requested size instead."""
        # the frame around the page (Electron's menu bar on Linux, nothing
        # with macOS's hidden title bar)
        chrome = self.eval("[outerWidth - innerWidth, outerHeight - innerHeight]") or [0, 0]
        self.eval(f"window.resizeTo({width + chrome[0]}, {height + chrome[1]})")
        deadline = time.time() + 3
        while time.time() < deadline:
            if self.eval(f"innerWidth === {width} && innerHeight === {height}"):
                self.call("Emulation.clearDeviceMetricsOverride")
                return
            time.sleep(0.1)
        self.call("Emulation.setDeviceMetricsOverride", width=width, height=height, deviceScaleFactor=self.scale, mobile=False)

    def hook_context_menus(self):
        """Record `show-contextual-menu` IPC calls instead of letting the main
        process pop a native menu; the promise stays pending until
        `pick_menu` / `dismiss_menu` (GHD then runs the chosen action)."""
        self.eval(
            "(()=>{if(window.__parityMenuHook)return;window.__parityMenuHook=true;"
            "const {ipcRenderer}=require('electron');const orig=ipcRenderer.invoke.bind(ipcRenderer);"
            "window.__parityMenuPop=false;"
            "ipcRenderer.invoke=(ch,...a)=>{if(ch==='show-contextual-menu'){window.__parityMenu=a[0];"
            "if(window.__parityMenuPop)return orig(ch,...a);"
            "return new Promise(r=>{window.__parityMenuResolve=r;});}return orig(ch,...a);};})()"
        )

    def set_menu_pop(self, pop: bool):
        """Let GHD pop its real menus (visual passes; something on screen has
        to dismiss them) instead of only recording them."""
        self.eval(f"window.__parityMenuPop={'true' if pop else 'false'}")

    def menu_items(self) -> list[str]:
        items = self.eval("window.__parityMenu || null") or []

        def walk(items, depth, out):
            for it in items:
                pad = "  " * depth
                if it.get("type") == "separator":
                    out.append(pad + "-")
                    continue
                line = pad + (it.get("label") or "")
                if it.get("enabled") is False:
                    line += " [disabled]"
                if it.get("checked"):
                    line += " [x]"
                out.append(line)
                if it.get("submenu"):
                    walk(it["submenu"], depth + 1, out)
        out: list[str] = []
        walk(items, 0, out)
        return out

    def pick_menu(self, label: str):
        found = self.eval(
            "(()=>{const find=(items,path)=>{for(let i=0;i<items.length;i++){const it=items[i];"
            # scenarios name macOS labels; GHD's Linux ones are sentence case
            "const L=%s;if((it.label===L||(it.label||'').toLowerCase()===L.toLowerCase())&&it.enabled!==false)return path.concat(i);"
            "if(it.submenu){const r=find(it.submenu,path.concat(i));if(r)return r;}}return null;};"
            "const p=find(window.__parityMenu||[],[]);if(p&&window.__parityMenuResolve){window.__parityMenuResolve(p);"
            "window.__parityMenu=null;}return p;})()" % json.dumps(label)
        )
        if not found:
            raise LookupError(f"GHD menu has no enabled item {label!r}")

    def dismiss_menu(self):
        self.eval("(()=>{if(window.__parityMenuResolve)window.__parityMenuResolve(null);window.__parityMenu=null;})()")

    def add_repository(self, path: Path) -> bool:
        """`cli-action open-repository` → Add Repository dialog → submit."""
        self.emit("cli-action", {"kind": "open-repository", "path": str(path)})
        if not self.wait_for("document.querySelector('dialog button[type=submit]')", 10):
            return False
        time.sleep(0.3)
        self.eval("document.querySelector('dialog button[type=submit]').click()")
        return self.wait_for(f"!document.querySelector('dialog') && document.body.innerText.includes({json.dumps(path.name)})", 15)

    # -- geometry ----------------------------------------------------------
    def resolve(self, target) -> tuple[float, float]:
        """`[x, y]`, `{css: sel}` or `{text: label}` (+ `offset`) → window point."""
        if isinstance(target, (list, tuple)):
            return float(target[0]), float(target[1])
        if "css" in target:
            js = f"document.querySelector({json.dumps(target['css'])})"
        elif "contains" in target:
            js = (
                "(()=>{const t=%s;const root=document.querySelector(%s)||document.body;"
                "const all=[...root.querySelectorAll('*')].filter(e=>{"
                "const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&e.textContent.includes(t)});"
                "return all.find(e=>![...e.children].some(c=>c.textContent.includes(t)))||null})()"
            ) % (json.dumps(target["contains"]), json.dumps(target.get("within", "body")))
        elif "text" in target:
            # innermost visible element whose own text matches (optionally inside `within`)
            js = (
                "(()=>{const t=%s;const root=document.querySelector(%s)||document.body;"
                "const all=[...root.querySelectorAll('*')].filter(e=>{"
                "const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&e.textContent.trim()===t});"
                "return all.find(e=>![...e.children].some(c=>c.textContent.trim()===t))||null})()"
            ) % (json.dumps(target["text"]), json.dumps(target.get("within", "body")))
        else:
            raise ValueError(f"bad target {target}")
        rect = self.eval(f"(()=>{{const e={js};if(!e)return null;const r=e.getBoundingClientRect();return [r.x,r.y,r.width,r.height]}})()")
        if not rect:
            raise LookupError(f"GHD element not found: {target}")
        ox, oy = target.get("offset", [0, 0])
        ax, ay = target.get("anchor", [0.5, 0.5])
        return rect[0] + rect[2] * ax + ox, rect[1] + rect[3] * ay + oy

    def describe(self, x: float, y: float) -> str:
        """CSS path of the element at a point (for diff regions)."""
        return self.eval(
            "(()=>{let e=document.elementFromPoint(%f,%f);const out=[];while(e&&e!==document.body&&out.length<5){"
            "let s=e.tagName.toLowerCase();if(e.id)s+='#'+e.id;const c=[...e.classList].slice(0,3).join('.');if(c)s+='.'+c;"
            "out.unshift(s);e=e.parentElement}return out.join(' > ')})()" % (x, y)
        ) or ""

    def dump(self, root: str = "body") -> list:
        """Visible elements under `root` with their box and the styles that
        decide how they look (a spec to implement against)."""
        js = r"""
(() => {
  const props = ['color','backgroundColor','fontSize','fontWeight','lineHeight','fontFamily',
    'paddingTop','paddingRight','paddingBottom','paddingLeft','marginTop','marginRight','marginBottom','marginLeft',
    'borderTopWidth','borderTopColor','borderRightWidth','borderBottomWidth','borderBottomColor','borderLeftWidth',
    'borderRadius','boxShadow','opacity','zoom','textAlign','letterSpacing'];
  const root = document.querySelector(%s) || document.body;
  const out = [];
  const walk = (e, depth) => {
    const r = e.getBoundingClientRect();
    const cs = getComputedStyle(e);
    if (r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none') {
      const own = [...e.childNodes].filter(n => n.nodeType === 3).map(n => n.textContent.trim()).join(' ').trim();
      const st = {};
      for (const p of props) {
        const v = cs[p];
        if (v && !['0px','none','normal','auto','rgba(0, 0, 0, 0)','1','start'].includes(v)) st[p] = v;
      }
      out.push({depth, tag: e.tagName.toLowerCase(), id: e.id || undefined,
        cls: [...e.classList].join(' ') || undefined, text: own.slice(0, 80) || undefined,
        rect: [r.x, r.y, r.width, r.height].map(v => Math.round(v * 100) / 100), style: st});
    }
    for (const c of e.children) walk(c, depth + 1);
  };
  walk(root, 0);
  return out;
})()""" % json.dumps(root)
        return self.eval(js) or []

    # -- input -------------------------------------------------------------
    def _mouse(self, kind, x, y, button="left", clicks=1, mods="", buttons=0):
        self.call(
            "Input.dispatchMouseEvent",
            type=kind,
            x=x,
            y=y,
            button=button if kind != "mouseMoved" else ("left" if buttons else "none"),
            buttons=buttons,
            clickCount=clicks,
            modifiers=_cdp_mods(parse_mods(mods)),
        )

    def move(self, x, y, mods="", pressed=False):
        self._mouse("mouseMoved", x, y, mods=mods, buttons=1 if pressed else 0)

    def down(self, x, y, button="left", clicks=1, mods=""):
        self._mouse("mouseMoved", x, y, mods=mods)
        for n in range(1, clicks + 1):
            self._mouse("mousePressed", x, y, button, n, mods, buttons=1)
            if n < clicks:
                self._mouse("mouseReleased", x, y, button, n, mods)

    def up(self, x, y, button="left", clicks=1, mods=""):
        self._mouse("mouseReleased", x, y, button, clicks, mods)

    def click(self, x, y, button="left", clicks=1, mods=""):
        self.down(x, y, button, clicks, mods)
        self.up(x, y, button, clicks, mods)

    def drag(self, x, y, x2, y2, steps=10):
        self._mouse("mouseMoved", x, y)
        self._mouse("mousePressed", x, y, buttons=1)
        for i in range(1, steps + 1):
            t = i / steps
            self._mouse("mouseMoved", x + (x2 - x) * t, y + (y2 - y) * t, buttons=1)
            time.sleep(0.01)
        self._mouse("mouseReleased", x2, y2)

    def scroll(self, x, y, dx, dy):
        self.call("Input.dispatchMouseEvent", type="mouseWheel", x=x, y=y, deltaX=dx, deltaY=dy)

    def key(self, keys: str):
        for chord in platform_keys(keys).split():
            parts = chord.split("-")
            base = parts[-1] if parts[-1] != "" else "-"
            m = parse_mods("-".join(parts[:-1]))
            params = {"modifiers": _cdp_mods(m)}
            if base in _KEYS:
                k, code, vk, command = _KEYS[base]
                params.update(key=k, code=code, windowsVirtualKeyCode=vk)
                # editing commands are AppKit's; Chromium elsewhere acts on the key
                if command and not m["cmd"] and IS_MAC:
                    params["commands"] = [command]
                if base == "enter":
                    params["text"] = "\r"
                if base == "space":
                    params["text"] = " "
            else:
                ch = base.upper() if m["shift"] and len(base) == 1 else base
                vk = ord(base.upper()) if len(base) == 1 else 0
                code = f"Key{base.upper()}" if base.isalpha() and len(base) == 1 else (f"Digit{base}" if base.isdigit() else "")
                params.update(key=ch, code=code, windowsVirtualKeyCode=vk)
                if m["cmd"] and base in _CMD_COMMANDS and IS_MAC:
                    params["commands"] = [_CMD_COMMANDS[base] if not (base == "z" and m["shift"]) else "redo"]
                elif not (m["cmd"] or m["ctrl"]):
                    params["text"] = ch
            self.call("Input.dispatchKeyEvent", type="keyDown" if "text" not in params else "keyDown", **params)
            up = {k: v for k, v in params.items() if k not in ("text", "commands")}
            self.call("Input.dispatchKeyEvent", type="keyUp", **up)

    def type(self, text: str):
        self.call("Input.insertText", text=text)

    def menu(self, event: str):
        self.emit("menu-event", event)

    def popup(self, name: str):
        """GHD's own UI test hooks are menu events (`test-*`)."""
        self.emit("menu-event", name)

    def snap(self, path: Path):
        r = self.call("Page.captureScreenshot", format="png", fromSurface=True)
        path.write_bytes(base64.b64decode(r["data"]))


class Corvane:
    """A Corvane instance with an isolated data dir under `CORVANE_CONTROL`."""

    name = "corvane"

    def __init__(self, binary: Path, data_dir: Path, log: Path, theme: str):
        self.binary = binary
        self.data_dir = data_dir
        self.log = log
        self.theme = theme
        self.port = free_port()
        self.proc: subprocess.Popen | None = None
        self.sock = None
        self.file = None
        self.scale = DEFAULT_SCALE

    def start(self, timeout: float = 20, extra_env: dict | None = None):
        self.data_dir.mkdir(parents=True, exist_ok=True)
        env = {k: v for k, v in os.environ.items() if not k.startswith("CORVANE_")}
        env.update(
            CORVANE_DATA_DIR=str(self.data_dir),
            CORVANE_CONTROL=str(self.port),
            CORVANE_THEME=self.theme,
            CORVANE_LOG=env.get("PARITY_CORVANE_LOG", "info"),
            # every flag at its GHD value, so the diff measures true parity
            # (.docs/flags.md); PARITY_CORVANE_FLAGS overrides
            CORVANE_FLAGS=env.get("PARITY_CORVANE_FLAGS", "preset=github-desktop"),
        )
        env.update(extra_env or {})
        with open(self.log, "ab") as log:
            self.proc = subprocess.Popen([str(self.binary)], env=env, stdout=log, stderr=log, start_new_session=True)
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                self.sock = socket.create_connection(("127.0.0.1", self.port), timeout=60)
                self.file = self.sock.makefile("rw")
                info = self.cmd("ping")
                self.scale = info.get("scale", DEFAULT_SCALE)
                # CORVANE_THEME only overrides the look; store the setting too,
                # as GHD's fixture does (Settings › Appearance shows it)
                self.hook("theme", self.theme)
                return
            except OSError:
                if self.proc.poll() is not None:
                    raise RuntimeError(f"corvane exited early (see {self.log})")
                time.sleep(0.2)
        raise RuntimeError("corvane control socket did not come up")

    def stop(self):
        if self.file:
            try:
                self.cmd("quit")
            except Exception:
                pass
        if self.sock:
            self.sock.close()
        self.sock = self.file = None
        if self.proc:
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        self.proc = None

    def cmd(self, cmd: str, **args) -> dict:
        self.file.write(json.dumps({"cmd": cmd, **args}) + "\n")
        self.file.flush()
        line = self.file.readline()
        if not line:
            raise RuntimeError("corvane closed the control socket")
        reply = json.loads(line)
        if not reply.get("ok"):
            raise RuntimeError(f"corvane {cmd}: {reply.get('error')}")
        return reply

    def hook(self, name: str, arg: str = ""):
        return self.cmd("hook", name=name, arg=arg)

    def resize(self, w, h):
        self.cmd("resize", w=w, h=h)
        deadline = time.time() + 3
        while time.time() < deadline:
            info = self.cmd("ping")
            if int(info["w"]) == w and int(info["h"]) == h:
                return info
            time.sleep(0.1)
        return self.cmd("ping")

    def move(self, x, y, mods="", pressed=False):
        self.cmd("move", x=x, y=y, mods=platform_keys(mods), pressed=pressed)

    def down(self, x, y, button="left", clicks=1, mods=""):
        self.cmd("down", x=x, y=y, button=button, clicks=clicks, mods=platform_keys(mods))

    def up(self, x, y, button="left", clicks=1, mods=""):
        self.cmd("up", x=x, y=y, button=button, clicks=clicks, mods=platform_keys(mods))

    def click(self, x, y, button="left", clicks=1, mods=""):
        for n in range(1, clicks + 1):
            self.cmd("click", x=x, y=y, button=button, clicks=n, mods=platform_keys(mods))

    def drag(self, x, y, x2, y2, steps=10):
        self.cmd("drag", x=x, y=y, x2=x2, y2=y2, steps=steps)

    def scroll(self, x, y, dx, dy):
        self.cmd("scroll", x=x, y=y, dx=dx, dy=dy)

    def key(self, keys: str):
        self.cmd("key", keys=platform_keys(keys))

    def type(self, text: str):
        self.cmd("type", text=text)

    def menu(self, event: str):
        action = MENU_ACTIONS.get(event)
        if not action:
            raise LookupError(f"no Corvane action mapped for GHD menu event {event!r} (drivers.MENU_ACTIONS)")
        self.cmd("action", name=action)

    def popup(self, name: str):
        self.hook("popup", name)

    def menu_items(self) -> list[str]:
        return self.cmd("menu").get("items", [])

    def pick_menu(self, label: str):
        self.cmd("menu-pick", label=label)

    def dismiss_menu(self):
        pass

    def snap(self, path: Path):
        self.cmd("snap", path=str(path))
