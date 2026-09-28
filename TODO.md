# TODO — deferred features

Everything GitHub Desktop 3.6.6 has that Corvane v1 (core parity) intentionally skips. Each entry names the GHD source location so the port has a reference. Move an item to `PLAN.md` milestones when scheduled.

Legend: **[GH]** needs GitHub API · **[UI]** UI-only · **[PLAT]** platform work · **[INFRA]** build/release

## GitHub layer

- [ ] **[GH]** Pull Requests tab in Branch foldout (`app/src/ui/branches/pull-request-list.tsx`, `BranchesTab.PullRequests`), PR updater every 30 min (`lib/stores/pull-request-store.ts`, `pull-request-updater.ts`)
- [ ] **[GH]** CI check-run status in branch button + PR list + popover (`ui/branches/ci-status.tsx`, `ui/check-runs/`), `CICheckRunRerun` popup
- [ ] **[GH]** Preview Pull Request dialog (`ui/open-pull-request/`, `PopupType.StartPullRequest`); the `Branch › Preview Pull Request` item stays disabled. "Create Pull Request" pushes an unpublished branch and opens `/pull/new/<branch>`
- [ ] **[GH]** PR quick view / review / comment popups (`PullRequestReview`, `PullRequestComment`, `PullRequestChecksFailed`)
- [ ] **[GH]** Notifications: reviews, comments, failed checks via Alive websockets (`lib/stores/notifications-store.ts`, `alive-store.ts`); Settings › Notifications › "Enable notifications" is persisted but nothing emits notifications yet (no permission hint either)
- [ ] **[GH]** Forks: `CreateFork`, `ChooseForkSettings`, Repository Settings › Fork Behavior tab, upstream remote handling (`lib/stores/app-store.ts` `_updateRepositoryFork…`)
- [ ] **[GH]** Repo rules / protected-branch warnings in commit form (`ui/changes/commit-warning.tsx`, `lib/api.ts fetchRepoRulesForBranch`)
- [ ] **[GH]** Secret-scanning push protection (`PushProtectionError`, `BypassPushProtection`)
- [ ] **[GH]** `PushRejectedDueToMissingWorkflowScope` re-auth flow, `SAMLReauthRequired`, `InvalidatedToken`
- [ ] **[GH]** GitHub Enterprise OAuth (needs GHES-registered OAuth app); v1 = PAT only
- [ ] **[GH]** Browser OAuth web flow with loopback / `x-corvane-auth://` as alternative to device flow (`docs/technical/oauth.md`)
- [ ] **[GH]** Clone dialog: org filter, repo search across pages, "Your repositories" caching in redb
- [ ] **[GH]** Issue creation with template picker (v1 opens `/issues/new` in browser)
- [ ] **[GH]** Evaluate `octocrab` + `graphql_client` once GraphQL-heavy PR features land

## Copilot (omitted by design)

- [ ] Decide whether to expose a provider-agnostic "AI commit message" hook (`GenerateCommitMessage*`, `Copilot*` popups, Settings › Copilot tab). Not planned; keep menu/tab out of Corvane to avoid dead UI.

## Worktrees (GHD 3.6 feature flag)

- [ ] **[UI]** Fourth toolbar button "Current Worktree" + Worktree foldout (Main Worktree / Linked Worktrees, filter, New Worktree)
- [ ] `AddWorktree`, `RenameWorktree`, `DeleteWorktree`, `DeleteWorktreeFailed` popups; `Repository › New Worktree…` ⇧⌘W; `View › Show Worktrees List` ⌥⌘W

## Tutorial + onboarding extras

- [ ] Tutorial repository + right-hand `TutorialPanel` (`ui/tutorial-panel/`, `TutorialStep`), `CreateTutorialRepository`, `ConfirmExitTutorial`
- [ ] Welcome "Create a tutorial repository" card
- [ ] Import repository list from GitHub Desktop's own data dir (best-effort helper)

## Create / clone dialogs

- [ ] Git-ignore and license templates in "Create a New Repository" (GHD bundles `gitignore` + `choosealicense` data; the selects currently show "None")
- [ ] Clone dialog GitHub.com / Enterprise tabs: repository list from the API with filter (`clone-github-repository.tsx`), needs avatar cache
- [ ] Clone path validation messages (`.app` suffix, non-empty folder, file exists) before starting

## History

- [ ] Unreachable-commits dialog (`ui/history/unreachable-commits-dialog.tsx`, `PopupType.UnreachableCommits`); the multi-commit summary shows "N unreachable commits not included." as plain text
- [ ] Drop tooltips while dragging commits ("Copy to <branch>", "Squash N commits", "Move commits here"); drop targets highlight instead
- [ ] Highlighting the commits in / not in the range diff on hover of the summary text (`onHighlightShas`)
- [ ] Cherry-pick by dropping commits on a pull request (needs the Pull Requests tab)

## Changes list

- [ ] Multi-selection (⌘/⇧-click, ⌘A) and the "N selected" context-menu variants: `Discard N Selected Changes…`, `Ignore N Selected Files`, `Include/Exclude Selected Files`, `Copy Paths` / `Copy Relative Paths`
- [ ] Rebase-conflict context menu variant (`getRebaseContextMenu`)

## Editor / commit form

