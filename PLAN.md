# Corvane — Implementation Plan

Native Rust GitHub Desktop clone. Layout, buttons, workflows and positions match GitHub Desktop 3.6.6 one-to-one. Rendering quality, motion and typography target Zed. Fast cold start, near-zero idle CPU, low resident memory.

Status (2026-09-29): M0–M7 implemented; `v0.1.0` tagged locally. The first public release still needs the maintainer's minisign key pair, the `homebrew-corvane` tap repository and the release workflow (`packaging/release.md`, `TODO.md` › Infra). Perf numbers and open budgets in `docs/perf.md`.

## 1. Context

GitHub Desktop (GHD) is Electron + React. It idles at hundreds of MB, starts slowly, and re-renders through a browser. Its UX, however, is the best-understood git GUI workflow in the industry, and users have muscle memory for it. Corvane keeps that UX exactly and replaces the engine: Rust, GPU-rendered UI (GPUI), in-process git reads (gitoxide), git CLI for writes so behaviour stays identical to GHD.

Reference artefacts for the port live in `docs/reference/`:

- `ghd-ui-inventory.md` — layout constants, menu tree, every dialog/foldout, feature list, refresh model (from `desktop/desktop@release-3.6.6` source + live screenshots).
- `ghd-theme-tokens.md` — Primer palette + GHD light/dark CSS tokens to port into `corvane-ui::theme`.

## 2. Decisions (locked with Wasi, 2026-09-28)

| Area | Decision |
|---|---|
| Platform v1 | macOS 15+ (arm64 + x86_64). Platform layer abstracted; Windows/Linux later. |
| Scope v1 | Core parity: repos, changes/commit, history, branches, remote ops, merge/rebase/squash/cherry-pick/reorder + conflicts, stash, settings, sign-in. GitHub layer (PRs, CI, notifications, forks, Copilot, tutorial, worktrees) tracked in `TODO.md`. |
| Look | GHD geometry + Primer light/dark palettes as default themes. Zed-grade rendering polish. Themeable (JSON themes). |
| UI stack | GPUI via `gpui-kit = "0.7.0"` (pins `gpui-pre = 0.3.7`). Own widgets for GHD-specific chrome; gpui-kit for Input/Textarea/VirtualList/Popover/Menu/Resizable/Dialog/Scrollbar. |
| Git engine | Hybrid: `gix 0.88` for reads; system `git` CLI (≥2.40) for writes + network. Detect git at launch, `InstallGit` dialog if missing. |
| GitHub auth | OAuth device flow (no client secret) with PAT paste fallback. Tokens in macOS Keychain via `keyring 4`. GHES: the same flows with an administrator-registered OAuth app (client ID entered per host or `CORVANE_GHES_OAUTH` at build time), else PAT. |
| HTTP | `ureq 3` (blocking, rustls) on background threads. No tokio in v1. `octocrab` evaluated and rejected 2026-09-30 (0.54.2: +1.56 MB release binary, +61 crates incl. tokio/hyper/tower and mandatory JWT/RSA crypto, cold `corvane-github` build 37 s → 196 s, a tokio runtime bridge that serialises our background threads; its only gain, retries/rate-limit backoff, is a small wrapper on `get_json_accept`). GraphQL, if ever needed: `graphql_client` 0.16 codegen only (`graphql_query_derive`, no reqwest) posted with ureq `send_json` (+66 KB, +6 crates, no duplicates); GHES GraphQL is `<host>/api/graphql`, not under `/api/v3`. |
| Highlighting | `syntect 5.3` + `two-face` grammars, line-stateful, diff-lines only, first 256 KB. Core grammar set bundled; extended set is an on-demand pack. Since 2026-09-30 GHD's CodeMirror modes are ported (`corvane-highlight::cm`) and an opt-in tree-sitter engine (flag `105-tree-sitter-highlighting`) runs 310 grammars from `corvane-grammars`, shipped as native `tree-sitter-all` / `tree-sitter-rest` packs (one gzipped library per grammar, unpacked on first use); the core syntect set adds 330 TextMate grammars from GitHub Linguist (`tools/tm-grammars`). |
| Storage | `redb 4` single file `~/Library/Application Support/Corvane/corvane.redb`. |
| Updates | Custom in-app self-updater (M7): GitHub Releases as feed, minisign-verified `.zip`, swap `Corvane.app` in place, relaunch. "Update available" banner from M0. Velopack rejected: its macOS flow requires paid signing + notarization. |
| Distribution | Homebrew tap cask (primary install path) + `.zip`/`.dmg` on GitHub Releases. Ad-hoc signed; no Apple Developer ID (hobby project, see R5). |
| Builds | `default` (lean) and `full` (batteries-included: extended grammars + portable git). |
| Bundle ID | `com.wasimaster.corvane`. Repo `github.com/wasi-master/corvane`. |
| License | MIT. `NOTICE` credits GHD (MIT), Primer Octicons (MIT), Primer colors. |

