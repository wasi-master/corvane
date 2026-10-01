# GitHub Desktop parity harness

Drives GitHub Desktop 3.6.6 and Corvane side by side with identical input,
captures both after every `snap` step, diffs the captures and fails any step
whose difference exceeds its threshold. Use it to find and burn down every
visual / behavioural deviation from GHD, including states that take several
clicks to reach, hover and pressed states.

```bash
pip3 install -r tools/parity/requirements.txt   # pillow numpy pyyaml websocket-client (scipy optional)
cargo build -p corvane --features snapshots
python3 tools/parity/parity.py                  # all scenarios, dark + light
python3 tools/parity/parity.py 'branch-*' preferences --themes dark
python3 tools/parity/parity.py --list
python3 tools/parity/parity.py no-repositories --ghd-only   # GHD alone: captures + `dump` specs
open target/parity/latest/index.html
```

Exit status is 1 when any snap is over its threshold or a scenario errors, so
the run can gate a change.

## How it works

| | GitHub Desktop | Corvane |
|---|---|---|
| Instance | private copy: `--user-data-dir=<scratch>` (your GHD and its settings are untouched) | `CORVANE_DATA_DIR=<scratch>` (your store is untouched) |
| Control | Chrome DevTools Protocol (`--remote-debugging-port`) | `CORVANE_CONTROL=<port>` socket (`crates/corvane/src/parity_control.rs`, `--features snapshots`) |
| Flags | - | `CORVANE_FLAGS=preset=github-desktop`: every deviation at its GHD value (`.docs/flags.md`); `PARITY_CORVANE_FLAGS=<spec>` overrides |
| Input | `Input.dispatchMouseEvent` / `dispatchKeyEvent` / `insertText` into the renderer | synthetic `PlatformInput` into GPUI's window dispatch |
| Menus | `menu-event` IPC emitted in the renderer (accelerators live in the main process) | the mapped action (`drivers.MENU_ACTIONS`) |
| Capture | `Page.captureScreenshot` | `Window::draw` + `render_to_image` |
| Size | real window resized to 1367×814 (`window.resizeTo`; emulated viewport only if the screen is too small) | `Window::resize` 1367×814 |

