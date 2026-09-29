# Deviations from GitHub Desktop 3.6.6

Features Corvane has that behave or look slightly differently from GHD, with the reason. Unbuilt features are in `TODO.md`; each module's doc comment also names its deviations next to the GHD file it ports.

## GitHub layer

- **Pull request updater** runs every 30 minutes whether or not the window is focused (GHD starts/stops it on focus). Cheap: one list call per repository.
- **CI check runs**: GHES version gating (`lib/endpoint-capabilities.ts`) is reduced to GitHub.com vs. Enterprise (Enterprise gets no re-run and no Actions job steps). The "loading" spinner does not animate. Views subscribe to a ref's status by touching it while rendering (`Dispatcher::touch_commit_status`, never notifies); a key nobody rendered for five minutes stops refreshing (GHD subscribes on mount / unsubscribes on unmount).
- **Pull request cache** is keyed by endpoint + `owner/name` in redb (`pull-requests:<key>`) with the head/base repositories embedded in each record; GHD keys IndexedDB rows by the GitHub repository's database id.
- **Preview Pull Request**: the Diff Settings gear sits in the selected file's header (GHD: in the "Showing changes from all commits" row). No `PushBranchCommits` prompt: an unpublished branch is pushed automatically before the compare page opens.
- **Forks**: the "fork this repository?" prompt follows a push refused with "Permission denied" (GHD reads the repository's API `permissions` up front, see TODO.md).
- **Repository rules**: GHD skips the rules API for free-plan private repositories (`useRepoRulesLogic`); Corvane always asks and treats 404 as "no rules". The commit-message check formats summary + description without the `Co-Authored-By` trailers. Committer-email rules are fetched but not checked (the committer identity is not known in the form).
- **Re-authorization** (`workflow` scope, SAML SSO, invalidated token) opens the device-flow sign-in dialog; GHD's browser flow retries the push automatically after signing in.
- **Clone dialog** caches the repository list in redb per endpoint and filters it locally (as GHD does); there is no server-side search.
- **Unknown co-author lookup** stores a resolved handle with GHD's stealth e-mail (`login@users.noreply.github.com`) when the profile has no public e-mail.

## Editor / commit form

- Co-author tokens stay on one line (GHD's `AuthorInput` is a wrapping CodeMirror field); the row does not grow.
- Spellcheck language follows `NSSpellChecker`'s automatic identification; there is no per-language picker and no Chromium-style "Ignore" item.
- The "commit summary is long" warning is the Copilot-free variant.

## Diff viewer

- Overlaid image modes (Swipe, Onion Skin, Difference) letterbox both images with `ObjectFit::Contain`; GHD top-left aligns them. The "Difference" blend is computed on the CPU.
- Split mode pairs deletions with additions per block like GHD, but long lines are not word-wrapped inside a column (rows keep one line).

## Settings

- Notifications › permission hint only means something from the signed `.app` bundle (`UNUserNotificationCenter` needs a bundle); a bare binary shows no hint.
- Settings › Copilot tab, Git › Hooks sub-tab, Advanced › Usage and the Git Credential Manager toggle are omitted (see the module doc of `dialogs/preferences.rs`).

## Window / menus

- `Window › Close Window` ⌘W hides the app (GPUI has no per-window hide with a Dock relaunch); GHD hides just the window.
- Worktree toolbar button appears only with linked worktrees (or while its foldout is open), as in GHD, but the buttons are not resizable (TODO.md).