## 3. Architecture

### 3.1 Workspace layout

```
corvane/
├─ Cargo.toml                  # workspace, [profile.release] lto="fat", codegen-units=1, panic="abort", strip
├─ rust-toolchain.toml         # stable (≥1.92 for gpui-kit)
├─ crates/
│  ├─ corvane/                 # bin. main.rs: Application, window, native menus, keymap, askpass subcommand
│  ├─ corvane-ui/              # views + widgets + theme (GPUI). No git/network code.
│  ├─ corvane-core/            # AppState, RepositoryState, Dispatcher (actions), models, refresh scheduler
│  ├─ corvane-git/             # gix reads, git CLI writes, parsers, patch formatter, progress, watcher
│  ├─ corvane-github/          # device-flow auth, REST client (ureq), API models, host detection
│  ├─ corvane-store/           # redb tables, settings schema, migrations
│  ├─ corvane-highlight/       # CodeMirror ports, syntect wrapper, tree-sitter engine, grammar pack loading
│  ├─ corvane-grammars/        # tree-sitter grammars + queries (rlib for the full build, cdylib for the packs)
│  ├─ corvane-packs/           # on-demand component manifest, download, verify, install
│  └─ corvane-platform/        # keychain, trash, open-in-editor/shell, notifications, app paths, git detection
├─ assets/                     # octicons SVG subset, app icon (.icns + icon/Corvane.icon for macOS 26), default theme JSONs
├─ packaging/                  # velopack config, Info.plist template, Homebrew cask template, release scripts
├─ docs/reference/             # GHD inventory + tokens (see §1)
├─ PLAN.md  TODO.md  CLAUDE.md  NOTICE  LICENSE
```

Dependency direction: `corvane` → `corvane-ui` → `corvane-core` → {`corvane-git`, `corvane-github`, `corvane-store`, `corvane-highlight`, `corvane-packs`} → `corvane-platform`. UI never calls git/network directly; it dispatches actions.

### 3.2 State + concurrency model

Mirrors GHD's `AppStore` + `Dispatcher` + per-repo `RepositoryStateCache`, mapped onto GPUI entities.

- `Entity<AppState>`: accounts, repository list, selected repo, foldout/popup stack, settings snapshot, banners.
- `Entity<RepositoryState>` per repo: branches, tags, remotes, ahead/behind, working-directory status, selected section (Changes/History), commit list + selection, diff, conflict state, stash entry, multi-commit-operation state.
- Views hold `Entity<_>` handles and call `cx.observe` / `cx.subscribe`; re-render on change. No global mutable statics.
- `Dispatcher` methods are `async fn` run on GPUI's background executor (`cx.background_spawn`). Git CLI spawns use `smol::process::Command` (compatible with GPUI executor). Results apply to entities on the foreground via `cx.update`.
- Every mutating action ends with `refresh_repository(repo)` exactly as GHD does (`_refreshRepository`: status, remotes, branches, current section).
- Refresh triggers (GHD parity + one improvement): window focus, after every action, on repo selection, FS watcher (`notify` 8 FSEvents, 300 ms debounce, watches worktree + `.git/{HEAD,index,refs,packed-refs,MERGE_HEAD,REBASE_HEAD,CHERRY_PICK_HEAD,logs/HEAD}`, ignores `.git/objects`). Indicator updater for all repos every 15 min, background fetch every 60 min (min 5), manual fetch throttled 30 min.
- Cancellation: each repo has a `RefreshGeneration` counter; stale results are dropped.

