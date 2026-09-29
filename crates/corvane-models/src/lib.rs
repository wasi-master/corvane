//! Domain models, named after GitHub Desktop's `app/src/models/*`.
//! Leaf crate: no gpui, no git, no network.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A repository known to Corvane (`models/repository.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub path: PathBuf,
    /// User-chosen display name (`ChangeRepositoryAlias`).
    #[serde(default)]
    pub alias: Option<String>,
    /// Set when the `origin` remote points at GitHub.com / GHES.
    #[serde(default)]
    pub github: Option<GitHubRepository>,
    /// The directory disappeared; GHD shows "Can't find" and offers Locate/Remove.
    #[serde(default)]
    pub missing: bool,
    /// Commit form gear menu (`CommitOptions`, persisted per repository).
    #[serde(default)]
    pub commit_options: RepoCommitOptions,
    /// `workflowPreferences.forkContributionTarget` (Repository Settings ›
    /// Fork Behavior); `None` = the GHD default (contribute to the parent).
    #[serde(default)]
    pub fork_contribution_target: Option<ForkContributionTarget>,
}

/// GHD `ICommitOptions`: `skipCommitHooks`, `signOffCommits`, `allowEmptyCommit`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCommitOptions {
    pub skip_commit_hooks: bool,
    pub sign_off_commits: bool,
    pub allow_empty_commit: bool,
}

impl Repository {
    pub fn new(id: u64, path: impl Into<PathBuf>) -> Self {
        Self {
            id,
            path: path.into(),
            alias: None,
            github: None,
            missing: false,
            commit_options: RepoCommitOptions::default(),
            fork_contribution_target: None,
        }
    }

    /// Alias, else GitHub repo name, else the directory name.
    pub fn name(&self) -> String {
        if let Some(alias) = &self.alias {
            return alias.clone();
        }
        if let Some(gh) = &self.github {
            return gh.name.clone();
        }
        dir_name(&self.path)
    }
}

pub fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// A GitHub-hosted repository (`models/github-repository.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRepository {
    /// API base, e.g. `https://api.github.com`.
    pub endpoint: String,
    pub owner: String,
    pub name: String,
    pub html_url: String,
    pub clone_url: String,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    #[serde(default)]
    pub parent: Option<Box<GitHubRepository>>,
    /// Shown as an "Archived" badge in the clone list.
    #[serde(default)]
    pub archived: bool,
}

impl GitHubRepository {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchKind {
    Local,
    Remote,
}

/// `models/branch.ts`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// Short name, e.g. `main` or `origin/main`.
    pub name: String,
    pub kind: BranchKind,
    /// Full ref name, e.g. `refs/heads/main`.
    pub full_name: String,
    /// Commit id the branch points at, if resolvable.
    pub tip: Option<String>,
    /// Upstream tracking branch full name (`refs/remotes/origin/main`), local branches only.
    pub upstream: Option<String>,
    /// Committer time of the tip commit (seconds since the epoch), for the
    /// relative dates in the branch list.
    #[serde(default)]
    pub tip_time: Option<i64>,
}

impl Branch {
    /// `nameWithoutRemote`: `origin/feature` → `feature`.
    pub fn name_without_remote(&self) -> &str {
        match self.kind {
            BranchKind::Local => &self.name,
            BranchKind::Remote => self
                .name
                .split_once('/')
                .map(|(_, n)| n)
                .unwrap_or(&self.name),
        }
    }

    /// `upstreamRemoteName`
    pub fn upstream_remote_name(&self) -> Option<&str> {
        let upstream = self.upstream.as_deref()?;
        let rest = upstream.strip_prefix("refs/remotes/").unwrap_or(upstream);
        rest.split_once('/').map(|(remote, _)| remote)
    }

    /// Short upstream name (`origin/main`).
    pub fn upstream_short(&self) -> Option<&str> {
        self.upstream
            .as_deref()
            .map(|u| u.strip_prefix("refs/remotes/").unwrap_or(u))
    }
}

/// `IStashEntry`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StashEntry {
    /// `stash@{0}`
    pub name: String,
    pub sha: String,
    /// Branch recorded in a GitHub Desktop / Corvane stash message.
    pub branch: Option<String>,
    pub message: String,
    pub tree: String,
    pub parents: Vec<String>,
}

/// `models/tip.ts`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tip {
    Unknown,
    Unborn { name: String },
    Detached { sha: String },
    Valid { branch: Branch },
}

impl Tip {
    pub fn branch_name(&self) -> Option<&str> {
        match self {
            Tip::Valid { branch } => Some(&branch.name),
            Tip::Unborn { name } => Some(name),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
}

/// `models/remote.ts`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

/// A signed-in GitHub account (`models/account.ts`). The token lives in the
/// OS keychain, keyed by `endpoint` + `login`, never in the store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// API base URL, e.g. `https://api.github.com` or `https://ghe.corp/api/v3`.
    pub endpoint: String,
    pub id: u64,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub emails: Vec<String>,
    /// OAuth scopes granted to the token.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// `plan.name` of `/user` (`free`, `pro`, …); `None` until the account
    /// has been read from the API since this field was added.
    #[serde(default)]
    pub plan: Option<String>,
}

impl Account {
    pub fn is_dotcom(&self) -> bool {
        self.endpoint == "https://api.github.com"
    }

    /// Host for keychain keys and remote matching (`github.com`, `ghe.corp`).
    pub fn host(&self) -> String {
        let trimmed = self
            .endpoint
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let host = trimmed.split('/').next().unwrap_or(trimmed);
        if host == "api.github.com" {
            "github.com".to_string()
        } else {
            host.to_string()
        }
    }
}

/// Author identity from git config (`models/commit-identity.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Identity {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// What Corvane knows about an opened repository (subset of GHD's
/// `IRepositoryState`, filled by `corvane-git`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub workdir: PathBuf,
    pub tip: Tip,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub identity: Identity,
    pub ahead_behind: Option<AheadBehind>,
    /// `commit.template` contents with comment lines removed (`None` when
    /// unset, unreadable or empty).
    pub commit_template: Option<String>,
}

