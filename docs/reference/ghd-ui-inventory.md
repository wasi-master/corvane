# GitHub Desktop 3.6.6 — UI + feature inventory

Source: `desktop/desktop` at `release-3.6.6` (published 2026-09-21), plus live screenshots of the installed app on macOS 26.4 (dark theme, 1367×814 window) taken 2026-09-28. File paths below are relative to `app/` in the GHD repo.

## A. Layout skeleton

```
window (hiddenInset title bar, 32 px on macOS 26 "Tahoe"; 22/26 on older)
├─ #desktop-app-toolbar  50 px, full width
│   ├─ Repository dropdown   (fills sidebar width, resizable with sidebar)   octicon: repo
│   ├─ [Worktree dropdown]   230 px  (shown with linked worktrees)          octicon: file-directory
│   ├─ Branch dropdown       230 px  (resize handle 6 px between buttons)    octicon: git-branch
│   └─ Push/Pull button      230 px  + 39 px arrow dropdown                  octicon: sync / arrow-up / arrow-down / upload
├─ Banner row (optional: merge success/conflicts, update available, …)
└─ #repository  (flex row)
    ├─ #repository-sidebar  default 250 px, min 220 px, resize handle 6 px
    │   ├─ TabBar 29 px:  "Changes [count badge]" | "History"   (selected: 3 px blue underline)
    │   ├─ Changes: [FilterOptions ▾][Filter input] / "☑ N changed files" header / file rows 29 px / CommitMessage form (avatar, Summary input, Description textarea 80 px, action icons, "Commit to <branch>" primary button) / "Committed N ago · <summary> [Undo]" bar after a commit
    │   └─ History: "Select Branch to Compare…" input / commit rows (summary bold, avatar 16 px, author · relative time) / Ahead|Behind tabs when comparing
    └─ content
        ├─ Changes: diff for selected file  |  NoChanges blankslate  |  MultipleSelection  |  StashDiffViewer
        └─ History: SelectedCommit = header (summary, expand-details button) + meta (avatar, author, SHA + copy, +adds −dels) + [file list 250 px resizable | diff]
Overlays: #foldout-container (z 17) anchored under toolbar buttons; <dialog> popups (z 19) with overlay (z 18, rgba(0,0,0,.4) light / .5 dark); tooltips z 20.
```

### Measured constants (`styles/_variables.scss`, `main-process/app-window.ts`, `lib/stores/app-store.ts`)

| Token | Value |
|---|---|
| Window min / default | 960 × 660 |
| `--toolbar-height` | 50 px (buttons 49 px tall, arrow button 39 px wide) |
| Toolbar button widths | branch 230, push-pull 230, repository = sidebar width |
| Sidebar | default 250, min 220 |
| Commit-detail file list | default 250, min 100; diff min 150 |
| `--tab-bar-height` | 29 px |
| List row height | 29 px (repositories, changes, branches, commits) |
| Fonts | `system-ui` 12 px base; sm 11; md 14; lg 28; xl 32; xxl 42; xs 9; semibold 600; light 300 |
| Monospace | `SFMono-Regular, Consolas, Liberation Mono, Menlo, monospace` |
| `--spacing` | 10 px (half 5, third 3.33, double 20, triple 30, quad 40, quint 50) |
| `--border-radius` | 6 px; `--outlined-border-radius` 3 px |
| Button / text-field height | 25 px |
| Dialog | min-width 400, max 600 (wide 750), header 50 px, padding 20 px, shadow `0 2px 7px rgba(71,83,95,.19)` |
| Foldout popup | min 260 / max 400 (branch foldout observed 365 px wide) |
| Ahead/behind badge | 13 px tall, radius 8, font 9 px |
| Diff | line-number column 50 px ×2, line padding-y 2 px |
| Resize handle | 6 px, `ew-resize` |
| Zoom steps | 0.67 .75 .8 .9 1 1.1 1.25 1.5 1.75 2 |

### Observed screens (2026-09-28)