### 3.3 Git engine (`corvane-git`)

Reads via gix (in-process, one `gix::ThreadSafeRepository` per repo, LRU object cache capped ~32 MB total):

| Read | Implementation |
|---|---|
| Refs | `repo.references()`, `head()`, remotes, upstream tracking, tags |
| Ahead/behind | merge-base + revwalk count; batched for indicator updater |
| Log | `rev_walk` topo order, chunks of 100, lazy on scroll; commit graph used when present |
| Status | `gix::status` with rename tracking + untracked. Fallback flag `status-cli` runs `git status --porcelain=v2 -z --branch --untracked-files=all` when gix errors or worktree > 200k entries |
| Diff | blob diff via `gix-diff`/`imara-diff` (Histogram), unified hunks with context 3; binary + oversize (>10 MB) short-circuit; image diff for PNG/JPG/GIF/WebP/SVG side-by-side |
| Config | user.name / user.email / core.editor read through gix config; global writes via `git config --global` |
| Stash | `refs/stash` reflog; GHD's stash message marker `!!GitHub_Desktop<branch>` is kept as is, so stashes are shared with a real GitHub Desktop |
| Conflicts | index stage 1/2/3 entries + `MERGE_HEAD`/`REBASE_HEAD`/`CHERRY_PICK_HEAD` presence |

Writes via git CLI (identical semantics to GHD, hooks + LFS + credential helpers work unchanged):

| Write | Command |
|---|---|
| Stage/unstage files | `git add -- <paths>` / `git reset -- <paths>` |
| Hunk/line staging | synthesise patch (port of GHD `app/src/lib/patch-formatter.ts`) → `git apply --cached --unidiff-zero -` |
| Commit | `git commit -F -` (+ `--amend`, `--no-verify`, `--signoff`, `--allow-empty`); co-author trailers appended |
| Branch ops | `git checkout -b`, `git branch -m`, `git branch -D`, `git push origin --delete` |
| Checkout w/ stash strategy | `git stash push --include-untracked` → checkout → optional `git stash pop` |
| Merge / squash | `git merge`, `git merge --squash` |
| Rebase / reorder / squash-in-history | `git rebase` with `GIT_SEQUENCE_EDITOR` pointing at the Corvane binary (`corvane --sequence-editor`) writing the generated todo (GHD approach) |
| Cherry-pick / revert / reset | `git cherry-pick`, `git revert`, `git reset --soft|--mixed|--hard` |
| Fetch / pull / push | `git fetch --progress --prune`, `git pull`, `git push --progress --porcelain` (+ `--force-with-lease`, `-u`) with progress parsed from stderr |
| Remotes / LFS | `git remote add|set-url`, `git lfs install --local`, `git lfs track` |

Process env for every spawn: `GIT_TERMINAL_PROMPT=0`, `LC_ALL=en_US.UTF-8`, `GIT_ASKPASS=<corvane binary>`, `CORVANE_ASKPASS_SOCKET=<unix socket>`, `-c protocol.version=2`, `-c credential.helper=` only for hosts where Corvane owns the token. `corvane --askpass <prompt>` connects to the running app, which answers from Keychain or raises `GenericGitAuthentication`. Every spawn is a `tracing` span with argv, duration, exit code; stderr retained for error dialogs.

### 3.4 UI (`corvane-ui`)

Widget map, GHD component → Corvane widget → base:

