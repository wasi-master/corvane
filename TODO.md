# TODO — deferred features

Everything GitHub Desktop 3.6.6 has that Corvane does not, with the GHD source location. Move an item to `PLAN.md` milestones when scheduled. Features that exist but differ from GHD in detail live in `docs/reference/deviations.md`, not here.

Legend: **[GH]** needs GitHub API · **[UI]** UI-only · **[PLAT]** platform work · **[INFRA]** build/release

## GitHub layer

- [ ] **[GH]** Alive websocket producer for pull request notifications (`lib/stores/alive-store.ts`, the event handling in `notifications-store.ts`: PR cache lookup, API fetch of the review / comment / checks, commit-author and check-suite dedup) feeding `Dispatcher::notify_pull_request_event`; posting, clicks and the dialogs are built
- [ ] **[GH]** GitHub Enterprise OAuth (needs GHES-registered OAuth app); v1 = PAT only
- [ ] **[GH]** Browser OAuth web flow with loopback / `x-corvane-auth://` as alternative to device flow (`docs/technical/oauth.md`)
- [ ] **[GH]** Evaluate `octocrab` + `graphql_client` once GraphQL-heavy PR features land

## Copilot (omitted by design)

- [ ] Decide whether to expose a provider-agnostic "AI commit message" hook (`GenerateCommitMessage*`, `Copilot*` popups, Settings › Copilot tab, Prompts › "Overriding commit message with generated message"). Not planned; keep menu/tab out of Corvane to avoid dead UI.

## Repository view

- [ ] `MissingRepository` "Can't find" variant for a deleted directory (`ui/missing-repository.tsx`: "It was last seen at …", Check again, Locate…, Clone Again, Remove); only the unsafe-repository variant is built, a missing repository keeps the normal view

## Tutorial + onboarding extras

- [ ] Import repository list from GitHub Desktop's own data dir (best-effort helper)

## History

- [ ] Cherry-pick by dropping commits on a pull request in the Pull Requests tab (`onDropOntoPullRequest`)

## Diff viewer

- [ ] Text selection + "Copy" in the diff context menu (GPUI's static text is not selectable; needs a custom selection layer across the virtualized rows)
- [ ] tree-sitter grammar packs as an alternative highlighter (dylib packs; codesign implications)

## View menu

- [ ] Zoom levels (`Reset Zoom` ⌘0, `Zoom In` ⌘=, `Zoom Out` ⌘-, `#window-zoom-info` overlay, 0.67…2.0 steps): GPUI has no page-zoom; Corvane's layout is in absolute pixels, so this needs a rem-based size pass first. Items stay disabled

## Platform

- [ ] **[PLAT]** Windows: in-app menu bar (`ui/app-menu/`), 28 px custom title bar, `x-github-desktop-auth` style protocol registration, Credential Manager, NSIS via Velopack, editor/shell detection (`lib/editors/win32.ts`)
- [ ] **[PLAT]** Linux: Wayland/X11 via `gpui_wgpu`, secret-service keyring, AppImage/deb/flatpak, editor/shell detection (`lib/editors/linux.ts`)
- [ ] **[PLAT]** macOS 14 support via raw GPUI (currently blocked by gpui-kit's 15+ floor)

## Packs (PLAN.md §3.7)

- [ ] "Download extended highlighting (6 MB)?" banner on the first diff whose extension the built-in grammar set does not know; today the pack is only offered in Settings › Advanced › Optional components
- [ ] `git-portable` (dugite-native git + lfs) and `git-lfs` packs: the manifest and downloader accept them, but nothing builds or offers them (`InstallGit` / `InitializeLFS` would offer the download)

## Infra / release

- [ ] **[INFRA]** Developer ID signing + notarization — not planned (hobby project, no paid Apple Developer account). If it ever happens: `rcodesign` notarize step in release CI, drop `--no-quarantine` from cask docs
- [ ] **[INFRA]** Velopack or Sparkle — rejected for now (both assume signed + notarized bundles); custom self-updater in M7 instead
- [ ] **[INFRA]** `.github/workflows/release.yml`: fmt/clippy/tests, two `--target` builds + `lipo`, `packaging/release.sh`, minisign with the repository secret, upload on tag (`packaging/release.md` has the signing step)
- [ ] **[INFRA]** Screenshot-regression CI job on `macos-15` runner (`cargo build --features snapshots` + `CORVANE_SNAPSHOT=<png>` renders a window offscreen)
- [ ] **[INFRA]** `cargo vendor` snapshot of `gpui-pre`/`gpui-kit` in release builds

## Accessibility

- [ ] Full keyboard navigation audit vs GHD (`docs/technical/keyboard-navigation.md` in desktop/desktop)
- [ ] Full VoiceOver walkthrough (list containers own their rows, banners and the Expand/Contract Active Resizable announcement are live regions, icon-only buttons and dialogs are labelled)
- [ ] Windows Narrator support once Windows lands (GPUI AccessKit gap on Windows)