impl RepositoryInfo {
    pub fn current_branch(&self) -> Option<&Branch> {
        match &self.tip {
            Tip::Valid { branch } => Some(branch),
            _ => None,
        }
    }

    pub fn remote(&self, name: &str) -> Option<&Remote> {
        self.remotes.iter().find(|r| r.name == name)
    }
}

/// GHD `gitAuthorNameIsValid`: git strips "crud" characters from names and
/// refuses one that consists only of them (`ident.c`). Empty is valid.
pub fn git_author_name_is_valid(name: &str) -> bool {
    !(!name.is_empty()
        && name.chars().all(|c| {
            (c as u32) <= 0x20 || matches!(c, '.' | ',' | ':' | ';' | '<' | '>' | '"' | '\\' | '\'')
        }))
}

/// GHD `InvalidGitAuthorNameMessage`.
pub const INVALID_GIT_AUTHOR_NAME_MESSAGE: &str =
    "Name is invalid, it consists only of disallowed characters.";

/// Parse `owner/name` + host out of a remote URL if it points at GitHub.
/// Handles `https://github.com/o/n(.git)`, `git@github.com:o/n(.git)`,
/// `ssh://git@github.com/o/n` and GHES hosts when `ghes_hosts` lists them.
pub fn github_from_remote(url: &str, ghes_hosts: &[String]) -> Option<GitHubRepository> {
    let (host, path) = split_remote(url)?;
    let is_dotcom = host.eq_ignore_ascii_case("github.com");
    if !is_dotcom && !ghes_hosts.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let name = parts.next()?.trim_end_matches('/').to_string();
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    let endpoint = if is_dotcom {
        "https://api.github.com".to_string()
    } else {
        format!("https://{host}/api/v3")
    };
    Some(GitHubRepository {
        endpoint,
        html_url: format!("https://{host}/{owner}/{name}"),
        clone_url: format!("https://{host}/{owner}/{name}.git"),
        owner,
        name,
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
        archived: false,
    })
}

/// `(host, path)` of a remote URL (`https://`, `http://`, `ssh://`, `git://`
/// or scp-style `git@host:path`); `None` for anything else (GHD `parseRemote`).
pub fn split_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("git@") {
        // git@host:owner/name
        let (host, path) = rest.split_once(':')?;
        return Some((host.to_string(), path.to_string()));
    }
    for scheme in ["https://", "http://", "ssh://", "git://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
            let (host, path) = rest.split_once('/')?;
            let host = host.split(':').next()?.to_string();
            return Some((host, path.to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_remotes() {
        for url in [
            "https://github.com/wasi-master/corvane.git",
            "https://github.com/wasi-master/corvane",
            "git@github.com:wasi-master/corvane.git",
            "ssh://git@github.com/wasi-master/corvane.git",
            "https://user@github.com/wasi-master/corvane/",
        ] {
            let gh = github_from_remote(url, &[]).unwrap_or_else(|| panic!("{url}"));
            assert_eq!(gh.owner, "wasi-master");
            assert_eq!(gh.name, "corvane");
            assert_eq!(gh.html_url, "https://github.com/wasi-master/corvane");
            assert_eq!(gh.endpoint, "https://api.github.com");
        }
    }

    #[test]
    fn ignores_non_github_and_ghes_without_config() {
        assert!(github_from_remote("https://gitlab.com/a/b.git", &[]).is_none());
        assert!(github_from_remote("git@ghe.corp:a/b.git", &[]).is_none());
        let gh = github_from_remote("git@ghe.corp:a/b.git", &["ghe.corp".into()]).unwrap();
        assert_eq!(gh.endpoint, "https://ghe.corp/api/v3");
    }

    #[test]
    fn repository_name_precedence() {
        let mut repo = Repository::new(1, "/tmp/my-dir");
        assert_eq!(repo.name(), "my-dir");
        repo.github = github_from_remote("git@github.com:o/gh-name.git", &[]);
        assert_eq!(repo.name(), "gh-name");
        repo.alias = Some("Alias".into());
        assert_eq!(repo.name(), "Alias");
    }
}

/// Which sidebar section is shown (`RepositorySectionTab`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Section {
    #[default]
    Changes,
    History,
}

/// Settings › Appearance › Theme (`ApplicationTheme`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSetting {
    Light,
    Dark,
    #[default]
    System,
}

// ---- working directory status (`models/status.ts`) ----

/// `AppFileStatusKind`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileStatusKind {
    New,
    Modified,
    Deleted,
    Copied,
    Renamed,
    Conflicted,
    Untracked,
}

impl FileStatusKind {
    /// GHD's status octicon per kind (`ui/octicons/status.ts`).
    pub fn is_new_or_untracked(self) -> bool {
        matches!(self, FileStatusKind::New | FileStatusKind::Untracked)
    }
}