Neither app needs focus or Screen Recording permission; both windows do
appear on screen while a scenario runs. Every scenario starts from fresh
instances: fresh profiles, a freshly built fixture repository per app
(`fixture.py`: fixed author and dates, so both copies have identical SHAs and
"x days ago" texts), default panel widths (250pt sidebar, 250pt commit
summary; GHD's `localStorage` width keys are left unset), the theme under test,
and GHD's CSS transitions disabled (`--no-freeze` keeps them) so captures never
land mid-animation.

Targets resolve in GHD's DOM (`{css: …}` / `{text: …, within: …}`) or are
literal window points; **both apps receive the same point**, so a Corvane
element that is not where GHD has it misses the click and the next snap shows
it.

## The diff

`imgdiff.py` counts a pixel as different only when no pixel within `radius`
points (default 1) of it in the other capture is close enough: within
`tolerance` (max channel delta, default 6) on flat pixels - fills, borders,
hover and pressed backgrounds - and within `edge_tolerance` (default 40) next
to a high-contrast edge, where Chromium and Core Text anti-alias glyphs
differently. The private GHD runs with `--force-color-profile=srgb` so both
captures are sRGB (otherwise Chromium bakes the display profile in and every
colour reads a few levels off). A 1pt offset, a hover colour that is Δ10 off, or
a missing element fails. The pass/fail figure is the share of differing pixels;
the report also gives the share of 4pt blocks containing any difference,
which tracks layout mismatches better (a misplaced text column changes few
pixels but many blocks). Differences are grouped into regions (4pt blocks); for
each region the report gives

- a hint: *"Corvane content is offset 3pt right of GHD"* (best-aligning shift
  search) or *"colour/content: GHD #1f2328 vs Corvane #24292e"* (median colours
  of the differing pixels),
- the GHD DOM path at the region centre (which GHD component to read),
- a zoomed GHD · Corvane · diff crop.

The report (`target/parity/<run>/index.html`, `latest` symlink) shows every
snap as diff overlay, side by side, swipe and blink; `results.json` holds the
same data for scripts.

## Scenario format

`scenarios/*.yaml`, one or more documents per file (`---`):

```yaml
name: branch-foldout
description: What this covers
setup: repo            # repo (fixture added + selected) | empty (no repositories) | welcome (first launch)
ghd_env: {GITHUB_DESKTOP_PREVIEW_FEATURES: 1}   # optional: extra env for GHD (test-* popups need this; it also turns on beta features)
threshold: 1.0         # optional per-scenario defaults: threshold, tolerance, radius, settle, width, height
steps:
  - hover: [365, 56]                     # window points
  - press: {css: ".toolbar-dropdown button"}   # mouse down and hold (pressed / :active state)
  - release: [365, 56]
  - click: {text: "Integrations", within: "dialog"}   # also clicks: 2, mods: cmd-shift
  - dblclick: [120, 214]
  - rclick: [120, 214]                   # right-click (then `context_menu` to compare the menus)
  - drag: {from: [249, 400], to: [320, 400]}
  - scroll: {at: [124, 300], dy: 200}
  - key: cmd-a backspace                 # GPUI keystroke syntax, space separated
  - type: "feature/login"
  - menu: show-preferences               # GHD menu-event name
  - popup: {ghd: test-release-notes-popup, corvane: release-notes}   # GHD test hook / CORVANE_POPUP
  - accounts: dotcom                     # fake signed-in accounts + repository lists (accounts.py: dotcom | enterprise | two)
  - resize: [1100, 700]
  - wait: 500                            # alone: sleep; on a step: settle time after it (default 350ms)
  - ghd: {eval: "…"}                     # app-specific step (either side can be `{skip: true}`)
    corvane: {hook: {name: popup, arg: about}}   # hooks: complete-welcome, add-repo, theme, popup, refresh (GHD's `focus` IPC)
  - fixture: move                        # rename both fixture repositories away (missing repository)
  - context_menu: add                    # compare both apps' last native menu (items, separators, disabled/checked)
  - context_menu_pick: "Clone Repository…"   # choose an item in both (GHD: resolves its IPC; Corvane: menu-pick)
  - dump: open                           # GHD DOM boxes + computed styles as JSON ({name, root: css})
  - snap: open                           # or {name, threshold, tolerance, edge_tolerance, radius, mask: [[x,y,w,h]…], region: [x,y,w,h], note}
```

Masks are for deliberate differences only (product name, version numbers) and
each should carry a `note` pointing at `.docs/deviations.md`.

## Signed-in states

Neither app can sign in during a run, so `accounts: <fixture>` injects the
accounts and repository lists of `accounts.py` into both: GHD's `AppStore`
(found through the root component's React fiber) gets them as `accounts` and
`ApiRepositoriesStore` state, Corvane gets them through the `fake-accounts`
control hook. No API call is made (every list is already loaded; the tokens
are fake), so only views that render from that state can be compared: the
signed-in blank slate, Welcome › Configure Git after "Skip this step",
account pickers.

## Linux

GitHub Desktop 3.6.6 has no official Linux build; build it from source
(`desktop/desktop` at `release-3.6.6`: `yarn install && yarn build:prod`,
Node from `.nvmrc`) and point the harness at it. Both apps run on an X
server (Xvfb works: `Xvfb :99 -screen 0 1920x1080x24`, a window manager such
as openbox, `DISPLAY=:99`) and a session bus:

```bash
export PARITY_GHD_APP=$HOME/desktop/dist/desktop-linux-x64/desktop
cargo build -p corvane --features snapshots
dbus-run-session -- python3 tools/parity/parity.py main-window
```

- Scenarios keep macOS chords: `cmd` is Ctrl off macOS, and AppKit editing
  commands are only sent on macOS. Menu items are matched case-insensitively
  (GHD's Linux labels are sentence case).
- Captures are the page: CDP leaves Electron's menu bar out, and Corvane's
  control socket works in page coordinates below its own menu bar
  (`PAGE_TOP` in `parity_control.rs`). Scale is 1.
- Scenarios are written for GHD's macOS page, which starts with a 32 pt
  title bar. Off macOS the page is the content below it (the scenario
  height minus 32) and fixed points, masks and regions move up by 32, so
  top- and bottom-anchored elements both line up (`TITLE_BAR` in
  `drivers.py`).
- `PARITY_OFFLINE=1` takes both apps offline (an unreachable proxy), so
  avatars, emoji and API calls fail alike. Use it behind an intercepting
  HTTPS proxy Chromium does not trust: there GHD's first request opens an
  "Untrusted server" dialog over every scenario. `PARITY_GHD_ARGS` adds
  other Chromium switches.
- GHD runs without the Chromium sandbox when the harness runs as root.
- Masks placed for macOS text do not always cover the same text on Linux
  (Noto Sans wraps differently from SF).

## Limits

- Native chrome is not captured as pixels by either side (context menus,
  the app menu bar, open/save panels). Contextual menus are compared as
  item lists instead: GHD's `show-contextual-menu` IPC is intercepted in the
  renderer (GHD's menu is not popped: CDP cannot dismiss a main-process
  menu), while Corvane pops its real `NSMenu`, records it and closes it after
  `CORVANE_MENU_HOLD_MS` (default 1500) so the control loop is only held
  briefly (`context_menu` / `context_menu_pick` steps). Pixel comparisons of
  popped menus need a screen capture: do them as a visual pass with a
  screen-capable tool.
- GHD keyboard shortcuts that are menu accelerators never reach the renderer
  through CDP; use `menu:` steps. Plain keys (arrows, Enter, Escape, Space,
  typing, ⌘A in text fields) work as keys in both.
- Relative dates ("15 days ago") move with the clock; both apps see the same
  clock, so they still compare equal.
- Caret blink is not synchronised; a caret costs a few pixels at most.