- **Repository foldout**: `[🔍 Filter                ][Add ▾]`; groups `Recent`, then per-account (`wasi-master`), `Other`; row = octicon (repo / lock / fork / device-desktop for local), name, right-aligned ahead/behind arrows badge or blue dot (uncommitted changes). Selected row highlighted.
- **Branch foldout**: `[🔍 Filter      ][New Branch]`; groups `Default Branch`, `Recent Branches`, `Other Branches`; row = check mark when current, name, right-aligned relative date; footer button `⑂ Choose a branch to merge into <current>` spanning width. (Pull Requests tab appears only for GitHub repos.)
- **Worktree foldout** (flagged): `[🔍 Filter][New Worktree]`; `Main Worktree` / `Linked Worktrees`; row = check, name, right-aligned branch.
- **NoChanges blankslate**: title "No local changes", subtitle, right-side illustration; suggested-action cards: "Pull N commit(s) from the origin remote" (primary, blue tint) / "Open the repository in your external editor" / "View the files of your repository in Finder" / "Open the repository page on GitHub in your browser" — each with description, keyboard hint (`Repository menu or ⌘⇧A`) and right-aligned button.
- **Changes list row**: checkbox 13 px, path with directory dimmed and filename bold, right octicon status square (yellow dot = modified, green + = added, red − = deleted, blue → = renamed, orange ! = conflicted).
- **Diff header**: file path (dir dimmed) left; right: gear ▾ menu (hide whitespace changes, side-by-side, tab size…) and status icon. Hunk header row with expand-up/down handles; gutter shows check marks per selectable line; selected lines blue.
- **Commit form**: 25 px avatar, `Summary (required)` input, `Description` textarea (~80 px) with bottom-left icons (add co-authors, Copilot sparkle, gear), `Commit to main` full-width primary button. After committing: `Committed 20 days ago` / summary text / `Undo` button bar.
- **Toolbar Push/Pull variants**: "Fetch origin · Last fetched N ago", "Pull origin · Last fetched N ago [1↓]", "Push origin [1↑]", "Publish branch", "Publish repository · Publish this repository to GitHub", "Fetching origin · Hang on…" with progress fill.
- **Publish Repository dialog**: tabs `GitHub.com | GitHub Enterprise`; Name, Description, `☑ Keep this code private`, Organization popup; Cancel / Publish Repository.
- **Settings dialog** (≈500×470, vertical nav 150 px): Accounts · Integrations · Copilot · Git · Appearance · Notifications · Prompts · Advanced · Accessibility; footer Cancel / Save.
  - Accounts: GitHub.com card (avatar, name, @login, Sign Out); GitHub Enterprise paragraph + `Sign Into GitHub Enterprise` primary button.
  - Integrations: External Editor popup; Shell popup.
  - Git: sub-tabs Author (Name, Email popup) | Default branch | Hooks; note "These preferences will edit your global Git config file".
  - Appearance: Theme radio cards Light / Dark / System with previews; Formatting: Date Format, Time Format, Number Format popups, `☐ Prefer absolute dates over relative`; Diff: Tab Size popup `4 (default)`.
  - Notifications: `☑ Enable notifications` + permission link.
  - Prompts: "Show a confirmation dialog before…" checkboxes: Removing repositories, Discarding changes, Discarding changes permanently, Discarding stash, Checking out a commit, Force pushing, Undo commit, Overriding commit message with generated message, Removing worktrees, Committing changes hidden by filter. "If I have changes and I switch branches…" radios: Ask me where I want the changes to go / Always bring my changes to my new branch / Always stash and leave my changes on the current branch. Commit Length: `☑ Show commit length warning`.
  - Advanced: Background updates `☑ Show status icons in the repository list` (+ explanation); Usage `☑ Help GitHub Desktop improve by submitting usage stats` (Corvane: omit); Network and credentials `☐ Use Git Credential Manager`.
  - Accessibility: `☑ Underline links`, `☑ Show check marks in the diff`.

## B. macOS menu bar (`main-process/menu/build-default-menu.ts`; verified live)