- [ ] Spellcheck in commit summary/description (Electron built-in in GHD; needs native `NSSpellChecker` via objc2 on macOS)
- [ ] Emoji autocomplete `:smile:` and issue `#123` autocomplete (co-author `@` autocomplete is in v1)
- [ ] Copilot-free "commit length warning" is in v1; per-repo commit message templates are not

## Diff viewer

- [ ] Hunk expansion (fold up/down/whole, `DiffHunkExpansionType`) and the expansion handles in hunk header rows
- [ ] Gutter/hunk right-click menu: `Discard Added Line…`, `Discard Hunk…`, `Discard Selected Lines…` (`formatPatchToDiscardChanges` + `git apply --unidiff-zero`)
- [ ] Diff search (⌘F inside the diff), whitespace hint popover when `hideWhitespaceInDiff` is on
- [ ] Submodule diff panel (`ui/diff/submodule-diff.tsx`)
- [ ] Image diff modes beyond side-by-side: 2-up, swipe, onion skin, difference (`ui/diff/image-diffs/`)
- [ ] Expand-all-context in hunk headers, "Open file in external editor" from diff gear menu
- [ ] tree-sitter grammar packs as an alternative highlighter (dylib packs; codesign implications)

## Settings (`ui/preferences/`)

- [ ] Git › Hooks sub-tab (`enableGitHookEnv`, `cacheGitHookEnv`: load the user's shell environment before running hooks, `lib/shell.ts` `getShellEnv`)
- [ ] Advanced › "Use Git Credential Manager" (`useExternalCredentialHelper`: `-c credential.helper=manager` for non-GitHub remotes); Corvane answers generic prompts through its askpass helper instead
- [ ] Appearance › Formatting (date / time / number format selects, "Prefer absolute dates") — behind `enableFormattingPreferences()` in GHD 3.6.6
- [ ] Prompts › "Overriding commit message with generated message" (Copilot) and "Removing worktrees" (worktree flag)
- [ ] Integrations › custom editor / shell (`enableCustomIntegration()`: path + arguments form)
- [ ] Accounts: avatar images (needs the avatar cache), `lookupPreferredEmail`
- [ ] Notifications tab permission hint (`getNotificationsPermission`, macOS `UNUserNotificationCenter`)
- [ ] Settings dialog `DialogError` banner for an invalid author name (`gitAuthorNameIsValid`); the Save button just writes what was typed
- [ ] Editor detection on Windows/Linux (`lib/editors/win32.ts`, `linux.ts`); macOS uses LaunchServices (`NSWorkspace URLForApplicationWithBundleIdentifier`)

## View menu

- [ ] Zoom levels (`Reset Zoom` ⌘0, `Zoom In` ⌘=, `Zoom Out` ⌘-, `#window-zoom-info` overlay, 0.67…2.0 steps): GPUI has no page-zoom; Corvane's layout is in absolute pixels, so this needs a rem-based size pass first. Items stay disabled
- [ ] `Expand Active Resizable` ⌘9 / `Contract Active Resizable` ⌘8 (GHD nudges the focused resizable sidebar); items stay disabled
- [ ] `Window › Close Window` ⌘W hides the app (GPUI has no per-window hide with a Dock relaunch); GHD hides just the window

## Platform

- [ ] **[PLAT]** Windows: in-app menu bar (`ui/app-menu/`), 28 px custom title bar, `x-github-desktop-auth` style protocol registration, Credential Manager, NSIS via Velopack
- [ ] **[PLAT]** Linux: Wayland/X11 via `gpui_wgpu`, secret-service keyring, AppImage/deb/flatpak
- [ ] **[PLAT]** macOS 14 support via raw GPUI (currently blocked by gpui-kit's 15+ floor)
- [ ] **[PLAT]** OS notifications (`ui/notifications/`), `TestNotifications` popup
- [ ] **[PLAT]** `Install Command Line Tool…` (`github` CLI shim → `corvane` shim), Finder Services "Open in Corvane"
- [ ] **[PLAT]** `MoveToApplicationsFolder` prompt

## Infra / release

- [ ] **[INFRA]** Developer ID signing + notarization — not planned (hobby project, no paid Apple Developer account). If it ever happens: `rcodesign` notarize step in release CI, drop `--no-quarantine` from cask docs
- [ ] **[INFRA]** Velopack or Sparkle — rejected for now (both assume signed + notarized bundles); custom self-updater in M7 instead
- [ ] **[INFRA]** Release notes dialog (`ReleaseNotes` popup) fed from GitHub Releases body
- [ ] **[INFRA]** `About` › Acknowledgements generated from `cargo about`
- [ ] **[INFRA]** Crash reporting (opt-in, local `.crash` capture only — no telemetry, ever)
- [ ] **[INFRA]** Screenshot-regression CI job on `macos-15` runner
- [ ] **[INFRA]** `cargo vendor` snapshot of `gpui-pre`/`gpui-kit` in release builds

## Accessibility

- [ ] Full keyboard navigation audit vs GHD (`docs/technical/keyboard-navigation.md` in desktop/desktop)
- [ ] VoiceOver pass over every dialog and list (GPUI exposes AccessKit roles; labels for icon-only buttons, dialog titles as `AXWindow` titles, live regions for banners)
- [ ] High-contrast theme (GHD has none; nice-to-have)
- [ ] Windows Narrator support once Windows lands (GPUI AccessKit gap on Windows)