/// Which side of the index a change lives on (porcelain X / Y columns).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitStatusEntry {
    Unchanged,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Unmerged,
    Untracked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStatus {
    pub kind: FileStatusKind,
    pub index: GitStatusEntry,
    pub working_tree: GitStatusEntry,
    /// `R100`/`C90` similarity score for renames and copies.
    pub score: Option<u8>,
    /// Two-letter porcelain code (`.M`, `UU`, `??`), kept for conflict handling.
    pub code: String,
    pub submodule: bool,
    /// Porcelain v2 `S<c><m><u>` flags for submodules (GHD `SubmoduleStatus`).
    #[serde(default)]
    pub submodule_status: Option<SubmoduleStatus>,
    /// Conflicted text files: number of leftover `<<<<<<<`/`=======`/`>>>>>>>`
    /// markers (0 once resolved in an editor). `None` for binary / delete
    /// conflicts that need a manual "use ours / theirs" choice
    /// (GHD `ConflictsWithMarkers` vs `ManualConflict`).
    #[serde(default)]
    pub conflict_markers: Option<u32>,
}

impl FileStatus {
    /// GHD `isConflictWithMarkers`
    pub fn is_text_conflict(&self) -> bool {
        self.kind == FileStatusKind::Conflicted && self.conflict_markers.is_some()
    }

    /// GHD `isManualConflict`
    pub fn is_manual_conflict(&self) -> bool {
        self.kind == FileStatusKind::Conflicted && self.conflict_markers.is_none()
    }

    /// GHD `hasUnresolvedConflicts`: a manual choice resolves anything; text
    /// conflicts are resolved once no markers remain; binary conflicts never
    /// resolve on their own.
    pub fn has_unresolved_conflicts(&self, resolution: Option<ManualConflictResolution>) -> bool {
        if resolution.is_some() {
            return false;
        }
        match self.conflict_markers {
            Some(count) => count > 0,
            None => true,
        }
    }

    /// GHD `status.entry.us` for unmerged entries (`u XY`: X is our side).
    pub fn us(&self) -> GitStatusEntry {
        self.index
    }

    /// GHD `status.entry.them` (Y is their side).
    pub fn them(&self) -> GitStatusEntry {
        self.working_tree
    }
}

/// GHD `ManualConflictResolution`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManualConflictResolution {
    Ours,
    Theirs,
}

/// GHD `CommitOneLine`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitOneLine {
    pub sha: String,
    pub summary: String,
}

/// GHD `MultiCommitOperationKind`; `label()` is the capitalised user-facing name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiCommitOperationKind {
    Rebase,
    CherryPick,
    Squash,
    Merge,
    Reorder,
}

impl MultiCommitOperationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rebase => "Rebase",
            Self::CherryPick => "Cherry-pick",
            Self::Squash => "Squash",
            Self::Merge => "Merge",
            Self::Reorder => "Reorder",
        }
    }

    pub fn lower(self) -> &'static str {
        match self {
            Self::Rebase => "rebase",
            Self::CherryPick => "cherry-pick",
            Self::Squash => "squash",
            Self::Merge => "merge",
            Self::Reorder => "reorder",
        }
    }
}

/// GHD `IMultiCommitOperationProgress`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct McoProgress {
    /// 0..=1
    pub value: f32,
    /// 1-based index of the commit being applied.
    pub position: usize,
    pub total: usize,
    pub current_summary: String,
}

/// GHD `RebaseInternalState` (`.git/rebase-merge/{head-name,onto,orig-head}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebaseInternalState {
    /// The branch being rebased (`head-name` without `refs/heads/`).
    pub target_branch: String,
    /// `onto`: the commit the branch is replayed on top of.
    pub base_branch_tip: String,
    /// `orig-head`: the branch tip before the rebase started.
    pub original_branch_tip: String,
}

/// GHD `MergeTreeResult` / `ComputedAction` for a would-be merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mergeability {
    Clean,
    Conflicts(u32),
    /// Unrelated histories.
    Invalid,
}

/// How much of a file is included in the next commit (`DiffSelectionType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffSelectionType {
    All,
    Partial,
    None,
}

/// GHD `DiffSelection`: a default state (all or none) plus the lines that
/// diverge from it. `selectable` bounds what "all" means once a diff is
/// loaded. Line indices are absolute positions in the unified diff (hunk
/// header lines included), i.e. `hunk.unified_diff_start + index`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSelection {
    default_all: bool,
    diverging: BTreeSet<u32>,
    selectable: Option<BTreeSet<u32>>,
}

impl Default for DiffSelection {
    fn default() -> Self {
        Self::all()
    }
}

impl DiffSelection {
    pub fn all() -> Self {
        Self {
            default_all: true,
            diverging: BTreeSet::new(),
            selectable: None,
        }
    }

    pub fn none() -> Self {
        Self {
            default_all: false,
            diverging: BTreeSet::new(),
            selectable: None,
        }
    }

    fn default_kind(&self) -> DiffSelectionType {
        if self.default_all {
            DiffSelectionType::All
        } else {
            DiffSelectionType::None
        }
    }

    fn inverse_kind(&self) -> DiffSelectionType {
        if self.default_all {
            DiffSelectionType::None
        } else {
            DiffSelectionType::All
        }
    }

    /// `getSelectionType`
    pub fn kind(&self) -> DiffSelectionType {
        if self.diverging.is_empty() {
            return self.default_kind();
        }
        match &self.selectable {
            None => DiffSelectionType::Partial,
            Some(selectable) => {
                if selectable.len() == self.diverging.len()
                    && selectable.iter().all(|l| self.diverging.contains(l))
                {
                    self.inverse_kind()
                } else {
                    DiffSelectionType::Partial
                }
            }
        }
    }

    pub fn is_selected(&self, line: u32) -> bool {
        let diverges = self.diverging.contains(&line);
        if self.default_all {
            !diverges
        } else {
            diverges
        }
    }

    pub fn is_selectable(&self, line: u32) -> bool {
        self.selectable.as_ref().is_none_or(|s| s.contains(&line))
    }

