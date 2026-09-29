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

fn split_remote(url: &str) -> Option<(String, String)> {
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

/// GHD `IDiff` (`DiffType`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Diff {
    Text {
        hunks: Vec<DiffHunk>,
    },
    /// `LargeText`: shown only after "Show Diff" (performance).
    LargeText {
        hunks: Vec<DiffHunk>,
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
            Diff::Text { hunks } | Diff::LargeText { hunks } => {
                hunks.iter().map(|h| h.lines.len() + 1).sum()
            }
            _ => 0,
        }
    }

    /// The hunks of a text diff (large ones included).
    pub fn hunks(&self) -> Option<&[DiffHunk]> {
        match self {
            Diff::Text { hunks } | Diff::LargeText { hunks } => Some(hunks),
            _ => None,
        }
    }
}
