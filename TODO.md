# TODO — deferred features

Everything GitHub Desktop 3.6.6 has that Corvane does not, with the GHD source location. Move an item to `PLAN.md` milestones when scheduled. Features that exist but differ from GHD in detail live in `docs/reference/deviations.md`, not here.

Legend: **[GH]** needs GitHub API · **[UI]** UI-only · **[PLAT]** platform work · **[INFRA]** build/release

## GitHub layer

- [ ] **[GH]** Notifications: reviews, comments, failed checks via Alive websockets (`lib/stores/notifications-store.ts`, `alive-store.ts`) that open the built `PullRequestReview` / `PullRequestComment` / `PullRequestChecksFailed` dialogs; Settings › Notifications is persisted but nothing emits notifications yet
- [ ] **[GH]** GitHub Enterprise OAuth (needs GHES-registered OAuth app); v1 = PAT only
- [ ] **[GH]** Browser OAuth web flow with loopback / `x-corvane-auth://` as alternative to device flow (`docs/technical/oauth.md`)
- [ ] **[GH]** Evaluate `octocrab` + `graphql_client` once GraphQL-heavy PR features land

## Copilot (omitted by design)

- [ ] Decide whether to expose a provider-agnostic "AI commit message" hook (`GenerateCommitMessage*`, `Copilot*` popups, Settings › Copilot tab, Prompts › "Overriding commit message with generated message"). Not planned; keep menu/tab out of Corvane to avoid dead UI.

## Worktrees (GHD 3.6 feature flag)

- [ ] `mainWorktreePath` for unsafe (untrusted) repositories (the fallback to the main worktree when a linked worktree is deleted is built)

## Tutorial + onboarding extras

- [ ] Tutorial repository + right-hand `TutorialPanel` (`ui/tutorial-panel/`, `TutorialStep`), `CreateTutorialRepository`, `ConfirmExitTutorial`
- [ ] Welcome "Create a tutorial repository" card
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
- [ ] **[PLAT]** OS notifications (`ui/notifications/`), `TestNotifications` popup
- [ ] **[PLAT]** `Install Command Line Tool…` (`github` CLI shim → `corvane` shim), Finder Services "Open in Corvane"

## Infra / release

- [ ] **[INFRA]** Developer ID signing + notarization — not planned (hobby project, no paid Apple Developer account). If it ever happens: `rcodesign` notarize step in release CI, drop `--no-quarantine` from cask docs
- [ ] **[INFRA]** Velopack or Sparkle — rejected for now (both assume signed + notarized bundles); custom self-updater in M7 instead
- [ ] **[INFRA]** Crash reporting (opt-in, local `.crash` capture only — no telemetry, ever)
- [ ] **[INFRA]** Screenshot-regression CI job on `macos-15` runner
- [ ] **[INFRA]** `cargo vendor` snapshot of `gpui-pre`/`gpui-kit` in release builds

## Accessibility

- [ ] Full keyboard navigation audit vs GHD (`docs/technical/keyboard-navigation.md` in desktop/desktop)
- [ ] VoiceOver pass over every list (rows as `ListItem`/`Row` nodes with names and selection state), live regions for banners and the Expand/Contract Active Resizable announcement, and a full walkthrough; icon-only buttons (`widgets::IconButtonA11y`), dialog roles and dialog titles as window titles are done
- [ ] High-contrast theme (GHD has none; nice-to-have)
- [ ] Windows Narrator support once Windows lands (GPUI AccessKit gap on Windows)