    /// `isRangeSelected`: All / None / Partial for `len` lines from `from`.
    pub fn range_kind(&self, from: u32, len: u32) -> DiffSelectionType {
        if len == 0 {
            return DiffSelectionType::None;
        }
        let kind = self.kind();
        if kind != DiffSelectionType::Partial {
            return kind;
        }
        let first = self.is_selected(from);
        for line in from + 1..from + len {
            if self.is_selected(line) != first {
                return DiffSelectionType::Partial;
            }
        }
        if first {
            DiffSelectionType::All
        } else {
            DiffSelectionType::None
        }
    }

    pub fn with_line(&self, line: u32, selected: bool) -> Self {
        self.with_range(line, 1, selected)
    }

    /// `withRangeSelection`: mark `len` lines from `from` (only selectable ones).
    pub fn with_range(&self, from: u32, len: u32, selected: bool) -> Self {
        let kind = self.kind();
        let already = (kind == DiffSelectionType::All && selected)
            || (kind == DiffSelectionType::None && !selected);
        if already {
            return self.clone();
        }
        if kind == DiffSelectionType::Partial {
            let mut diverging = self.diverging.clone();
            let matches_default = self.default_all == selected;
            for line in from..from + len {
                if matches_default {
                    diverging.remove(&line);
                } else if self.is_selectable(line) {
                    diverging.insert(line);
                }
            }
            Self {
                default_all: self.default_all,
                diverging,
                selectable: self.selectable.clone(),
            }
        } else {
            // Re-base on the computed state so "all lines flipped" collapses.
            let diverging = (from..from + len)
                .filter(|l| self.is_selectable(*l))
                .collect();
            Self {
                default_all: kind == DiffSelectionType::All,
                diverging,
                selectable: self.selectable.clone(),
            }
        }
    }

    pub fn toggled(&self, line: u32) -> Self {
        self.with_line(line, !self.is_selected(line))
    }

    pub fn select_all(&self) -> Self {
        Self {
            default_all: true,
            diverging: BTreeSet::new(),
            selectable: self.selectable.clone(),
        }
    }

    pub fn select_none(&self) -> Self {
        Self {
            default_all: false,
            diverging: BTreeSet::new(),
            selectable: self.selectable.clone(),
        }
    }

    /// `withSelectableLines`: drops diverging lines that no longer exist.
    pub fn with_selectable_lines(&self, selectable: BTreeSet<u32>) -> Self {
        let diverging = self
            .diverging
            .iter()
            .copied()
            .filter(|l| selectable.contains(l))
            .collect();
        Self {
            default_all: self.default_all,
            diverging,
            selectable: Some(selectable),
        }
    }
}

/// `WorkingDirectoryFileChange`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingDirectoryFileChange {
    /// Repository-relative path (new path for renames).
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub selection: DiffSelection,
}

impl WorkingDirectoryFileChange {
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    pub fn directory(&self) -> &str {
        match self.path.rfind('/') {
            Some(ix) => &self.path[..=ix],
            None => "",
        }
    }
}

/// `WorkingDirectoryStatus`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkingDirectoryStatus {
    pub files: Vec<WorkingDirectoryFileChange>,
    /// Branch header info from `# branch.*` (None when detached/unborn).
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead_behind: Option<AheadBehind>,
    pub merge_head_found: bool,
    pub rebase_in_progress: bool,
    /// `# branch.oid`
    #[serde(default)]
    pub current_tip: Option<String>,
    /// `.git/CHERRY_PICK_HEAD` exists (`isCherryPickingHeadFound`).
    #[serde(default)]
    pub cherry_pick_head_found: bool,
    /// `.git/SQUASH_MSG` exists (a `merge --squash` that has not been committed).
    #[serde(default)]
    pub squash_msg_found: bool,
    #[serde(default)]
    pub rebase_internal_state: Option<RebaseInternalState>,
}

impl WorkingDirectoryStatus {
    /// `includeAll` tri-state: Some(true) all, Some(false) none, None mixed.
    pub fn include_all(&self) -> Option<bool> {
        if self.files.is_empty() {
            return Some(true);
        }
        let all = self
            .files
            .iter()
            .all(|f| f.selection.kind() == DiffSelectionType::All);
        let none = self
            .files
            .iter()
            .all(|f| f.selection.kind() == DiffSelectionType::None);
        if all {
            Some(true)
        } else if none {
            Some(false)
        } else {
            None
        }
    }

    pub fn has_conflicts(&self) -> bool {
        self.files
            .iter()
            .any(|f| f.status.kind == FileStatusKind::Conflicted)
    }
}

// ---- history (`models/commit.ts`, `IChangesetData`) ----

/// `CommitIdentity`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitIdentity {
    pub name: String,
    pub email: String,
    /// Seconds since the Unix epoch (author/committer time).
    pub seconds: i64,
    /// Time zone offset in seconds east of UTC.
    pub offset: i32,
}

impl CommitIdentity {
    pub fn date(&self) -> std::time::SystemTime {
        if self.seconds >= 0 {
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(self.seconds as u64)
        } else {
            std::time::UNIX_EPOCH
        }
    }
}

/// `Commit` (trimmed to what the history views need).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub summary: String,
    pub body: String,
    pub author: CommitIdentity,
    pub committer: CommitIdentity,
    pub parents: Vec<String>,
    pub tags: Vec<String>,
}

impl Commit {
    pub fn short_sha(&self) -> &str {
        &self.sha[..self.sha.len().min(7)]
    }

    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// `CommittedFileChange`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommittedFileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub commitish: String,
}

/// `IChangesetData`: files plus line counts for one commit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangesetData {
    pub files: Vec<CommittedFileChange>,
    pub lines_added: u64,
    pub lines_deleted: u64,
}

impl CommittedFileChange {
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    pub fn directory(&self) -> &str {
        match self.path.rfind('/') {
            Some(i) => &self.path[..=i],
            None => "",
        }
    }
}