| GHD | Corvane | Base |
|---|---|---|
| `#desktop-app-toolbar` 50 px, three `ToolbarDropdown` + 39 px arrow button | `toolbar::{Toolbar, ToolbarButton, ToolbarDropdown, PushPullButton}` | custom |
| Foldouts (Repository / Branch / PushPull / AddMenu) | `foldout::Foldout` anchored under toolbar button, overlay dismiss, Esc | `gpui::anchored` + `deferred`, custom |
| Sidebar `Resizable` 250 px (min 220) | `sidebar::RepositorySidebar` | gpui-kit `Resizable` |
| `TabBar` 29 px Changes/History | `tab_bar::TabBar` | custom |
| Changes file list | `changes::ChangesList` (29 px rows, checkbox, dimmed dir + bold name, status octicon) | gpui-kit `VirtualList` |
| Filter row + "Filter Options" | `changes::FilterBar` | gpui-kit `Input`, `Popover` |
| Commit form | `changes::CommitMessage` (avatar, summary `Input`, description `Textarea`, co-author autocomplete, action icons, primary button, "Committed N ago / Undo" bar) | gpui-kit Input/Textarea + custom |
| History list | `history::CommitList` (summary, avatar, author, relative time, tag/unpushed badges) | gpui-kit `VirtualList` |
| Compare box | `history::CompareSidebar` ("Select Branch to Compare…", Ahead/Behind tabs) | gpui-kit `Input`, custom |
| Commit details | `history::SelectedCommit` (expandable summary, file list `Resizable`, diff) | custom |
| Diff viewer | `diff::DiffView` (virtual lines, dual 50 px gutters, +/- markers, check marks, hunk headers with expand, selection drag, gear menu: hide whitespace / side-by-side / tab size, context menu) | custom |
| Blankslate "No local changes" | `changes::NoChanges` suggested-action cards | custom |
| Banners | `banners::Banner` | custom |
| Dialogs (`<dialog>`, header 50 px, 400–600 px) | `dialog::Dialog` + one module per `PopupType` | gpui-kit `Dialog` restyled |
| Settings (9 vertical tabs) | `settings::SettingsDialog` | custom nav + gpui-kit form controls |
| Native menu bar | `menus::build_menus()` → `cx.set_menus` | GPUI |
| Keymap | `keymap.json` shipped in binary, GHD chords | GPUI `bind_keys` |
| Title bar | hiddenInset transparent title bar, traffic lights at GHD offset | GPUI `WindowOptions` |

Theme: `theme::Theme` struct with GHD token names (`text`, `text_secondary`, `background`, `box_background`, `box_border`, `toolbar_*`, `tab_bar_*`, `list_item_*`, `diff_*`, `button_*`, `status_*` …). Two built-ins (`ghd-light`, `ghd-dark`) generated from `docs/reference/ghd-theme-tokens.md`; `system` follows appearance. Theme also feeds gpui-kit's `ThemeColor` so shared components match. Extra themes load from `~/Library/Application Support/Corvane/themes/*.json`.

Typography: `.SystemUIFont` 12 px base (11 sm, 14 md); mono `SF Mono` → `Menlo` fallback for diffs. Line height 16/20 px per GHD SCSS. Octicons from `assets/octicons/*.svg` rendered with `svg()`.

Polish rules (Zed-grade): hairlines snapped to device pixels; hover/active states with no layout shift; foldout open/close 120 ms ease-out fade+4 px slide; list selection keyboard-navigable; focus rings only on keyboard focus; idle = zero redraws (no cursor blink when unfocused, no timers while hidden).

### 3.5 GitHub layer v1 (`corvane-github`)

Only what core parity needs: device-flow sign-in, `GET /user` + emails, list user + org repos for Clone dialog (GitHub.com tab), create repo for Publish, repo metadata for a remote (default branch, fork parent), avatar URLs (disk-cached under `avatars/`), "View on GitHub" / "Create Issue" / "Create Pull Request" open browser URLs. Everything else: `TODO.md`.

OAuth App registration is a manual step: create app at github.com/settings/developers, enable Device Flow, bake `client_id` via `CORVANE_GITHUB_CLIENT_ID` build env. Scopes `repo workflow read:user user:email`.

### 3.6 Storage (`corvane-store`, redb)

