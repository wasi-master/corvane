# TODO — deferred features

Everything GitHub Desktop 3.6.6 has that Corvane does not, with the GHD source location. Move an item to `PLAN.md` milestones when scheduled. Features that exist but differ from GHD in detail live in `docs/reference/deviations.md`, not here.

Legend: **[GH]** needs GitHub API · **[UI]** UI-only · **[PLAT]** platform work · **[INFRA]** build/release

## Copilot (omitted by design)

- [ ] Decide whether to expose a provider-agnostic "AI commit message" hook (`GenerateCommitMessage*`, `Copilot*` popups, Settings › Copilot tab, Prompts › "Overriding commit message with generated message"). Not planned; keep menu/tab out of Corvane to avoid dead UI.

## Platform

- [ ] **[PLAT]** Windows: in-app menu bar (`ui/app-menu/`), 28 px custom title bar, `x-github-desktop-auth` style protocol registration, Credential Manager, NSIS via Velopack, editor/shell detection (`lib/editors/win32.ts`)
- [ ] **[PLAT]** Linux: Wayland/X11 via `gpui_wgpu`, secret-service keyring, AppImage/deb/flatpak, editor/shell detection (`lib/editors/linux.ts`)
- [ ] **[PLAT]** macOS 14 support via raw GPUI (currently blocked by gpui-kit's 15+ floor)
- [ ] **[PLAT]** Tree-sitter grammar packs for Linux / Windows: `packaging/packs.sh` builds the macOS dylib only; `corvane-grammars` builds as `.so` / `.dll` unchanged and the loader and manifest (`target` = `linux-x86_64`, `windows-x86_64`, …) are platform-neutral, so a per-target build + upload step is what is missing

## Packs (PLAN.md §3.7)

- [ ] "Download extended highlighting (6 MB)?" banner on the first diff whose extension the built-in grammar set does not know; today the pack is only offered in Settings › Advanced › Optional components
- [ ] `git-portable` (dugite-native git + lfs) and `git-lfs` packs: the manifest and downloader accept them, but nothing builds or offers them (`InstallGit` / `InitializeLFS` would offer the download)

## Infra / release

- [ ] **[INFRA]** Developer ID signing + notarization — not planned (hobby project, no paid Apple Developer account). If it ever happens: `rcodesign` notarize step in release CI, drop `--no-quarantine` from cask docs
- [ ] **[INFRA]** Velopack or Sparkle — rejected for now (both assume signed + notarized bundles); custom self-updater in M7 instead
- [ ] **[INFRA]** `.github/workflows/release.yml`: fmt/clippy/tests, two `--target` builds + `lipo`, `packaging/release.sh`, minisign with the repository secret, upload on tag (`packaging/release.md` has the signing step)
- [ ] **[INFRA]** Screenshot-regression CI job on `macos-15` runner (`cargo build --features snapshots` + `CORVANE_SNAPSHOT=<png>` renders a window offscreen)
- [ ] **[INFRA]** `cargo vendor` snapshot of `gpui-pre`/`gpui-kit` in release builds

## Flags (`docs/reference/flags.md`)

New Corvane-only extras land behind a flag that is off in the Corvane preset and on in Everything (`everything: ON` in `crates/corvane-core/src/flags/registry.rs`). Candidates:

- [ ] Opt out of update checks (desktop/desktop#3410; `corvane_core::updater::updates_enabled`), `5xx`
- [ ] Flags dialog: ↑ / ↓ row navigation, a "Reset to preset" per category, a link that opens the flag's deviations.md entry

## Accessibility

- [ ] Keyboard navigation leftovers (audit 2026-09-29, `deviations.md` › Keyboard): arrow / Enter navigation from the filter box into the branch, pull request and worktree lists (GHD `FilterList`; the repository list has it), PageUp / PageDown in lists, Enter submitting a dialog's default button when no text box has focus, Shift+F10 opening the selected row's context menu, Tab traversal through toolbar buttons and list rows
- [ ] Full VoiceOver walkthrough (list containers own their rows, banners and the Expand/Contract Active Resizable announcement are live regions, icon-only buttons and dialogs are labelled)
- [ ] Windows Narrator support once Windows lands (GPUI AccessKit gap on Windows)