// ---- diffs (`models/diff/*`) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineKind {
    Context,
    Add,
    Delete,
    Hunk,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// Line text without the leading marker.
    pub text: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    /// `\ No newline at end of file` followed this line.
    pub no_trailing_newline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Absolute index of this hunk's header line in the unified diff.
    #[serde(default)]
    pub unified_diff_start: u32,
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

/// GHD `UnknownAuthor.state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnknownAuthorState {
    Searching,
    Error,
}

/// GHD `Author` (`models/author.ts`): a co-author handle in the commit form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Author {
    Known {
        name: String,
        email: String,
        username: Option<String>,
    },
    Unknown {
        username: String,
        state: UnknownAuthorState,
    },
}

impl Author {
    pub fn username(&self) -> Option<&str> {
        match self {
            Author::Known { username, .. } => username.as_deref(),
            Author::Unknown { username, .. } => Some(username),
        }
    }

    /// GHD `getDisplayTextForAuthor`: `@login`, or the name without a login.
    pub fn display_text(&self) -> String {
        match self {
            Author::Known {
                name,
                username: None,
                ..
            } => name.clone(),
            Author::Known {
                username: Some(u), ..
            }
            | Author::Unknown { username: u, .. } => format!("@{u}"),
        }
    }

    /// GHD `getFullTextForAuthor`: `@login (Name)`.
    pub fn full_text(&self) -> String {
        match self {
            Author::Known {
                name,
                username: Some(u),
                ..
            } => format!("@{u} ({name})"),
            _ => self.display_text(),
        }
    }

    /// The `Co-Authored-By` trailer value of a known author.
    pub fn trailer_value(&self) -> Option<String> {
        match self {
            Author::Known { name, email, .. } => Some(format!("{name} <{email}>")),
            Author::Unknown { .. } => None,
        }
    }
}

/// GHD `getLegacyStealthEmailForUser`: the no-reply address of a login.
pub fn legacy_stealth_email(login: &str, endpoint: &str) -> String {
    let host = if endpoint == "https://api.github.com" {
        "github.com".to_string()
    } else {
        endpoint
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("github.com")
            .to_string()
    };
    format!("{login}@users.noreply.{host}")
}

/// GHD `getStealthEmailForUser`: `<id>+<login>@users.noreply.<host>`.
pub fn stealth_email(id: u64, login: &str, endpoint: &str) -> String {
    format!("{id}+{}", legacy_stealth_email(login, endpoint))
}

/// GHD `WorktreeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorktreeType {
    Main,
    Linked,
}

/// GHD `WorktreeEntry` (`git worktree list --porcelain`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub head: String,
    /// Full ref (`refs/heads/main`) when checked out on a branch.
    pub branch: Option<String>,
    pub is_detached: bool,
    pub kind: WorktreeType,
    pub is_locked: bool,
    pub is_prunable: bool,
}

impl WorktreeEntry {
    /// GHD `getWorktreeDisplayName`: the folder name.
    pub fn display_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    /// GHD `getWorktreeDescription`: the branch, else the short SHA.
    pub fn description(&self) -> String {
        match &self.branch {
            Some(b) => b.strip_prefix("refs/heads/").unwrap_or(b).to_string(),
            None => self.head.chars().take(7).collect(),
        }
    }
}

/// GHD `SubmoduleStatus`: what changed inside a submodule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmoduleStatus {
    pub commit_changed: bool,
    pub modified_changes: bool,
    pub untracked_changes: bool,
}

/// GHD `Image`: the bytes of one side of an image diff.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageBlob {
    pub bytes: Vec<u8>,
    /// `image/png`, `image/jpg`, …
    pub media_type: String,
}

/// GHD `ISubmoduleDiff`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmoduleDiff {
    pub path: String,
    pub full_path: PathBuf,
    /// `submodule.<path>.url` from the repository config.
    pub url: Option<String>,
    pub old_sha: Option<String>,
    pub new_sha: Option<String>,
    pub status: SubmoduleStatus,
}

/// GHD `ImageDiffType` (the tabs of a modified-image diff).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ImageDiffType {
    #[default]
    TwoUp,
    Swipe,
    OnionSkin,
    Difference,
}

/// GHD `imageFileExtensions` + `getMediaType`: `Some` when a binary file
/// can be shown as an image.
pub fn image_media_type(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => return None,
    })
}

/// GHD `LineEndingsChange`: git's "CRLF will be replaced by LF" warning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineEndingsChange {
    pub from: String,
    pub to: String,
}

/// Warnings shown above a text diff (`DiffContentsWarning`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffWarnings {
    /// GHD `hasHiddenBidiChars`: U+202A–U+202E / U+2066–U+2069 in the text.
    pub hidden_bidi: bool,
    pub line_endings: Option<LineEndingsChange>,
}

/// GHD `IDiff` (`DiffType`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Diff {
    Text {
        hunks: Vec<DiffHunk>,
        #[serde(default)]
        warnings: DiffWarnings,
    },
    /// `LargeText`: shown only after "Show Diff" (performance).
    LargeText {
        hunks: Vec<DiffHunk>,
        #[serde(default)]
        warnings: DiffWarnings,
    },
    Binary,
    /// `Image`: `previous` is missing for new files, `current` for deleted ones.
    Image {
        previous: Option<ImageBlob>,
        current: Option<ImageBlob>,
    },
    /// Nothing to show (e.g. empty file, mode-only change).
    Empty,
    /// `Unrenderable`: beyond what git would even hand over.
    TooLarge,
    Submodule(SubmoduleDiff),
}