Tables: `meta` (schema version), `settings` (key → JSON), `repositories` (id → JSON: path, alias, github_repo?, missing flag, last_stash_check), `repo_indicators` (id → ahead/behind/has_changes/last_fetch), `accounts` (host → login/avatar/scopes; token lives in Keychain), `ui_state` (window bounds, sidebar width, selected repo, last section), `recent_repositories`. Writes are batched through one `StoreActor` on a background thread; reads are snapshot into `AppState` at boot. Legacy import: File › Import Repositories from GitHub Desktop… (flag `206-import-from-github-desktop`) reads the repository paths and aliases out of GHD's IndexedDB LevelDB files under `~/Library/Application Support/GitHub Desktop/` (best effort, read-only; `corvane_platform::ghd_import`) and adds the ones picked; nothing else of GHD's state is imported.

### 3.7 On-demand packs (`corvane-packs`)

Manifest `packs/manifest.json` on GitHub Releases: `{name, version, min_app, url, sha256, size, kind}`. Verified by sha256 + minisign signature (public key embedded). Installed under `~/Library/Application Support/Corvane/packs/<name>/<version>/`.

| Pack | Default build | Full build | Trigger |
|---|---|---|---|
| `syntax-core` (syntect defaults + 330 Linguist TextMate grammars, `assets/syntaxes.packdump`) | bundled | bundled | — |
| `syntax-extended` (two-face full set) | on demand | bundled | first diff with unknown extension → banner "Download extended highlighting (6 MB)?" or Settings › Advanced |
| `tree-sitter-all` (every tree-sitter grammar: one gzipped library per grammar package + `index.json`, per OS + architecture, ~25 MB; units unpack into the cache on first use) | on demand | bundled | Settings › Appearance › Syntax highlighting "Tree-sitter" (Download under the choice) or Settings › Advanced, with `105-tree-sitter-highlighting` |
| `tree-sitter-rest` (grammars of languages GHD does not highlight) | on demand | bundled | "Tree-sitter for other languages" |
| `git-portable` (dugite-native git + lfs) | on demand | bundled | `InstallGit` dialog offers "Download portable git" alongside Xcode CLT/Homebrew |
| `git-lfs` | on demand | bundled | `InitializeLFS` when repo has `.gitattributes` lfs filters and no lfs binary |
| `emoji` | bundled (small) | bundled | — |

Cargo features `bundled-syntax-extended`, `bundled-tree-sitter`, `bundled-git` select the full build; CI publishes `Corvane` and `Corvane-Full`. Pack UI lives inside Settings › Advanced (no new tab, keeps GHD tab list intact).

### 3.8 Packaging / updates / release

- `cargo build --release` → `packaging/bundle.sh` assembles `Corvane.app` (Info.plist, `.icns`, `LSMinimumSystemVersion 15.0`, `NSHighResolutionCapable`, URL scheme reserved `x-corvane-auth` for future web-flow), ad-hoc `codesign -s - --deep`, then `Corvane-<ver>-macos-universal.zip` (+ `.dmg` via `hdiutil`). `minisign` signature (`.minisig`) published next to each asset; public key embedded in the binary.
- Self-updater (`corvane-platform::updater`, M7): check `GET /repos/wasi-master/corvane/releases/latest` every 4 h + at launch (+jitter); banner "Corvane <ver> is available" → download zip to `~/Library/Caches/Corvane/updates/` with `ureq` (files the app writes itself carry no quarantine flag, so ad-hoc signing keeps working) → verify minisign → unzip → rename running bundle to `Corvane.app.old`, move new bundle in, `open -n` new bundle, exit; old bundle removed on next launch. Skipped when the bundle lives under `/opt/homebrew/Caskroom` (banner says "run `brew upgrade corvane`").
- Homebrew tap `wasi-master/homebrew-corvane`, cask pointing at the zip; README documents `brew install --cask wasi-master/corvane/corvane --no-quarantine` as the primary path and Privacy & Security → Open Anyway for direct downloads (macOS 15 removed the right-click → Open bypass).
- CI (GitHub Actions, `macos-15` runner): fmt, clippy `-D warnings`, tests, universal binary via `lipo`, bundle script, minisign (secret key in repo secrets), upload to GitHub Releases on tag. No Developer ID steps.

