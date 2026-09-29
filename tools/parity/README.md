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
`tolerance` (max channel delta, default 6) on flat pixels — fills, borders,
hover and pressed backgrounds — and within `edge_tolerance` (default 40) next
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
threshold: 1.0         # optional per-scenario defaults: threshold, tolerance, radius, settle, width, height
steps:
  - hover: [365, 56]                     # window points
  - press: {css: ".toolbar-dropdown button"}   # mouse down and hold (pressed / :active state)
  - release: [365, 56]
  - click: {text: "Integrations", within: "dialog"}   # also clicks: 2, mods: cmd-shift
  - dblclick: [120, 214]
  - drag: {from: [249, 400], to: [320, 400]}
  - scroll: {at: [124, 300], dy: 200}
  - key: cmd-a backspace                 # GPUI keystroke syntax, space separated
  - type: "feature/login"
  - menu: show-preferences               # GHD menu-event name
  - popup: {ghd: test-release-notes-popup, corvane: release-notes}   # GHD test hook / CORVANE_POPUP
  - resize: [1100, 700]
  - wait: 500                            # alone: sleep; on a step: settle time after it (default 350ms)
  - ghd: {eval: "…"}                     # app-specific step (either side can be `{skip: true}`)
    corvane: {hook: {name: popup, arg: about}}
  - dump: open                           # GHD DOM boxes + computed styles as JSON ({name, root: css})
  - snap: open                           # or {name, threshold, tolerance, edge_tolerance, radius, mask: [[x,y,w,h]…], region: [x,y,w,h], note}
```

Masks are for deliberate differences only (product name, version numbers) and
each should carry a `note` pointing at `docs/reference/deviations.md`.

## Limits

- Native chrome is not captured by either side: context menus (both apps use
  real `NSMenu`s, and a popped menu would block Corvane's control loop, so
  right-clicks are not offered), the app menu bar, open/save panels, and
  anything drawn by AppKit outside the web view / GPUI scene.
- GHD keyboard shortcuts that are menu accelerators never reach the renderer
  through CDP; use `menu:` steps. Plain keys (arrows, Enter, Escape, Space,
  typing, ⌘A in text fields) work as keys in both.
- Relative dates ("15 days ago") move with the clock; both apps see the same
  clock, so they still compare equal.
- Caret blink is not synchronised; a caret costs a few pixels at most.