impl Diff {
    pub fn line_count(&self) -> usize {
        match self {
            Diff::Text { hunks, .. } | Diff::LargeText { hunks, .. } => {
                hunks.iter().map(|h| h.lines.len() + 1).sum()
            }
            _ => 0,
        }
    }

    /// The hunks of a text diff (large ones included).
    pub fn hunks(&self) -> Option<&[DiffHunk]> {
        match self {
            Diff::Text { hunks, .. } | Diff::LargeText { hunks, .. } => Some(hunks),
            _ => None,
        }
    }

    /// The warnings of a text diff.
    pub fn warnings(&self) -> Option<&DiffWarnings> {
        match self {
            Diff::Text { warnings, .. } | Diff::LargeText { warnings, .. } => Some(warnings),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Pull requests (`models/pull-request.ts`)
// ---------------------------------------------------------------------------

/// `PullRequestRef`: a ref in a GitHub repository. `repository` is `None`
/// when the head repository was deleted after the pull request was opened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestRef {
    pub ref_name: String,
    pub sha: String,
    pub repository: Option<GitHubRepository>,
}

/// `PullRequest`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    /// ISO-8601 timestamps as the API sends them (`2024-01-31T12:00:00Z`).
    pub created_at: String,
    pub updated_at: String,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
    /// The author's login.
    pub author: String,
    pub draft: bool,
    pub body: String,
}

impl PullRequest {
    /// `getPullRequestCommitRef`: the ref GitHub exposes for the PR head.
    pub fn commit_ref(&self) -> String {
        format!("refs/pull/{}/head", self.number)
    }

    /// `<base html url>/pull/<number>`
    pub fn html_url(&self) -> Option<String> {
        self.base
            .repository
            .as_ref()
            .map(|r| format!("{}/pull/{}", r.html_url, self.number))
    }
}

/// Parse an ISO-8601 UTC timestamp (`2024-01-31T12:00:00Z`, optional
/// fractional seconds) into a `SystemTime`.
pub fn parse_iso8601(value: &str) -> Option<std::time::SystemTime> {
    let value = value.trim().trim_end_matches('Z');
    let (date, time) = value.split_once('T')?;
    let mut d = date.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    let time = time.split(['+', '-']).next()?;
    let mut t = time.split(':');
    let hour: u64 = t.next()?.parse().ok()?;
    let minute: u64 = t.next()?.parse().ok()?;
    let second: u64 = t
        .next()
        .map(|s| s.split('.').next().unwrap_or("0"))
        .unwrap_or("0")
        .parse()
        .ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Howard Hinnant's days-from-civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = (month as u64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe as i64 - 719_468;
    let secs = days * 86_400 + (hour * 3600 + minute * 60 + second) as i64;
    if secs < 0 {
        return None;
    }
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64))
}

/// `urlMatchesRemote` / `repositoryMatchesRemote`: same host and
/// `owner/name`, ignoring scheme, credentials, case and a `.git` suffix.
pub fn url_matches_remote(a: &str, b: &str) -> bool {
    fn key(url: &str) -> Option<(String, String)> {
        let (host, path) = split_remote(url)?;
        let path = path.trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        Some((host.to_lowercase(), path.to_lowercase()))
    }
    match (key(a), key(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Forks (`models/workflow-preferences.ts`)
// ---------------------------------------------------------------------------

/// `ForkContributionTarget`: what the user contributes to with a fork.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForkContributionTarget {
    #[default]
    Parent,
    #[serde(rename = "self")]
    Own,
}

impl Repository {
    /// `getForkContributionTarget`
    pub fn fork_contribution_target(&self) -> ForkContributionTarget {
        self.fork_contribution_target.unwrap_or_default()
    }

    /// `isForkedRepositoryContributingToParent`
    pub fn is_fork_contributing_to_parent(&self) -> bool {
        self.github.as_ref().is_some_and(|gh| gh.parent.is_some())
            && self.fork_contribution_target() == ForkContributionTarget::Parent
    }

    /// `getNonForkGitHubRepository`: the parent when this fork contributes
    /// to it, else the repository itself.
    pub fn non_fork_github(&self) -> Option<&GitHubRepository> {
        let gh = self.github.as_ref()?;
        match (&gh.parent, self.fork_contribution_target()) {
            (Some(parent), ForkContributionTarget::Parent) => Some(parent),
            _ => Some(gh),
        }
    }
}

// ---------------------------------------------------------------------------
// CI checks (`lib/ci-checks/ci-checks.ts`, `lib/api.ts`)
// ---------------------------------------------------------------------------

/// `APICheckStatus`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Queued,
    InProgress,
    Completed,
    /// Anything else the API sends (`waiting`, `requested`, `pending`).
    #[serde(other)]
    Pending,
}

/// `APICheckConclusion`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckConclusion {
    ActionRequired,
    Cancelled,
    TimedOut,
    Failure,
    Neutral,
    Success,
    Skipped,
    Stale,
    #[serde(other)]
    Unknown,
}

impl CheckConclusion {
    /// `getCheckRunConclusionAdjective`
    pub fn adjective(conclusion: Option<CheckConclusion>) -> &'static str {
        match conclusion {
            None => "In progress",
            Some(CheckConclusion::ActionRequired) => "Action required",
            Some(CheckConclusion::Cancelled) => "Canceled",
            Some(CheckConclusion::TimedOut) => "Timed out",
            Some(CheckConclusion::Failure) => "Failed",
            Some(CheckConclusion::Neutral) | Some(CheckConclusion::Unknown) => "Neutral",
            Some(CheckConclusion::Success) => "Successful",
            Some(CheckConclusion::Skipped) => "Skipped",
            Some(CheckConclusion::Stale) => "Marked as stale",
        }
    }

    /// `FailingCheckConclusions`
    pub fn is_failing(self) -> bool {
        matches!(
            self,
            CheckConclusion::Failure
                | CheckConclusion::Cancelled
                | CheckConclusion::ActionRequired
                | CheckConclusion::TimedOut
        )
    }
}