| Menu | Items |
|---|---|
| GitHub Desktop | About · Settings… ⌘, · Install Command Line Tool… · Services · Hide ⌘H · Hide Others ⌥⌘H · Show All · Quit ⌘Q |
| File | New Repository… ⌘N · Add Local Repository… ⌘O · Clone Repository… ⇧⌘O |
| Edit | Undo ⌘Z · Redo ⇧⌘Z · Cut ⌘X · Copy ⌘C · Paste ⌘V · Select All ⌘A · Find ⌘F (+ system AutoFill/Dictation/Emoji) |
| View | Show Changes ⌘1 · Show History ⌘2 · Show Repository List ⌘T · Show Branches List ⌘B · Show Worktrees List ⌥⌘W (flag) · Go to Summary ⌘G · Show/Hide Stashed Changes ⌃H · Show/Hide Changes Filter ⌘L · Toggle Full Screen ⌃⌘F · Reset Zoom ⌘0 · Zoom In ⌘= · Zoom Out ⌘- · Expand Active Resizable ⌘9 · Contract Active Resizable ⌘8 · Toggle Developer Tools ⌥⌘I |
| Repository | Push / Force Push… ⌘P · Pull ⇧⌘P · Fetch ⇧⌘T · Remove… ⌘⌫ · View on GitHub ⇧⌘G · Open in <Shell> ⌃` · Show in Finder ⇧⌘F · Open in <Editor> ⇧⌘A · Open With… ⇧⌥⌘A · Create Issue on GitHub ⌘I · New Worktree… ⇧⌘W (flag) · Repository Settings… |
| Branch | New Branch… ⇧⌘N · Rename… ⇧⌘R · Delete… ⇧⌘D · Discard All Changes… ⇧⌘⌫ · Stash All Changes ⇧⌘S · Update from <default> ⇧⌘U · Compare to Branch ⇧⌘B · Merge into Current Branch… ⇧⌘M · Squash and Merge into Current Branch… ⇧⌘H · Rebase Current Branch… ⇧⌘E · Compare on GitHub ⇧⌘C · View Branch on GitHub ⌥⌘B · Preview Pull Request ⌥⌘P · Create Pull Request ⌘R |
| Window | Minimize ⌘M · Zoom · Close ⌘W · Bring All to Front |
| Help | Report Issue… · Contact GitHub Support… · Show User Guides · Show Keyboard Shortcuts · Show Logs in Finder |

Other shortcuts: **⌃Tab** toggles Changes/History; **⌘Enter** commits from anywhere in the commit form; **Esc** closes foldouts/dialogs; **⌘⇧P** hint shown in blankslate for Pull.

## C. Dialogs — `PopupType` (`models/popup.ts`)

| Popup | Purpose | v1 |
|---|---|---|
| RenameBranch / DeleteBranch / DeleteRemoteBranch | branch rename/delete (+ remote) | ✔ |
| ConfirmDiscardChanges / ConfirmDiscardSelection / DiscardChangesRetry | discard files / selected lines; retry after trash failure | ✔ |
| Preferences | Settings (tabs in §A) | ✔ (no Copilot tab) |
| RepositorySettings | Remote · Ignored Files · Git Config · Fork Behavior | ✔ (no Fork Behavior) |
| AddRepository / CreateRepository / CloneRepository | add local / init (README, .gitignore, license) / clone (GitHub.com, Enterprise, URL tabs) | ✔ |
| CreateBranch / CreateTag / DeleteTag | ref creation/deletion | ✔ |
| SignIn / GenericGitAuthentication | device-flow sign-in; username/password prompt for non-GitHub remotes | ✔ |
| SAMLReauthRequired / InvalidatedToken | re-auth | TODO |
| About / Acknowledgements / TermsAndConditions / ReleaseNotes / ThankYou | info | About ✔, rest TODO |
| InstallGit / CLIInstalled / MoveToApplicationsFolder / InstallingUpdate | install/update | InstallGit ✔, InstallingUpdate ✔ (self-updater), rest TODO |
| PublishRepository / UpstreamAlreadyExists / PushBranchCommits / PushNeedsPull / ConfirmForcePush / WarnForcePush | publish/push edge cases | ✔ |
| PushRejectedDueToMissingWorkflowScope | re-auth for workflow scope | TODO |
| UntrustedCertificate / AddSSHHost / SSHKeyPassphrase / SSHUserPassword | TLS/SSH prompts | ✔ via askpass |
| RemoveRepository / ChangeRepositoryAlias | repo list mgmt | ✔ |
| ExternalEditorFailed / OpenWithExternalEditor / OpenShellFailed | integration errors | ✔ |
| CopilotAppNotFound | — | omit |
| InitializeLFS / LFSAttributeMismatch / OversizedFiles | LFS & big files | ✔ |
| CommitConflictsWarning / CommitMessage / ConfirmCommitFilteredChanges / UnknownAuthors / HookFailed / CommitProgress | commit flow | ✔ |
| StashAndSwitchBranch / ConfirmOverwriteStash / ConfirmDiscardStash | stash | ✔ |
| ConfirmCheckoutCommit / WarnLocalChangesBeforeUndo / WarningBeforeReset / UnreachableCommits / LocalChangesOverwritten | history ops | ✔ |
| MultiCommitOperation | rebase / cherry-pick / squash / merge / reorder wizard (ChooseBranch → WarnForcePush → ShowProgress → ShowConflicts → HideConflicts → ConfirmAbort → CreateBranch) | ✔ |
| CreateFork / ChooseForkSettings / DeletePullRequest / StartPullRequest / PullRequestChecksFailed / CICheckRunRerun / PullRequestReview / PullRequestComment | GitHub/PR | TODO |
| CreateTutorialRepository / ConfirmExitTutorial | tutorial | TODO |
| PushProtectionError / BypassPushProtection | secret scanning | TODO |
| GenerateCommitMessage* / Copilot* / EditCopilotBYOK* | Copilot | omit |
| AddWorktree / RenameWorktree / DeleteWorktree / DeleteWorktreeFailed | worktrees | TODO |
| Error | generic error with git stderr | ✔ |

Dialog chrome: `ui/dialog/{dialog,header,content,footer,ok-cancel-button-group,error,success}.tsx`.

## D. Foldouts — `FoldoutType` (`lib/app-state.ts`)

`Repository` (filter, groups, Recent, footer "Add ▾": Clone…, Create New…, Add Existing…) · `Branch` (tabs Branches | Pull Requests, filter, New Branch, merge row) · `AppMenu` (Win/Linux only) · `AddMenu` · `PushPull` (arrow dropdown: fetch / pull / push variants, force push) · `Worktree` (flag).

## E. Feature list (core parity = v1; rest in TODO.md)

- **Repository mgmt**: add local, create (README/.gitignore/license), clone (GitHub.com/GHE/URL, progress), remove (optionally move to trash), alias, missing-repo recovery, indicators (blue dot / ahead-behind) via `RepositoryIndicatorUpdater`, Recent list, open in Finder/shell/editor.
- **Changes/commit**: include checkboxes, filter (text + included/excluded/new/modified/deleted), per-hunk/line staging via diff selection, discard (all/selected/lines), ignore file/extension, summary + description, co-authors (`@` autocomplete), amend, undo last commit, `--no-verify`/`--signoff`/`--allow-empty`, hook progress dialog, commit length warning, oversized-file warning, stash entry banner.
- **History**: virtualized commit list, multi-select (contiguous + non-contiguous), expandable summary, file list + diff, compare-to-branch (Ahead/Behind), tags, context menu (revert, amend, cherry-pick, create branch/tag, checkout, reset, copy SHA, view on GitHub), drag commits onto branches, unreachable-commits dialog.
- **Branches**: create/rename/delete, checkout with stash strategy (`UncommittedChangesStrategy`), groups Default/Recent/Other, update from default, branch pruning (`branch-pruner.ts`).
- **Remote ops**: fetch/pull/push/force-push/publish; background fetch (`BackgroundFetcher`: default 60 min, min 5 min, ≤30 s jitter; manual throttle 30 min); tags-to-push; credential prompts; LFS init; progress in toolbar button.
- **Conflicts**: merge/rebase/cherry-pick conflict state; conflicts dialog with per-file "use ours/theirs" or open editor; banners (`BannerType`: SuccessfulMerge, MergeConflictsFound, SuccessfulRebase, RebaseConflictsFound, BranchAlreadyUpToDate, SuccessfulCherryPick, …Undone, ConflictsFound, OSVersionNoLongerSupported).
- **Stash**: stash all / stash-on-switch, view diff, restore, discard.
- **Multi-commit ops** (`MultiCommitOperationKind`): Rebase, Cherry-pick, Squash, Merge, Reorder; undo state.
- **Onboarding** (`WelcomeStep`): Start → SignInToDotComWithBrowser / SignInToEnterprise → ConfigureGit.

## F. Theme model

Themes: Light (`:root` in `styles/_variables.scss`) and Dark (`body.theme-dark` in `styles/themes/_dark.scss`); `ApplicationTheme` = light | dark | system. Palette = Primer `color-system.scss` SCSS vars (`$blue`, `$gray-900`, …) exposed as CSS custom properties. Full token list: `ghd-theme-tokens.md`.

## G. Git execution + refresh model

- Shells out to bundled git via `dugite` (`lib/git/core.ts`), no libgit2. Askpass via `desktop-trampoline`.
- **No filesystem watcher.** Refresh is event-driven: window focus → `refreshRepository`; after every dispatcher git action; on repo selection. Background `RepositoryIndicatorUpdater` every 15 min (+≤30 s skew, paused when blurred, first run 2 min after launch); `BackgroundFetcher` (above); `PullRequestUpdater` 30 min.
- `git status --porcelain=v2 -z --branch --untracked-files=all`; `git diff --no-ext-diff --patch-with-raw -z --no-color`; `git log -z --format=%H%x00…`; partial staging via `git apply --cached --unidiff-zero -` (`lib/patch-formatter.ts`); reorder/squash via `GIT_SEQUENCE_EDITOR`; progress parsed from `--progress` stderr.
- Syntax highlighting: CodeMirror 5 modes in Web Workers, first 256 KB of each side, only lines the diff needs (`docs/technical/syntax-highlighting.md`).