### 3.9 Feature flags (`corvane_core::flags`)

- Every switchable deviation from GHD and every Corvane-only extra is a flag in `crates/corvane-core/src/flags/registry.rs`: numeric id in a category block (100 Appearance, 200 Repository, 300 GitHub, 400 Window & menus, 500 Settings & updates, 600 Accessibility, 900 Experimental) + slug (`201-commit-templates`), kind (toggle / select / number / text), a value per preset, restart flag, `desktop/desktop` upstream refs. "on" = the deviation is active.
- Resolution: preset base (**GitHub Desktop** · **Familiar** · **Corvane**, the default · **Everything**) → stored overrides (redb key `flags`) → `CORVANE_FLAGS` for one session (locks). `AppState.flags` is the snapshot every view reads; `Dispatcher::update_flags` is the single write path. Lower crates take parameters, never read flags.
- UI: Corvane › Flags… (⌘⇧,), a chrome://flags-style dialog (`crates/corvane-ui/src/dialogs/flags.rs`); `docs/reference/flags.md` is generated from the registry (`UPDATE_FLAGS_DOC=1 cargo test -p corvane-core flags_doc`). The parity harness runs with `preset=github-desktop`.

## 4. Milestones

Each milestone ends with: tests green, `cargo clippy -D warnings`, screenshot comparison against GHD for the touched views, RSS/idle CPU numbers recorded in `docs/perf.md`.

| # | Weeks | Deliverable | Done when |
|---|---|---|---|
| M0 Skeleton | 1 | Workspace, CI, gpui-kit window with GHD toolbar/tab bar/empty sidebar/blankslate, theme tokens, native menus (disabled items), keymap, redb settings, tracing | Chrome pixel-matches GHD dark+light at 1367×814; cold start < 300 ms; idle CPU 0 % |
| M1 Repos + sign-in | 2 | Repository foldout (groups, filter, Recent, Add ▾), Add Local / Create / Clone dialogs (clone progress), git detection + `InstallGit`, Welcome flow, device-flow sign-in, Keychain, avatar cache | Can clone from GitHub.com tab and switch repos |
| M2 Changes | 2 | gix status, changes list, filter, include checkboxes, diff viewer (unified + highlighting), hunk/line staging, discard/ignore, commit form (summary, description, co-authors, amend, undo, options), watcher + focus refresh, oversized/binary handling | Partial commit round-trips byte-identical to GHD's `git apply` patch |
| M3 History | 2 | Commit list, multi-select, commit details, file list + diff, compare-to-branch, context menu (revert, cherry-pick, create branch/tag, checkout, reset, copy SHA, view on GitHub), tags | 100k-commit repo scrolls at display refresh rate; RSS < 120 MB |
| M4 Branches + ops | 2 | Branch foldout (filter, Recent/Default/Other, New Branch, merge row), create/rename/delete, checkout w/ stash strategy prompts, MultiCommitOperation (merge, squash, rebase, cherry-pick, reorder) with progress/conflict dialogs, banners, stash view/restore/discard, drag commit → branch | All GHD Branch menu items functional |
| M5 Remote | 1 | Fetch/pull/push/publish/force-push, ahead/behind badges, background fetch + indicator updater, askpass helper, generic auth dialog, LFS init, push-needs-pull / upstream-exists flows | Push to private GitHub repo over HTTPS with Keychain token; SSH untouched |
| M6 Settings + polish | 1 | Settings (Accounts, Integrations, Git, Appearance, Notifications, Prompts, Advanced, Accessibility; Copilot tab omitted), Repository Settings (Remote, Ignored Files, Git Config), editor/shell detection (VS Code, Zed, Sublime, Cursor, Terminal, iTerm2, Warp, Ghostty…), zoom levels, full VoiceOver pass | Every menu item in `ghd-ui-inventory.md` §(b) enabled or in `TODO.md` |
| M7 Ship | 1 | Bundle script + ad-hoc codesign + zip/dmg, minisign, Homebrew cask, self-updater (check → download → verify → swap → relaunch), packs manifest + downloader, full build variant, perf pass, `docs/perf.md`, v0.1.0 release | `brew install --cask wasi-master/corvane/corvane --no-quarantine` works; in-app update 0.1.0 → 0.1.1 swaps the bundle and relaunches |