/// `IAPIWorkflowRun` (the fields the check-run list needs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub workflow_id: u64,
    pub name: String,
    #[serde(default)]
    pub event: String,
    pub check_suite_id: Option<u64>,
    pub created_at: String,
}

/// `IAPIWorkflowJobStep`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobStep {
    pub name: String,
    pub number: u64,
    pub status: CheckStatus,
    pub conclusion: Option<CheckConclusion>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// `IRefCheck`: one status or check run of a ref.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefCheck {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub status: CheckStatus,
    pub conclusion: Option<CheckConclusion>,
    pub app_name: String,
    pub html_url: Option<String>,
    pub check_suite_id: Option<u64>,
    pub actions_workflow: Option<WorkflowRun>,
    pub job_steps: Option<Vec<JobStep>>,
}

/// `ICombinedRefCheck`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombinedRefCheck {
    pub status: CheckStatus,
    pub conclusion: Option<CheckConclusion>,
    pub checks: Vec<RefCheck>,
}

impl RefCheck {
    /// `isIncomplete`
    pub fn is_incomplete(&self) -> bool {
        self.status == CheckStatus::Completed
            && matches!(
                self.conclusion,
                Some(CheckConclusion::TimedOut)
                    | Some(CheckConclusion::Stale)
                    | Some(CheckConclusion::Cancelled)
            )
    }

    /// `isFailure`
    pub fn is_failure(&self) -> bool {
        self.status == CheckStatus::Completed
            && matches!(
                self.conclusion,
                Some(CheckConclusion::Failure) | Some(CheckConclusion::ActionRequired)
            )
    }

    /// `isSuccess`
    pub fn is_success(&self) -> bool {
        self.status == CheckStatus::Completed
            && matches!(
                self.conclusion,
                Some(CheckConclusion::Success)
                    | Some(CheckConclusion::Neutral)
                    | Some(CheckConclusion::Skipped)
            )
    }
}

impl CombinedRefCheck {
    /// `createCombinedCheckFromChecks`
    pub fn from_checks(checks: Vec<RefCheck>) -> Option<Self> {
        if checks.is_empty() {
            return None;
        }
        if checks.len() == 1 {
            let (status, conclusion) = (checks[0].status, checks[0].conclusion);
            return Some(Self {
                status,
                conclusion,
                checks,
            });
        }
        if checks.iter().any(|c| c.is_incomplete() || c.is_failure()) {
            Some(Self {
                status: CheckStatus::Completed,
                conclusion: Some(CheckConclusion::Failure),
                checks,
            })
        } else if checks.iter().all(RefCheck::is_success) {
            Some(Self {
                status: CheckStatus::Completed,
                conclusion: Some(CheckConclusion::Success),
                checks,
            })
        } else {
            Some(Self {
                status: CheckStatus::InProgress,
                conclusion: None,
                checks,
            })
        }
    }
}

/// `formatPreciseDuration`: `1h 2m 3s`.
pub fn format_precise_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let (d, h, m, s) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60, secs % 60);
    let mut parts = Vec::new();
    if d > 0 {
        parts.push(format!("{d}d"));
    }
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    if s > 0 || parts.is_empty() {
        parts.push(format!("{s}s"));
    }
    parts.join(" ")
}

/// `getCheckDurationInMilliseconds`
pub fn check_duration_ms(started_at: Option<&str>, completed_at: Option<&str>) -> Option<u64> {
    let start = parse_iso8601(started_at?)?;
    let end = parse_iso8601(completed_at?)?;
    end.duration_since(start).ok().map(|d| d.as_millis() as u64)
}

/// `getCheckRunShortDescription`
pub fn check_short_description(
    status: CheckStatus,
    conclusion: Option<CheckConclusion>,
    duration_ms: Option<u64>,
) -> String {
    let Some(conclusion) = conclusion.filter(|_| status == CheckStatus::Completed) else {
        return "In progress".to_string();
    };
    let adjective = CheckConclusion::adjective(Some(conclusion));
    if matches!(
        conclusion,
        CheckConclusion::ActionRequired | CheckConclusion::Skipped | CheckConclusion::Stale
    ) {
        return adjective.to_string();
    }
    let preposition = if conclusion == CheckConclusion::Success {
        "in"
    } else {
        "after"
    };
    match duration_ms {
        Some(ms) if ms > 0 => format!("{adjective} {preposition} {}", format_precise_duration(ms)),
        _ => adjective.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Repository rules (`models/repo-rules.ts`)
// ---------------------------------------------------------------------------

/// `RepoRuleEnforced`: `false` | `true` | `'bypass'`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoRuleEnforced {
    #[default]
    No,
    Yes,
    /// The rule applies but the current user may bypass it.
    Bypass,
}

impl RepoRuleEnforced {
    /// `info.x !== true ? enforced : true`: once enforced, stays enforced.
    pub fn combine(self, other: RepoRuleEnforced) -> RepoRuleEnforced {
        if self == RepoRuleEnforced::Yes {
            self
        } else {
            other
        }
    }
}

/// `APIRepoRuleMetadataOperator`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOperator {
    StartsWith,
    EndsWith,
    Contains,
    #[serde(rename = "regex")]
    RegexMatch,
}

/// `IRepoRulesMetadataRule` (the matcher is built from the pattern in core).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRulesMetadataRule {
    pub enforced: RepoRuleEnforced,
    pub ruleset_id: u64,
    pub operator: RuleOperator,
    pub pattern: String,
    pub negate: bool,
}

