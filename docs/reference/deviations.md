# Deviations from GitHub Desktop 3.6.6

Features Corvane has that behave or look slightly differently from GHD, with the reason. Unbuilt features are in `TODO.md`; each module's doc comment also names its deviations next to the GHD file it ports.

## GitHub layer

- **CI check runs**: GHES version gating (`lib/endpoint-capabilities.ts`) is reduced to GitHub.com vs. Enterprise (Enterprise gets no re-run and no Actions job steps). Views subscribe to a ref's status by touching it while rendering (`Dispatcher::touch_commit_status`, never notifies); a key nobody rendered for five minutes stops refreshing (GHD subscribes on mount / unsubscribes on unmount).
- **Pull request cache** is keyed by endpoint + `owner/name` in redb (`pull-requests:<key>`) with the head/base repositories embedded in each record; GHD keys IndexedDB rows by the GitHub repository's database id.
- **Push Branch Commits** (before a pull request is created): the base branch picked in Preview Pull Request survives the push (GHD's `onConfirm` drops it), and a failed push leaves its error on screen instead of opening the compare page anyway.
- **Forks**: the repository's API `permissions` are refreshed on selection and after signing in (as GHD) and persisted with the repository. A push to a repository the user can only read opens `CreateFork` before git runs (GHD runs the push and offers the fork after the authentication failure); repositories whose permissions were never fetched still get the fork offer after a push refused with "Permission denied". The remote URL is not rewritten when the API reports a renamed repository (GHD `updateRemoteUrl`).
- **Repository rules**: `useRepoRulesLogic` needs the account plan; an account stored before Corvane read it counts as paid until the launch refresh fills it in. The commit-message check merges the `Co-Authored-By` trailers with a port of `git interpret-trailers`' block rules instead of spawning git on every keystroke.
- **Re-authorization** (`workflow` scope, SAML SSO, invalidated token) opens the device-flow sign-in dialog instead of GHD's browser flow; the push / failed action is retried after signing in, as in GHD.
- **Pull request notification dialogs** (`PullRequestReview`, `PullRequestComment`, `PullRequestChecksFailed`, `crates/corvane-ui/src/dialogs/pull_request_notifications.rs`) are only reachable through `CORVANE_POPUP=pr-review|pr-comment|pr-checks-failed` until the Alive trigger lands (TODO.md). "Switch to Pull Request" closes the dialog before switching (GHD spins in the header until the checkout is done); checks-failed shows the job steps the commit status store already has (no "Stand By" pane while logs load); re-running replaces the dialog, since Corvane shows one popup at a time.
- **Pull request quick view** (`crates/corvane-ui/src/pull_request_list.rs`): the card is a fixed 400 px wide (GHD's `min-width`), shows the list item's "opened … by author" line next to the `#N` badge (Corvane addition), and is positioned from the card height measured on the previous frame instead of staying hidden until the Markdown iframe has loaded.
- **Clone dialog** caches the repository list in redb per endpoint and filters it locally (as GHD does); there is no server-side search.
- **Clone dialog** resolution (`corvane_core::clone_info`): when every account answers 404 for an `owner/name` shorthand, Corvane shows GHD's "We couldn't find that repository" error (GHD passes the bare alias to git, which fails); when a lookup fails otherwise (offline, anonymous rate limit) the shorthand is cloned as `https://github.com/owner/name.git`. The account picker's filter is a fuzzy match on login and endpoint, and the list has no keyboard navigation.

## Markdown

- Markdown (release-notes pretext, pull request bodies, reviews and comments) is laid out natively by `crates/corvane-ui/src/markdown.rs` from a `pulldown-cmark` walk (`corvane_core::markdown`) instead of GHD's `marked` + DOMPurify output in a sandboxed webview. Headings, paragraphs, emphasis, strikethrough, inline and fenced code, links, lists, quotes and rules are styled after `static/common/markdown.css`; images show their alt text, tables become " | "-separated lines, raw HTML loses its tags. Text is not selectable, inline code keeps the paragraph size, and GHD's markdown filters (emoji, `@mentions`, `#123` / commit links, videos) are not applied; bare URLs are still linkified.

## Editor / commit form

- Spellcheck language follows `NSSpellChecker`'s automatic identification; there is no per-language picker and no Chromium-style "Ignore" item.
- The "commit summary is long" warning is the Copilot-free variant.
- Commit message templates (Corvane addition; GHD 3.6.6 ignores `commit.template`): the repository's resolved `commit.template` file, comment lines (`core.commentChar`) and outer blank lines stripped, prefills the description while the summary is empty and the description is empty or still the template; it comes back after every commit (`corvane_git::commit_template`).

## Diff viewer

- The image "Difference" mode blends on the CPU (GPUI has no `mix-blend-mode`), at the two images' on-screen relative scale; it is recomputed when that scale changes.

## Settings

- Notifications › permission hint only means something from the signed `.app` bundle (`UNUserNotificationCenter` needs a bundle); a bare binary shows no hint.
- Settings › Copilot tab, Git › Hooks sub-tab, Advanced › Usage and the Git Credential Manager toggle are omitted (see the module doc of `dialogs/preferences.rs`).

## Window / menus

- Move to Applications prompt: the backdrop dismisses it (GHD: `backdropDismissable={false}`); the move never asks for administrator rights, so it fails with the error when `/Applications` is not writable (Electron can authorize).
- About › License and Open Source Notices lists the Rust crates in the macOS build (`cargo about`, regenerated by `packaging/acknowledgements.sh`); license texts are not selectable. GHD's "Terms and Conditions" link is omitted (GitHub's terms do not apply to Corvane).
- Help › Show Release Notes (Corvane addition; GHD shows the dialog only after an update) opens the `ReleaseNotes` dialog for the running version from the GitHub Releases API. Release bodies are Markdown: `[Kind]` list items are classified as in GHD, untagged items by their `##` heading (GHD drops them), entries are plain text instead of `RichText`, and there is no "Install and Restart" button (`crates/corvane-core/src/release_notes.rs`).
- View › Expand / Contract Active Resizable (⌘9 / ⌘8) resize the focused pane by GHD's 5 px without GHD's aria-live "width increased. Set to N%" announcement. The commit, stash and pull request file lists take keyboard focus when clicked so the items apply to them (`crates/corvane-ui/src/active_resizable.rs`).
- A deleted linked worktree falls back to its main worktree as in GHD (`recoverMissingWorktree`), but Corvane records the main worktree path on every refresh rather than only on a worktree switch, because `git worktree remove` also deletes the metadata GHD's fallback reads. Repository entries saved before this cannot recover and show as missing (Locate…).
- Worktree toolbar button appears only with linked worktrees (or while its foldout is open), as in GHD.
- Resizable toolbar buttons (`crates/corvane-ui/src/toolbar.rs`, `corvane_core::toolbar_widths`): the worktree and branch buttons resize as in GHD; the push/pull button keeps its 230 px (GHD resizes it too), the handles do not take ⌘9 / ⌘8 or announce the new width, and the width is saved when the drag ends rather than on every move.

## Scrolling

- Scrollbars reproduce Chromium's macOS scrollers (`crates/corvane-ui/src/scrollbar.rs`), overlay or legacy as `NSScroller.preferredScrollerStyle` says. Mouse-wheel ticks are animated with Chromium's smooth-scroll curve; Electron on macOS only does that when `NSScrollAnimationEnabled` is set, so GHD usually jumps 40 px per tick.
- The diff list estimates rows it has not rendered yet at one line (20 px), so the thumb can shift slightly as wrapped lines come into view; GHD's react-virtualized list estimates too.