Total ≈ 12 weeks for one developer. Windows/Linux, GitHub layer: see `TODO.md`.

## 5. Performance budgets

| Metric | Target | Method |
|---|---|---|
| Cold start to first frame | < 300 ms | `tracing` span + `hyperfine` |
| Idle RSS, 1 repo | ≤ 80 MB | `footprint`/`vmmap` after 60 s idle |
| Idle RSS, 10 repos | ≤ 120 MB | same |
| Idle CPU | 0.0 % (no timers, no redraw) | Activity Monitor 60 s sample |
| `git status` equivalent, 50k files | < 300 ms warm | criterion bench in `corvane-git` |
| Diff open, 5k-line file | < 50 ms | bench |
| Binary size (default build) | ≤ 25 MB | CI check |

## 6. Verification

- Unit: parsers (porcelain v2, `--porcelain` push, progress lines, log format) against fixtures captured from real git; patch formatter snapshot tests (`insta`) ported from GHD's `patch-formatter-test.ts`; theme token loading.
- Integration (`corvane-git/tests`): temp repos built with git CLI (`tempfile`), exercise status/diff/log/commit/branch/stash/rebase/conflict paths end-to-end; run in CI on macOS.
- UI: gpui `test-support` for view state transitions (foldout stack, dialog stack, selection); screenshot regression vs GHD captures in `docs/reference/screens/` (manual approve, `pixelmatch`-style diff script).
- Manual QA checklist per GHD workflow (clone → branch → edit → partial commit → push → merge → conflict → stash) before each release.
- Accessibility: VoiceOver walkthrough of toolbar, lists, commit form, dialogs (AccessKit via GPUI).

## 7. Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | `gpui-pre` weekly snapshots break API | Pin exact `=0.3.7` + `gpui-kit =0.7.0`; upgrade on a branch monthly; vendor via `[patch.crates-io]` if a snapshot vanishes. Vendored today: `vendor/gpui-pre-macos` (0.3.7 + exact variable-font weights in `src/text_system.rs`, see its `exact_weight_variant`); re-apply that diff when upgrading |
| R2 | gpui-kit look leaks into GHD chrome | Own widgets for toolbar/tabs/lists/diff; gpui-kit only for form controls + infra; theme override tested by screenshot diff |
| R3 | gix status slower than git on huge/fsmonitor repos | `status-cli` fallback path; measure in M2; keep both parsers |
| R4 | Commit description editor (autocomplete, IME, spellcheck) | gpui-kit `Textarea` + custom autocomplete popover; spellcheck deferred (`TODO.md`) |
| R5 | No Apple Developer ID (hobby budget): ad-hoc signed app is blocked by Gatekeeper on first launch of a quarantined download; macOS 15 removed the right-click → Open bypass | Homebrew cask `--no-quarantine` documented as primary install; README shows Privacy & Security → Open Anyway and `xattr -d com.apple.quarantine`; self-updater downloads are quarantine-free so updates stay frictionless; minisign guards integrity instead of Apple signing |
| R6 | macOS 15 floor from gpui-kit | Documented; raw-GPUI path could lower to 14 later |
| R7 | GHD updates change UI | Track `desktop/desktop` releases; inventory doc pinned to 3.6.6 |
| R8 | Single-maintainer snapshot crates (supply chain) | `cargo vendor` in release CI; checksums pinned in `Cargo.lock` |

## 8. Next step

Start M0: `cargo new` workspace per §3.1, add `gpui-kit`, open a window with the toolbar, port theme tokens. First commit also adds `NOTICE`, `LICENSE`, CI.