impl RepoRulesMetadataRule {
    /// `toHumanDescription`: `must not start with "foo"`.
    pub fn human_description(&self) -> String {
        let mut description = String::from("must ");
        if self.negate {
            description.push_str("not ");
        }
        match self.operator {
            RuleOperator::RegexMatch => {
                description.push_str(&format!(
                    "match the regular expression \"{}\"",
                    self.pattern
                ));
            }
            RuleOperator::StartsWith => {
                description.push_str(&format!("start with \"{}\"", self.pattern))
            }
            RuleOperator::EndsWith => {
                description.push_str(&format!("end with \"{}\"", self.pattern))
            }
            RuleOperator::Contains => {
                description.push_str(&format!("contain \"{}\"", self.pattern))
            }
        }
        description
    }
}

/// `RepoRulesMetadataFailure`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRulesMetadataFailure {
    pub description: String,
    pub ruleset_id: u64,
}

/// `RepoRulesMetadataStatus`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoRulesMetadataStatus {
    Pass,
    Fail,
    Bypass,
}

/// `RepoRulesMetadataFailures`
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoRulesMetadataFailures {
    pub failed: Vec<RepoRulesMetadataFailure>,
    pub bypassed: Vec<RepoRulesMetadataFailure>,
}

impl RepoRulesMetadataFailures {
    pub fn status(&self) -> RepoRulesMetadataStatus {
        if !self.failed.is_empty() {
            RepoRulesMetadataStatus::Fail
        } else if !self.bypassed.is_empty() {
            RepoRulesMetadataStatus::Bypass
        } else {
            RepoRulesMetadataStatus::Pass
        }
    }

    pub fn total(&self) -> usize {
        self.failed.len() + self.bypassed.len()
    }
}

/// `RepoRulesInfo`: what the rulesets of the current branch enforce.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoRulesInfo {
    /// `update` / `required_deployments` / `required_status_checks`.
    pub basic_commit_warning: RepoRuleEnforced,
    pub creation_restricted: RepoRuleEnforced,
    pub signed_commits_required: RepoRuleEnforced,
    pub pull_request_required: RepoRuleEnforced,
    pub commit_message_patterns: Vec<RepoRulesMetadataRule>,
    pub commit_author_email_patterns: Vec<RepoRulesMetadataRule>,
    pub committer_email_patterns: Vec<RepoRulesMetadataRule>,
    pub branch_name_patterns: Vec<RepoRulesMetadataRule>,
}

// ---------------------------------------------------------------------------
// Secret scanning push protection (`ui/secret-scanning/`)
// ---------------------------------------------------------------------------

/// `ISecretLocation`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretLocation {
    pub commit_sha: String,
    pub path: String,
    pub line_number: u64,
}

/// `ISecretScanResult`: one secret the server refused to accept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretScanResult {
    /// The placeholder id at the end of the bypass URL.
    pub id: String,
    pub description: String,
    pub locations: Vec<SecretLocation>,
    pub bypass_url: String,
    /// Bypassing needs an admin's approval ("request an exemption").
    pub requires_approval: bool,
}

/// `BypassReason`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BypassReason {
    FalsePositive,
    UsedInTests,
    WillFixLater,
}

impl BypassReason {
    pub fn as_str(self) -> &'static str {
        match self {
            BypassReason::FalsePositive => "false_positive",
            BypassReason::UsedInTests => "used_in_tests",
            BypassReason::WillFixLater => "will_fix_later",
        }
    }
}

#[cfg(test)]
mod github_layer_tests {
    use super::*;

    #[test]
    fn parses_iso_timestamps() {
        let t = parse_iso8601("1970-01-02T00:00:00Z").unwrap();
        assert_eq!(
            t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            86_400
        );
        let t = parse_iso8601("2024-03-01T12:30:15.5Z").unwrap();
        assert_eq!(
            t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            1_709_296_215
        );
        assert!(parse_iso8601("nope").is_none());
    }

    #[test]
    fn matches_remote_urls() {
        assert!(url_matches_remote(
            "https://github.com/Octocat/Hello-World.git",
            "git@github.com:octocat/hello-world"
        ));
        assert!(!url_matches_remote(
            "https://github.com/octocat/hello-world",
            "https://github.com/octocat/other"
        ));
    }

    #[test]
    fn combines_checks() {
        let check = |conclusion| RefCheck {
            id: 1,
            name: "ci".into(),
            description: String::new(),
            status: CheckStatus::Completed,
            conclusion,
            app_name: String::new(),
            html_url: None,
            check_suite_id: None,
            actions_workflow: None,
            job_steps: None,
        };
        let combined = CombinedRefCheck::from_checks(vec![
            check(Some(CheckConclusion::Success)),
            check(Some(CheckConclusion::Skipped)),
        ])
        .unwrap();
        assert_eq!(combined.conclusion, Some(CheckConclusion::Success));
        let combined = CombinedRefCheck::from_checks(vec![
            check(Some(CheckConclusion::Success)),
            check(Some(CheckConclusion::Cancelled)),
        ])
        .unwrap();
        assert_eq!(combined.conclusion, Some(CheckConclusion::Failure));
        assert!(CombinedRefCheck::from_checks(vec![]).is_none());
        assert_eq!(
            check_short_description(
                CheckStatus::Completed,
                Some(CheckConclusion::Failure),
                Some(65_000)
            ),
            "Failed after 1m 5s"
        );
    }

    #[test]
    fn describes_rules() {
        let rule = RepoRulesMetadataRule {
            enforced: RepoRuleEnforced::Yes,
            ruleset_id: 1,
            operator: RuleOperator::StartsWith,
            pattern: "feat".into(),
            negate: true,
        };
        assert_eq!(rule.human_description(), "must not start with \"feat\"");
    }
}
