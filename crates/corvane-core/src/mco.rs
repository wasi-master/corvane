//! Multi-commit operations (GHD `IMultiCommitOperationState`,
//! `models/multi-commit-operation.ts`, `models/banner.ts` and the
//! `_rebase/_cherryPick/_squash/_reorderCommits/_mergeBranch` flows in
//! `app-store.ts` + `dispatcher.ts`): rebase, cherry-pick, squash, reorder
//! and merge, with the shared progress / conflicts / abort steps and the
//! banners shown when they finish.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use corvane_git::{CherryPickResult, RebaseResult};
use corvane_models::{
    Commit, CommitOneLine, FileStatusKind, ManualConflictResolution, McoProgress, Mergeability,
    MultiCommitOperationKind, Section, WorkingDirectoryFileChange, WorkingDirectoryStatus,
};
use gpui_kit::{App, AsyncApp};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::state::{Popup, RepositoryState, RetryAction};

/// `MultiCommitOperationStepKind`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McoStep {
    ChooseBranch,
    WarnForcePush,
    ShowProgress,
    ShowConflicts,
    /// The conflicts dialog was dismissed; a banner offers to reopen it.
    HideConflicts,
    ConfirmAbort,
    /// Cherry-pick to a new branch (`CreateBranchStep`).
    CreateBranch {
        initial_name: String,
    },
}

/// `MultiCommitOperationDetail`
#[derive(Clone, Debug, PartialEq)]
pub enum McoDetail {
    Rebase {
        /// The branch chosen to rebase onto (`sourceBranch`).
        base_branch: Option<String>,
        commits: Vec<CommitOneLine>,
    },
    CherryPick {
        source_branch: Option<String>,
        branch_created: bool,
        commits: Vec<CommitOneLine>,
    },
    Squash {
        commits: Vec<Commit>,
        target_commit: Commit,
        last_retained_ref: Option<String>,
        message: String,
    },
    Reorder {
        commits: Vec<Commit>,
        before_commit: Option<Commit>,
        last_retained_ref: Option<String>,
    },
    Merge {
        squash: bool,
        source_branch: Option<String>,
    },
}

impl McoDetail {
    pub fn kind(&self) -> MultiCommitOperationKind {
        match self {
            McoDetail::Rebase { .. } => MultiCommitOperationKind::Rebase,
            McoDetail::CherryPick { .. } => MultiCommitOperationKind::CherryPick,
            McoDetail::Squash { .. } => MultiCommitOperationKind::Squash,
            McoDetail::Reorder { .. } => MultiCommitOperationKind::Reorder,
            McoDetail::Merge { .. } => MultiCommitOperationKind::Merge,
        }
    }
}

/// `MultiCommitOperationConflictState` minus the resolutions, which live in
/// `RepositoryState::conflict_state`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McoConflicts {
    pub our_branch: Option<String>,
    pub their_branch: Option<String>,
}

/// `IMultiCommitOperationState`
#[derive(Clone, Debug, PartialEq)]
pub struct MultiCommitOperation {
    pub step: McoStep,
    pub detail: McoDetail,
    pub progress: McoProgress,
    pub user_has_resolved_conflicts: bool,
    pub original_branch_tip: Option<String>,
    /// Cherry-pick: the branch commits are copied to; otherwise the current branch.
    pub target_branch: Option<String>,
    pub conflicts: McoConflicts,
}

impl MultiCommitOperation {
    pub fn kind(&self) -> MultiCommitOperationKind {
        self.detail.kind()
    }

    pub fn in_conflict_step(&self) -> bool {
        matches!(
            self.step,
            McoStep::ShowConflicts | McoStep::HideConflicts | McoStep::ConfirmAbort
        )
    }
}

/// `IMultiCommitOperationUndoState` plus what the undo needs to know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McoUndo {
    pub sha: String,
    pub branch: String,
    pub kind: MultiCommitOperationKind,
    pub count: usize,
    pub source_branch: Option<String>,
    pub branch_created: bool,
}

/// `ConflictState` kinds derived from the repository (`MERGE_HEAD`,
/// `.git/rebase-merge`, `CHERRY_PICK_HEAD`, `SQUASH_MSG`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictKind {
    Merge {
        current_branch: String,
        current_tip: String,
    },
    Rebase {
        target_branch: String,
        base_branch_tip: String,
        original_branch_tip: String,
    },
    CherryPick {
        target_branch: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictState {
    pub kind: ConflictKind,
    pub manual_resolutions: BTreeMap<String, ManualConflictResolution>,
}

/// GHD `getConflictState`
pub fn derive_conflict_state(
    status: &WorkingDirectoryStatus,
    previous: Option<&ConflictState>,
) -> Option<ConflictState> {
    let kind = if status.merge_head_found || (status.squash_msg_found && status.has_conflicts()) {
        ConflictKind::Merge {
            current_branch: status.branch.clone()?,
            current_tip: status.current_tip.clone()?,
        }
    } else if let Some(rebase) = &status.rebase_internal_state {
        ConflictKind::Rebase {
            target_branch: rebase.target_branch.clone(),
            base_branch_tip: rebase.base_branch_tip.clone(),
            original_branch_tip: rebase.original_branch_tip.clone(),
        }
    } else if status.cherry_pick_head_found {
        ConflictKind::CherryPick {
            target_branch: status.branch.clone().unwrap_or_default(),
        }
    } else {
        return None;
    };
    let same_kind = previous
        .map(|p| std::mem::discriminant(&p.kind) == std::mem::discriminant(&kind))
        .unwrap_or(false);
    Some(ConflictState {
        manual_resolutions: if same_kind {
            previous
                .map(|p| p.manual_resolutions.clone())
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        },
        kind,
    })
}

/// GHD `getUnmergedFiles`
pub fn unmerged_files(status: &WorkingDirectoryStatus) -> Vec<&WorkingDirectoryFileChange> {
    status
        .files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Conflicted)
        .collect()
}

/// GHD `getConflictedFiles`: unmerged files that still need work.
pub fn conflicted_files<'a>(
    status: &'a WorkingDirectoryStatus,
    resolutions: &BTreeMap<String, ManualConflictResolution>,
) -> Vec<&'a WorkingDirectoryFileChange> {
    unmerged_files(status)
        .into_iter()
        .filter(|f| {
            f.status
                .has_unresolved_conflicts(resolutions.get(&f.path).copied())
        })
        .collect()
}

/// GHD `getResolvedFiles`
pub fn resolved_files<'a>(
    status: &'a WorkingDirectoryStatus,
    resolutions: &BTreeMap<String, ManualConflictResolution>,
) -> Vec<&'a WorkingDirectoryFileChange> {
    unmerged_files(status)
        .into_iter()
        .filter(|f| {
            !f.status
                .has_unresolved_conflicts(resolutions.get(&f.path).copied())
        })
        .collect()
}

/// `Banner`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Banner {
    SuccessfulMerge {
        our_branch: String,
        their_branch: Option<String>,
    },
    SuccessfulRebase {
        target_branch: String,
        base_branch: Option<String>,
    },
    BranchAlreadyUpToDate {
        our_branch: String,
        their_branch: Option<String>,
    },
    SuccessfulCherryPick {
        repo: u64,
        target_branch: String,
        count: usize,
    },
    CherryPickUndone {
        target_branch: String,
        count: usize,
    },
    SuccessfulSquash {
        repo: u64,
        count: usize,
    },
    SquashUndone {
        count: usize,
    },
    SuccessfulReorder {
        repo: u64,
        count: usize,
    },
    ReorderUndone {
        count: usize,
    },
    /// "Resolve conflicts to continue {description} **{branch}**."
    ConflictsFound {
        repo: u64,
        description: String,
        branch: Option<String>,
    },
}

impl Banner {
    /// GHD banner timeouts: 5 s for plain successes, 15 s when there is an Undo.
    pub fn timeout(&self) -> Option<Duration> {
        match self {
            Banner::SuccessfulMerge { .. }
            | Banner::SuccessfulRebase { .. }
            | Banner::BranchAlreadyUpToDate { .. }
            | Banner::CherryPickUndone { .. }
            | Banner::SquashUndone { .. }
            | Banner::ReorderUndone { .. } => Some(Duration::from_secs(5)),
            Banner::SuccessfulCherryPick { .. }
            | Banner::SuccessfulSquash { .. }
            | Banner::SuccessfulReorder { .. } => Some(Duration::from_secs(15)),
            Banner::ConflictsFound { .. } => None,
        }
    }

    pub fn dismissable(&self) -> bool {
        !matches!(self, Banner::ConflictsFound { .. })
    }
}

/// Merge dialog preview (`mergeStatus` + commit count).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergePreview {
    pub branch: String,
    pub commits: u32,
    pub mergeability: Option<Mergeability>,
}

/// Rebase dialog preview (`RebasePreview`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebasePreview {
    pub base_branch: String,
    /// Commits on the current branch that will be replayed (`commitsAhead`).
    pub commits_ahead: Vec<CommitOneLine>,
    /// Commits on the base branch missing from the current one (`commitsBehind`).
    pub behind: usize,
    pub valid: bool,
}

fn spawn_bg<T: Send + 'static>(
    cx: &mut App,
    work: impl FnOnce() -> T + Send + 'static,
    then: impl FnOnce(T, &mut App) + 'static,
) {
    let task = cx.background_executor().spawn(async move { work() });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let result = task.await;
        cx.update(|cx| then(result, cx));
    })
    .detach();
}

fn operation_description(kind: MultiCommitOperationKind) -> &'static str {
    match kind {
        MultiCommitOperationKind::Rebase => "rebasing",
        MultiCommitOperationKind::CherryPick => "cherry-picking onto",
        MultiCommitOperationKind::Squash => "squashing commits on",
        MultiCommitOperationKind::Reorder => "reordering commits on",
        MultiCommitOperationKind::Merge => "merge into",
    }
}

impl Dispatcher {
    // ---- banners ----

    /// `_setBanner`, auto-dismissed after the banner's timeout.
    pub fn set_banner(banner: Banner, cx: &mut App) {
        let timeout = banner.timeout();
        let nonce = Self::state(cx).update(cx, |s, cx| {
            s.banner_nonce += 1;
            s.banner = Some(banner);
            cx.notify();
            s.banner_nonce
        });
        if let Some(timeout) = timeout {
            cx.spawn(async move |cx: &mut AsyncApp| {
                cx.background_executor().timer(timeout).await;
                cx.update(|cx| {
                    Self::state(cx).update(cx, |s, cx| {
                        if s.banner_nonce == nonce {
                            s.banner = None;
                            cx.notify();
                        }
                    });
                });
            })
            .detach();
        }
    }

    pub fn clear_banner(cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if s.banner.take().is_some() {
                cx.notify();
            }
        });
    }

    fn clear_conflicts_banner(cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if matches!(s.banner, Some(Banner::ConflictsFound { .. })) {
                s.banner = None;
                cx.notify();
            }
        });
    }

    // ---- operation state ----

    fn mco(id: u64, cx: &App) -> Option<MultiCommitOperation> {
        Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.mco.clone())
    }

    fn update_mco(id: u64, cx: &mut App, edit: impl FnOnce(&mut MultiCommitOperation)) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(mco) = s.repo_state_mut(id).mco.as_mut() {
                edit(mco);
                cx.notify();
            }
        });
    }

    pub fn set_mco_step(id: u64, step: McoStep, cx: &mut App) {
        Self::update_mco(id, cx, |m| m.step = step);
    }

    fn init_mco(
        id: u64,
        detail: McoDetail,
        target_branch: Option<String>,
        original_branch_tip: Option<String>,
        step: McoStep,
        cx: &mut App,
    ) {
        let (first_summary, total) = match &detail {
            McoDetail::Rebase { commits, .. } | McoDetail::CherryPick { commits, .. } => (
                commits
                    .first()
                    .map(|c| c.summary.clone())
                    .unwrap_or_default(),
                commits.len(),
            ),
            McoDetail::Squash { commits, .. } | McoDetail::Reorder { commits, .. } => (
                commits
                    .first()
                    .map(|c| c.summary.clone())
                    .unwrap_or_default(),
                commits.len(),
            ),
            McoDetail::Merge { .. } => (String::new(), 0),
        };
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).mco_flow += 1;
            s.repo_state_mut(id).mco = Some(MultiCommitOperation {
                step,
                detail,
                progress: McoProgress {
                    value: 0.,
                    position: 1,
                    total,
                    current_summary: first_summary,
                },
                user_has_resolved_conflicts: false,
                original_branch_tip,
                target_branch,
                conflicts: McoConflicts::default(),
            });
            cx.notify();
        });
    }

    fn show_mco_popup(id: u64, cx: &mut App) {
        let flow = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .map(|r| r.mco_flow)
            .unwrap_or(0);
        Self::show_popup(Popup::MultiCommitOperation { repo: id, flow }, cx);
    }

    fn is_mco_popup(popup: &Option<Popup>, id: u64) -> bool {
        matches!(popup, Some(Popup::MultiCommitOperation { repo, .. }) if *repo == id)
    }

    fn close_mco_popup(id: u64, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if Self::is_mco_popup(&s.popup, id) {
                s.popup = None;
                cx.notify();
            }
        });
    }

    /// `_endMultiCommitOperation` (+ closing its dialog).
    pub fn end_mco(id: u64, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).mco = None;
            if Self::is_mco_popup(&s.popup, id) {
                s.popup = None;
            }
            cx.notify();
        });
    }

    pub(crate) fn current_branch_and_tip_pub(
        id: u64,
        cx: &App,
    ) -> Option<(String, Option<String>)> {
        Self::current_branch_and_tip(id, cx)
    }

    fn current_branch_and_tip(id: u64, cx: &App) -> Option<(String, Option<String>)> {
        let s = Self::state(cx).read(cx);
        let branch = s.repo_states.get(&id)?.info.as_ref()?.current_branch()?;
        Some((branch.name.clone(), branch.tip.clone()))
    }

    fn working_directory_files(id: u64, cx: &App) -> Vec<WorkingDirectoryFileChange> {
        Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.status.as_ref())
            .map(|st| st.files.clone())
            .unwrap_or_default()
    }

    fn manual_resolutions(id: u64, cx: &App) -> BTreeMap<String, ManualConflictResolution> {
        Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.conflict_state.as_ref())
            .map(|c| c.manual_resolutions.clone())
            .unwrap_or_default()
    }

    /// Store a fresh status (from an operation's background task) and derive
    /// the conflict state from it, like `_loadStatus`.
    fn apply_status(rs: &mut RepositoryState, status: WorkingDirectoryStatus) {
        rs.conflict_state = derive_conflict_state(&status, rs.conflict_state.as_ref());
        rs.status = Some(status);
    }

    /// `_checkForUncommittedChanges`: rebase-style operations need a clean
    /// working directory; offer to stash and retry.
    fn blocked_by_local_changes(id: u64, retry: RetryAction, cx: &mut App) -> bool {
        let files: Vec<String> = Self::working_directory_files(id, cx)
            .into_iter()
            .map(|f| f.path)
            .collect();
        if files.is_empty() {
            return false;
        }
        Self::show_popup(
            Popup::LocalChangesOverwritten {
                repo: id,
                retry,
                files,
            },
            cx,
        );
        true
    }

    /// `LocalChangesOverwrittenDialog` › Stash Changes and Continue.
    pub fn stash_and_retry(id: u64, retry: RetryAction, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some((branch, _)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        Self::close_popup(cx);
        spawn_bg(
            cx,
            move || {
                corvane_git::create_desktop_stash(git.clone(), &workdir, &branch)
                    .and_then(|_| corvane_git::get_status(git, &workdir, None))
            },
            move |result, cx| match result {
                Ok(status) => {
                    Self::state(cx).update(cx, |s, cx| {
                        Self::apply_status(s.repo_state_mut(id), status);
                        cx.notify();
                    });
                    Self::perform_retry(id, retry, cx);
                }
                Err(err) => {
                    Self::end_mco(id, cx);
                    Self::show_error("Could not stash changes", err.to_string(), cx);
                }
            },
        );
    }

    /// `performRetry`
    pub fn perform_retry(id: u64, retry: RetryAction, cx: &mut App) {
        match retry {
            RetryAction::CherryPick { target } => Self::cherry_pick_to_branch(id, target, cx),
            RetryAction::CherryPickNewBranch { name, start_point } => {
                Self::cherry_pick_to_new_branch(id, name, start_point, cx)
            }
            RetryAction::Squash {
                to_squash,
                onto,
                message,
            } => Self::squash(id, to_squash, onto, message, false, cx),
            RetryAction::Reorder { to_move, before } => {
                Self::reorder_commits(id, to_move, before, false, cx)
            }
            RetryAction::Push {
                force_with_lease,
                branch,
            } => Self::push(id, force_with_lease, branch, cx),
            RetryAction::Pull => Self::pull(id, cx),
            RetryAction::Fetch => Self::fetch(id, false, cx),
        }
    }

    /// `warnAboutRemoteCommits`: the branch tracks a remote that already has
    /// commits the operation is about to rewrite.
    fn remote_commits_would_be_rewritten(
        git: Arc<corvane_git::GitBinary>,
        workdir: &std::path::Path,
        upstream: Option<&str>,
        oldest_commit_ref: Option<&str>,
    ) -> bool {
        let Some(upstream) = upstream else {
            return false;
        };
        let upstream_exists =
            corvane_git::commits_in_range(git.clone(), workdir, &format!("{upstream}^!"))
                .ok()
                .flatten()
                .is_some();
        if !upstream_exists {
            return false;
        }
        let Some(oldest) = oldest_commit_ref else {
            return true;
        };
        corvane_git::commits_between(git, workdir, oldest, upstream)
            .ok()
            .flatten()
            .is_some_and(|c| !c.is_empty())
    }

    // ---- shared result processing ----

    /// `completeMultiCommitOperation`: banner, force-push bookkeeping, refresh.
    fn complete_mco(id: u64, count: usize, cx: &mut App) {
        let Some(mco) = Self::mco(id, cx) else {
            return;
        };
        let current = Self::current_branch_and_tip(id, cx);
        let banner = match &mco.detail {
            McoDetail::Squash { .. } => Banner::SuccessfulSquash { repo: id, count },
            McoDetail::Reorder { .. } => Banner::SuccessfulReorder { repo: id, count },
            McoDetail::CherryPick { .. } => Banner::SuccessfulCherryPick {
                repo: id,
                target_branch: mco.target_branch.clone().unwrap_or_default(),
                count,
            },
            McoDetail::Rebase { base_branch, .. } => Banner::SuccessfulRebase {
                target_branch: mco.target_branch.clone().unwrap_or_default(),
                base_branch: base_branch.clone(),
            },
            McoDetail::Merge { source_branch, .. } => Banner::SuccessfulMerge {
                our_branch: current.as_ref().map(|c| c.0.clone()).unwrap_or_default(),
                their_branch: source_branch.clone(),
            },
        };
        Self::set_banner(banner, cx);
        if mco.kind() != MultiCommitOperationKind::CherryPick
            && let Some((branch, Some(tip))) = current
            && let Some(original) = mco.original_branch_tip.clone()
            && original != tip
        {
            Self::state(cx).update(cx, |s, _| {
                s.repo_state_mut(id).force_push_branches.insert(branch, tip);
            });
        }
        Self::end_mco(id, cx);
        Self::refresh_repository(id, cx);
    }

    /// `startMultiCommitOperationConflictFlow`
    fn start_conflict_flow(id: u64, our: Option<String>, their: Option<String>, cx: &mut App) {
        let has_conflict_state = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|r| r.conflict_state.is_some());
        if !has_conflict_state {
            warn!(id, "conflict flow requested without a conflict state");
            Self::end_mco(id, cx);
            Self::refresh_repository(id, cx);
            return;
        }
        Self::update_mco(id, cx, |m| {
            m.step = McoStep::ShowConflicts;
            m.conflicts = McoConflicts {
                our_branch: our,
                their_branch: their,
            };
        });
        Self::show_mco_popup(id, cx);
        Self::refresh_repository(id, cx);
    }

    /// `processMultiCommitOperationRebaseResult`
    fn process_rebase_result(
        id: u64,
        result: RebaseResult,
        status: Option<WorkingDirectoryStatus>,
        count: usize,
        our: Option<String>,
        their: Option<String>,
        cx: &mut App,
    ) {
        if let Some(status) = status {
            Self::state(cx).update(cx, |s, cx| {
                Self::apply_status(s.repo_state_mut(id), status);
                cx.notify();
            });
        }
        match result {
            RebaseResult::CompletedWithoutError => {
                Self::show_section(id, Section::History, cx);
                Self::complete_mco(id, count, cx);
            }
            RebaseResult::AlreadyUpToDate => {
                let (our_branch, their_branch) = match Self::mco(id, cx).map(|m| m.detail) {
                    Some(McoDetail::Rebase { base_branch, .. }) => (
                        Self::current_branch_and_tip(id, cx)
                            .map(|c| c.0)
                            .unwrap_or_default(),
                        base_branch,
                    ),
                    _ => (our.unwrap_or_default(), their),
                };
                Self::set_banner(
                    Banner::BranchAlreadyUpToDate {
                        our_branch,
                        their_branch,
                    },
                    cx,
                );
                Self::end_mco(id, cx);
                Self::refresh_repository(id, cx);
            }
            RebaseResult::ConflictsEncountered => Self::start_conflict_flow(id, our, their, cx),
            RebaseResult::OutstandingFilesNotStaged | RebaseResult::Aborted => {
                Self::end_mco(id, cx);
                Self::refresh_repository(id, cx);
            }
            RebaseResult::Error(message) => {
                let kind = Self::mco(id, cx).map(|m| m.kind());
                Self::end_mco(id, cx);
                Self::show_error(
                    format!("{} failed", kind.map(|k| k.label()).unwrap_or("Operation")),
                    message,
                    cx,
                );
                Self::refresh_repository(id, cx);
            }
        }
    }

    // ---- rebase ----

    /// Branch › Rebase Current Branch…: open the choose-branch step.
    pub fn start_rebase_flow(id: u64, cx: &mut App) {
        let Some((current, tip)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).rebase_preview = None);
        Self::init_mco(
            id,
            McoDetail::Rebase {
                base_branch: None,
                commits: Vec::new(),
            },
            Some(current),
            tip,
            McoStep::ChooseBranch,
            cx,
        );
        Self::show_mco_popup(id, cx);
    }

    /// `updateRebasePreview`
    pub fn preview_rebase(id: u64, base_branch: String, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let base = base_branch.clone();
        spawn_bg(
            cx,
            move || {
                let ahead = corvane_git::commits_between(git.clone(), &workdir, &base, "HEAD");
                let behind = corvane_git::commits_between(git, &workdir, "HEAD", &base);
                (ahead, behind)
            },
            move |(ahead, behind), cx| {
                let preview = match (ahead, behind) {
                    (Ok(Some(ahead)), Ok(Some(behind))) => RebasePreview {
                        base_branch: base_branch.clone(),
                        commits_ahead: ahead,
                        behind: behind.len(),
                        valid: true,
                    },
                    _ => RebasePreview {
                        base_branch: base_branch.clone(),
                        commits_ahead: Vec::new(),
                        behind: 0,
                        valid: false,
                    },
                };
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_state_mut(id).rebase_preview = Some(preview);
                    cx.notify();
                });
            },
        );
    }

    /// `startRebase`: warn about a force push when needed, then rebase.
    pub fn start_rebase(id: u64, base_branch: String, force_push_checked: bool, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some((target, tip)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        let commits = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.rebase_preview.as_ref())
            .filter(|p| p.base_branch == base_branch)
            .map(|p| p.commits_ahead.clone())
            .unwrap_or_default();
        Self::init_mco(
            id,
            McoDetail::Rebase {
                base_branch: Some(base_branch.clone()),
                commits: commits.clone(),
            },
            Some(target.clone()),
            tip.clone(),
            McoStep::ShowProgress,
            cx,
        );
        Self::show_mco_popup(id, cx);
        let confirm = Self::state(cx).read(cx).settings.confirm_force_push;
        if confirm && !force_push_checked {
            // GHD warns when the branch being rewritten (the current one) is
            // published and its remote commits would be rewritten.
            let upstream = Self::branch_by_name(id, &target, cx).and_then(|b| b.upstream);
            let (git2, workdir2) = (git.clone(), workdir.clone());
            spawn_bg(
                cx,
                move || {
                    Self::remote_commits_would_be_rewritten(
                        git2,
                        &workdir2,
                        upstream.as_deref(),
                        tip.as_deref(),
                    )
                },
                move |warn, cx| {
                    if warn {
                        Self::set_mco_step(id, McoStep::WarnForcePush, cx);
                    } else {
                        Self::run_rebase(id, base_branch, target, commits, cx);
                    }
                },
            );
            return;
        }
        Self::run_rebase(id, base_branch, target, commits, cx);
    }

    fn run_rebase(
        id: u64,
        base_branch: String,
        target_branch: String,
        commits: Vec<CommitOneLine>,
        cx: &mut App,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        Self::set_mco_step(id, McoStep::ShowProgress, cx);
        info!(id, %base_branch, %target_branch, "starting rebase");
        let (base_for_result, target_for_result) = (base_branch.clone(), target_branch.clone());
        Self::run_with_progress(
            id,
            cx,
            move |on_progress| {
                let result = corvane_git::rebase(
                    git.clone(),
                    &workdir,
                    &base_branch,
                    &target_branch,
                    &commits,
                    on_progress,
                );
                let status = corvane_git::get_status(git, &workdir, None).ok();
                (result, status)
            },
            move |(result, status), cx| {
                let count = Self::mco(id, cx)
                    .map(|m| match m.detail {
                        McoDetail::Rebase { commits, .. } => commits.len(),
                        _ => 0,
                    })
                    .unwrap_or(0);
                Self::process_rebase_result(
                    id,
                    result,
                    status,
                    count,
                    Some(base_for_result),
                    Some(target_for_result),
                    cx,
                );
            },
        );
    }

    /// Run a git operation that reports `McoProgress` on a background thread,
    /// mirroring it into the operation state as it arrives.
    fn run_with_progress<T: Send + 'static>(
        id: u64,
        cx: &mut App,
        work: impl FnOnce(&mut dyn FnMut(McoProgress)) -> T + Send + 'static,
        then: impl FnOnce(T, &mut App) + 'static,
    ) {
        let (tx, rx) = async_channel::unbounded::<McoProgress>();
        let task = cx.background_executor().spawn(async move {
            let mut report = |p: McoProgress| {
                let _ = tx.send_blocking(p);
            };
            work(&mut report)
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(progress) = rx.recv().await {
                cx.update(|cx| Self::update_mco(id, cx, |m| m.progress = progress));
            }
            let result = task.await;
            cx.update(|cx| then(result, cx));
        })
        .detach();
    }

    /// `WarnForcePushDialog` › Begin: remember the checkbox, run the operation.
    pub fn begin_after_force_push_warning(id: u64, ask_again: bool, cx: &mut App) {
        if !ask_again {
            Self::update_settings(cx, |s| s.confirm_force_push = false);
        }
        let Some(mco) = Self::mco(id, cx) else {
            return;
        };
        match mco.detail {
            McoDetail::Rebase {
                base_branch: Some(base),
                commits,
            } => {
                let target = mco.target_branch.clone().unwrap_or_default();
                Self::run_rebase(id, base, target, commits, cx);
            }
            McoDetail::Squash {
                commits,
                target_commit,
                message,
                ..
            } => Self::squash(
                id,
                commits.iter().map(|c| c.sha.clone()).collect(),
                target_commit.sha,
                message,
                true,
                cx,
            ),
            McoDetail::Reorder {
                commits,
                before_commit,
                ..
            } => Self::reorder_commits(
                id,
                commits.iter().map(|c| c.sha.clone()).collect(),
                before_commit.map(|c| c.sha),
                true,
                cx,
            ),
            _ => Self::end_mco(id, cx),
        }
    }

    // ---- conflicts dialog ----

    /// `updateManualConflictResolution`
    pub fn set_manual_resolution(
        id: u64,
        path: String,
        resolution: Option<ManualConflictResolution>,
        cx: &mut App,
    ) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(conflict) = s.repo_state_mut(id).conflict_state.as_mut() {
                match resolution {
                    Some(r) => {
                        conflict.manual_resolutions.insert(path, r);
                    }
                    None => {
                        conflict.manual_resolutions.remove(&path);
                    }
                }
                cx.notify();
            }
        });
    }

    fn note_resolved_conflicts(id: u64, cx: &mut App) {
        let any_resolved = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            match (
                rs.and_then(|r| r.status.as_ref()),
                rs.and_then(|r| r.conflict_state.as_ref()),
            ) {
                (Some(status), Some(conflict)) => {
                    !resolved_files(status, &conflict.manual_resolutions).is_empty()
                }
                _ => false,
            }
        };
        if any_resolved {
            Self::update_mco(id, cx, |m| m.user_has_resolved_conflicts = true);
        }
    }

    /// Conflicts dialog dismissed (`onInvokeConflictsDialogDismissed`): keep
    /// the operation, show the "Resolve conflicts to continue…" banner.
    pub fn hide_conflicts(id: u64, cx: &mut App) {
        Self::note_resolved_conflicts(id, cx);
        let Some(mco) = Self::mco(id, cx) else {
            Self::close_mco_popup(id, cx);
            return;
        };
        let branch = match mco.kind() {
            MultiCommitOperationKind::Merge => mco
                .conflicts
                .our_branch
                .clone()
                .or_else(|| Self::current_branch_and_tip(id, cx).map(|c| c.0)),
            _ => mco.target_branch.clone(),
        };
        Self::set_mco_step(id, McoStep::HideConflicts, cx);
        Self::close_mco_popup(id, cx);
        Self::set_banner(
            Banner::ConflictsFound {
                repo: id,
                description: operation_description(mco.kind()).to_string(),
                branch,
            },
            cx,
        );
    }

    /// Banner › View conflicts.
    pub fn show_conflicts(id: u64, cx: &mut App) {
        Self::clear_conflicts_banner(cx);
        if Self::mco(id, cx).is_none() {
            return;
        }
        Self::set_mco_step(id, McoStep::ShowConflicts, cx);
        Self::show_mco_popup(id, cx);
    }

    /// Conflicts dialog › Abort: confirm first when work would be lost.
    pub fn request_abort_mco(id: u64, cx: &mut App) {
        Self::note_resolved_conflicts(id, cx);
        let Some(mco) = Self::mco(id, cx) else {
            return;
        };
        if mco.user_has_resolved_conflicts {
            Self::set_mco_step(id, McoStep::ConfirmAbort, cx);
        } else {
            Self::abort_mco(id, cx);
        }
    }

    /// Confirm-abort dialog › Cancel.
    pub fn return_to_conflicts(id: u64, cx: &mut App) {
        Self::set_mco_step(id, McoStep::ShowConflicts, cx);
    }

    /// `onAbort` per operation kind.
    pub fn abort_mco(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(mco) = Self::mco(id, cx) else {
            return;
        };
        let tip = Self::current_branch_and_tip(id, cx).and_then(|c| c.1);
        Self::end_mco(id, cx);
        let source = match &mco.detail {
            McoDetail::CherryPick { source_branch, .. } => source_branch
                .as_deref()
                .and_then(|name| Self::branch_by_name(id, name, cx)),
            _ => None,
        };
        let detail = mco.detail.clone();
        spawn_bg(
            cx,
            move || -> corvane_git::error::Result<()> {
                match detail {
                    McoDetail::Merge { squash: false, .. } => {
                        corvane_git::abort_merge(git, &workdir)
                    }
                    McoDetail::Merge { squash: true, .. } => match tip {
                        Some(tip) => corvane_git::abort_squash_merge(git, &workdir, &tip),
                        None => Ok(()),
                    },
                    McoDetail::Rebase { .. }
                    | McoDetail::Squash { .. }
                    | McoDetail::Reorder { .. } => corvane_git::abort_rebase(git, &workdir),
                    McoDetail::CherryPick { .. } => {
                        corvane_git::abort_cherry_pick(git.clone(), &workdir)?;
                        if let Some(source) = source {
                            corvane_git::checkout_branch(git, &workdir, &source)?;
                        }
                        Ok(())
                    }
                }
            },
            move |result, cx| {
                if let Err(err) = result {
                    Self::show_error("Could not abort", err.to_string(), cx);
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// Conflicts dialog › Continue.
    pub fn continue_after_conflicts(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(mco) = Self::mco(id, cx) else {
            return;
        };
        let files = Self::working_directory_files(id, cx);
        let resolutions = Self::manual_resolutions(id, cx);
        let (our, their) = (
            mco.conflicts.our_branch.clone(),
            mco.conflicts.their_branch.clone(),
        );
        match mco.detail.clone() {
            McoDetail::Merge {
                squash,
                source_branch,
            } => {
                let conflicted: Vec<WorkingDirectoryFileChange> = files
                    .into_iter()
                    .filter(|f| f.status.kind == FileStatusKind::Conflicted)
                    .collect();
                Self::set_mco_step(id, McoStep::ShowProgress, cx);
                spawn_bg(
                    cx,
                    move || {
                        corvane_git::create_merge_commit(git, &workdir, &conflicted, &resolutions)
                    },
                    move |result, cx| {
                        match result {
                            Ok(_) => {
                                let our_branch = Self::current_branch_and_tip(id, cx)
                                    .map(|c| c.0)
                                    .unwrap_or_default();
                                Self::set_banner(
                                    Banner::SuccessfulMerge {
                                        our_branch,
                                        their_branch: source_branch,
                                    },
                                    cx,
                                );
                                let _ = squash;
                            }
                            Err(err) => Self::show_error("Could not merge", err.to_string(), cx),
                        }
                        Self::end_mco(id, cx);
                        Self::show_section(id, Section::Changes, cx);
                        Self::refresh_repository(id, cx);
                    },
                );
            }
            McoDetail::Rebase { commits, .. } => {
                let count = commits.len();
                Self::set_mco_step(id, McoStep::ShowProgress, cx);
                Self::run_with_progress(
                    id,
                    cx,
                    move |on_progress| {
                        let result = corvane_git::continue_rebase(
                            git.clone(),
                            &workdir,
                            &files,
                            &resolutions,
                            &commits,
                            on_progress,
                        )
                        .unwrap_or_else(|e| RebaseResult::Error(e.to_string()));
                        let status = corvane_git::get_status(git, &workdir, None).ok();
                        (result, status)
                    },
                    move |(result, status), cx| {
                        Self::process_rebase_result(id, result, status, count, our, their, cx)
                    },
                );
            }
            McoDetail::Squash { commits, .. } | McoDetail::Reorder { commits, .. } => {
                let one_line: Vec<CommitOneLine> = commits
                    .iter()
                    .map(|c| CommitOneLine {
                        sha: c.sha.clone(),
                        summary: c.summary.clone(),
                    })
                    .collect();
                let count = if mco.kind() == MultiCommitOperationKind::Squash {
                    commits.len() + 1
                } else {
                    commits.len()
                };
                Self::set_mco_step(id, McoStep::ShowProgress, cx);
                Self::run_with_progress(
                    id,
                    cx,
                    move |on_progress| {
                        let result = corvane_git::continue_rebase(
                            git.clone(),
                            &workdir,
                            &files,
                            &resolutions,
                            &one_line,
                            on_progress,
                        )
                        .unwrap_or_else(|e| RebaseResult::Error(e.to_string()));
                        let status = corvane_git::get_status(git, &workdir, None).ok();
                        (result, status)
                    },
                    move |(result, status), cx| {
                        Self::process_rebase_result(id, result, status, count, our, their, cx)
                    },
                );
            }
            McoDetail::CherryPick {
                commits,
                source_branch,
                ..
            } => {
                Self::set_mco_step(id, McoStep::ShowProgress, cx);
                Self::run_with_progress(
                    id,
                    cx,
                    move |on_progress| {
                        let result = corvane_git::continue_cherry_pick(
                            git.clone(),
                            &workdir,
                            &files,
                            &resolutions,
                            on_progress,
                        )
                        .unwrap_or_else(|e| CherryPickResult::Error(e.to_string()));
                        let status = corvane_git::get_status(git, &workdir, None).ok();
                        (result, status)
                    },
                    move |(result, status), cx| {
                        Self::process_cherry_pick_result(
                            id,
                            result,
                            status,
                            commits,
                            source_branch,
                            cx,
                        )
                    },
                );
            }
        }
    }

    // ---- merge ----

    /// `_mergeBranch` (+ `initializeMergeOperation`).
    pub fn merge_branch(id: u64, branch: String, squash: bool, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some((current, tip)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        Self::init_mco(
            id,
            McoDetail::Merge {
                squash,
                source_branch: Some(branch.clone()),
            },
            Some(current.clone()),
            tip,
            McoStep::ShowProgress,
            cx,
        );
        let name = branch.clone();
        spawn_bg(
            cx,
            move || {
                let result = corvane_git::merge_branch(git.clone(), &workdir, &branch, squash);
                let status = corvane_git::get_status(git, &workdir, None).ok();
                (result, status)
            },
            move |(result, status), cx| {
                if let Some(status) = status {
                    Self::state(cx).update(cx, |s, cx| {
                        Self::apply_status(s.repo_state_mut(id), status);
                        cx.notify();
                    });
                }
                match result {
                    Ok(corvane_git::MergeOutcome::Success) => {
                        Self::set_banner(
                            Banner::SuccessfulMerge {
                                our_branch: current,
                                their_branch: Some(name),
                            },
                            cx,
                        );
                        Self::end_mco(id, cx);
                    }
                    Ok(corvane_git::MergeOutcome::AlreadyUpToDate) => {
                        Self::set_banner(
                            Banner::BranchAlreadyUpToDate {
                                our_branch: current,
                                their_branch: Some(name),
                            },
                            cx,
                        );
                        Self::end_mco(id, cx);
                    }
                    Ok(corvane_git::MergeOutcome::Conflicts) => {
                        Self::start_conflict_flow(id, Some(current), Some(name), cx);
                    }
                    Ok(corvane_git::MergeOutcome::Failed(msg)) => {
                        Self::end_mco(id, cx);
                        Self::show_error("Could not merge", msg, cx);
                    }
                    Err(err) => {
                        Self::end_mco(id, cx);
                        Self::show_error("Could not merge", err.to_string(), cx);
                    }
                }
                Self::show_section(id, Section::Changes, cx);
                Self::refresh_repository(id, cx);
            },
        );
    }

    // ---- cherry-pick ----

    /// History › Cherry-pick Commit(s)…: open the choose-target-branch step.
    pub fn start_cherry_pick_flow(id: u64, shas: Vec<String>, cx: &mut App) {
        let Some((current, tip)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        let commits = Self::commits_oldest_first(id, &shas, cx);
        if commits.is_empty() {
            return;
        }
        Self::init_mco(
            id,
            McoDetail::CherryPick {
                source_branch: Some(current),
                branch_created: false,
                commits,
            },
            None,
            tip,
            McoStep::ChooseBranch,
            cx,
        );
        Self::show_mco_popup(id, cx);
    }

    /// `orderCommitsByHistory`: the given commits in log order, oldest first.
    fn commits_oldest_first(id: u64, shas: &[String], cx: &App) -> Vec<CommitOneLine> {
        let s = Self::state(cx).read(cx);
        let Some(rs) = s.repo_states.get(&id) else {
            return Vec::new();
        };
        let wanted: HashSet<&str> = shas.iter().map(String::as_str).collect();
        rs.visible_commits()
            .iter()
            .rev()
            .filter(|c| wanted.contains(c.sha.as_str()))
            .map(|c| CommitOneLine {
                sha: c.sha.clone(),
                summary: c.summary.clone(),
            })
            .collect()
    }

    /// Choose-target-branch › New Branch: switch to the create-branch step.
    pub fn cherry_pick_show_create_branch(id: u64, initial_name: String, cx: &mut App) {
        Self::update_mco(id, cx, |m| {
            if let McoDetail::CherryPick { branch_created, .. } = &mut m.detail {
                *branch_created = true;
            }
            m.step = McoStep::CreateBranch { initial_name };
        });
    }

    /// `startCherryPickWithBranchName`
    pub fn cherry_pick_to_new_branch(
        id: u64,
        name: String,
        start_point: Option<String>,
        cx: &mut App,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        if Self::blocked_by_local_changes(
            id,
            RetryAction::CherryPickNewBranch {
                name: name.clone(),
                start_point: start_point.clone(),
            },
            cx,
        ) {
            return;
        }
        Self::set_mco_step(id, McoStep::ShowProgress, cx);
        let branch = name.clone();
        spawn_bg(
            cx,
            move || {
                corvane_git::create_branch(git, &workdir, &branch, start_point.as_deref(), false)
            },
            move |result, cx| match result {
                Ok(()) => {
                    Self::update_mco(id, cx, |m| {
                        if let McoDetail::CherryPick { branch_created, .. } = &mut m.detail {
                            *branch_created = true;
                        }
                    });
                    Self::cherry_pick_to_branch(id, name, cx);
                }
                Err(err) => {
                    Self::end_mco(id, cx);
                    Self::show_error("Could not create branch", err.to_string(), cx);
                }
            },
        );
    }

    /// `cherryPick`: check out the target branch, then copy the commits.
    pub fn cherry_pick_to_branch(id: u64, target_name: String, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(mco) = Self::mco(id, cx) else {
            return;
        };
        let McoDetail::CherryPick {
            commits,
            source_branch,
            branch_created,
        } = mco.detail.clone()
        else {
            return;
        };
        Self::update_mco(id, cx, |m| {
            m.target_branch = Some(target_name.clone());
            m.step = McoStep::ShowProgress;
        });
        Self::show_mco_popup(id, cx);
        if Self::blocked_by_local_changes(
            id,
            RetryAction::CherryPick {
                target: target_name.clone(),
            },
            cx,
        ) {
            return;
        }
        // a freshly created branch may not be in the cached branch list yet
        let target = Self::branch_by_name(id, &target_name, cx).unwrap_or(corvane_models::Branch {
            name: target_name.clone(),
            kind: corvane_models::BranchKind::Local,
            full_name: format!("refs/heads/{target_name}"),
            tip: None,
            upstream: None,
            tip_time: None,
        });
        let local_name = target.name_without_remote().to_string();
        let count = commits.len();
        let commits_for_result = commits.clone();
        Self::run_with_progress(
            id,
            cx,
            move |on_progress| {
                if let Err(err) = corvane_git::checkout_branch(git.clone(), &workdir, &target) {
                    return (CherryPickResult::Error(err.to_string()), None, None, false);
                }
                let undo_sha = corvane_git::head_sha(git.clone(), &workdir).ok();
                let result = corvane_git::cherry_pick(git.clone(), &workdir, &commits, on_progress);
                let status = corvane_git::get_status(git, &workdir, None).ok();
                (result, status, undo_sha, true)
            },
            move |(result, status, undo_sha, checked_out), cx| {
                if !checked_out {
                    Self::end_mco(id, cx);
                    if let CherryPickResult::Error(msg) = result {
                        Self::show_error("Could not check out the target branch", msg, cx);
                    }
                    Self::refresh_repository(id, cx);
                    return;
                }
                if let Some(sha) = undo_sha {
                    Self::state(cx).update(cx, |s, _| {
                        s.repo_state_mut(id).mco_undo = Some(McoUndo {
                            sha,
                            branch: local_name.clone(),
                            kind: MultiCommitOperationKind::CherryPick,
                            count,
                            source_branch: source_branch.clone(),
                            branch_created,
                        });
                    });
                }
                Self::update_mco(id, cx, |m| m.target_branch = Some(local_name.clone()));
                Self::process_cherry_pick_result(
                    id,
                    result,
                    status,
                    commits_for_result,
                    source_branch,
                    cx,
                );
            },
        );
    }

    /// `processCherryPickResult`
    fn process_cherry_pick_result(
        id: u64,
        result: CherryPickResult,
        status: Option<WorkingDirectoryStatus>,
        commits: Vec<CommitOneLine>,
        source_branch: Option<String>,
        cx: &mut App,
    ) {
        if let Some(status) = status {
            Self::state(cx).update(cx, |s, cx| {
                Self::apply_status(s.repo_state_mut(id), status);
                cx.notify();
            });
        }
        match result {
            CherryPickResult::CompletedWithoutError => {
                Self::show_section(id, Section::History, cx);
                Self::complete_mco(id, commits.len(), cx);
            }
            CherryPickResult::ConflictsEncountered => {
                let target = Self::mco(id, cx).and_then(|m| m.target_branch);
                Self::start_conflict_flow(id, target, source_branch, cx);
            }
            CherryPickResult::UnableToStart => {
                Self::end_mco(id, cx);
                Self::refresh_repository(id, cx);
            }
            CherryPickResult::OutstandingFilesNotStaged => {
                Self::end_mco(id, cx);
                Self::refresh_repository(id, cx);
            }
            CherryPickResult::Error(message) => {
                // clear the half-done cherry-pick and go back where the user was
                let Some((git, workdir)) = Self::repo_context(id, cx) else {
                    return;
                };
                let source = source_branch.and_then(|n| Self::branch_by_name(id, &n, cx));
                Self::end_mco(id, cx);
                Self::show_error("Could not cherry-pick", message, cx);
                spawn_bg(
                    cx,
                    move || {
                        if corvane_git::cherry_pick_head_found(&workdir) {
                            let _ = corvane_git::abort_cherry_pick(git.clone(), &workdir);
                        }
                        if let Some(source) = source {
                            let _ = corvane_git::checkout_branch(git, &workdir, &source);
                        }
                    },
                    move |_, cx| Self::refresh_repository(id, cx),
                );
            }
        }
    }

    // ---- squash / reorder ----

    /// GHD `getLastRetainedCommitRef`: `<oldest involved>^`, or `None` when
    /// the oldest involved commit is the root of the loaded history.
    fn last_retained_ref(rs: &RepositoryState, shas: &[String]) -> Option<Option<String>> {
        let indexes: Vec<usize> = shas
            .iter()
            .filter_map(|sha| rs.commits.iter().position(|c| &c.sha == sha))
            .collect();
        if indexes.len() != shas.len() {
            return None;
        }
        let max = *indexes.iter().max()?;
        // the oldest involved commit is the root of the (fully loaded) history: rebase --root
        Some(if max + 1 == rs.commits.len() && rs.commits_exhausted {
            None
        } else {
            Some(format!("{}^", rs.commits[max].sha))
        })
    }

    /// History › Squash N Commits… (and drag-onto-commit): ask for the
    /// combined message after checking for merge commits in the range.
    pub fn request_squash(id: u64, to_squash: Vec<String>, onto: String, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let to_squash: Vec<String> = to_squash.into_iter().filter(|s| s != &onto).collect();
        if to_squash.is_empty() {
            return;
        }
        let (last_retained, summary, description, count) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let mut involved = to_squash.clone();
            involved.push(onto.clone());
            let Some(last_retained) = Self::last_retained_ref(rs, &involved) else {
                return;
            };
            let Some(onto_commit) = rs.commits.iter().find(|c| c.sha == onto) else {
                return;
            };
            // `getSquashedCommitDescription`: onto's body, then each squashed
            // commit's summary + body, oldest first
            let mut parts: Vec<String> = Vec::new();
            if !onto_commit.body.trim().is_empty() {
                parts.push(onto_commit.body.trim().to_string());
            }
            for c in rs.commits.iter().rev() {
                if to_squash.contains(&c.sha) {
                    let mut text = c.summary.clone();
                    if !c.body.trim().is_empty() {
                        text.push_str("\n\n");
                        text.push_str(c.body.trim());
                    }
                    parts.push(text);
                }
            }
            (
                last_retained,
                onto_commit.summary.clone(),
                parts.join("\n\n"),
                to_squash.len() + 1,
            )
        };
        spawn_bg(
            cx,
            move || corvane_git::merge_commits_exist_after(git, &workdir, last_retained.as_deref()),
            move |has_merges, cx| {
                if has_merges.unwrap_or(false) {
                    Self::show_error(
                        "Unable to squash",
                        "Squashing replays all commits up to the last one required for the squash. A merge commit cannot exist among those commits.",
                        cx,
                    );
                    return;
                }
                Self::show_popup(
                    Popup::SquashCommitMessage {
                        repo: id,
                        to_squash,
                        onto,
                        summary,
                        description,
                        count,
                    },
                    cx,
                );
            },
        );
    }

    /// `squash`
    pub fn squash(
        id: u64,
        to_squash: Vec<String>,
        onto: String,
        message: String,
        force_push_checked: bool,
        cx: &mut App,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        if Self::blocked_by_local_changes(
            id,
            RetryAction::Squash {
                to_squash: to_squash.clone(),
                onto: onto.clone(),
                message: message.clone(),
            },
            cx,
        ) {
            return;
        }
        let Some((branch, tip)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        let (commits, target_commit, last_retained) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let commits: Vec<Commit> = rs
                .commits
                .iter()
                .filter(|c| to_squash.contains(&c.sha))
                .cloned()
                .collect();
            let Some(target) = rs.commits.iter().find(|c| c.sha == onto).cloned() else {
                return;
            };
            let mut involved = to_squash.clone();
            involved.push(onto.clone());
            let Some(last_retained) = Self::last_retained_ref(rs, &involved) else {
                return;
            };
            (commits, target, last_retained)
        };
        Self::init_mco(
            id,
            McoDetail::Squash {
                commits: commits.clone(),
                target_commit: target_commit.clone(),
                last_retained_ref: last_retained.clone(),
                message: message.clone(),
            },
            Some(branch.clone()),
            tip.clone(),
            McoStep::ShowProgress,
            cx,
        );
        Self::show_mco_popup(id, cx);
        if let Some(tip) = tip.clone() {
            Self::state(cx).update(cx, |s, _| {
                s.repo_state_mut(id).mco_undo = Some(McoUndo {
                    sha: tip,
                    branch: branch.clone(),
                    kind: MultiCommitOperationKind::Squash,
                    count: commits.len() + 1,
                    source_branch: None,
                    branch_created: false,
                });
            });
        }
        let count = commits.len() + 1;
        let run = move |cx: &mut App| {
            let (git, workdir) = (git.clone(), workdir.clone());
            let branch = branch.clone();
            let (commits, target_commit, last_retained, message) = (
                commits.clone(),
                target_commit.clone(),
                last_retained.clone(),
                message.clone(),
            );
            Self::run_with_progress(
                id,
                cx,
                move |on_progress| {
                    let result = corvane_git::squash(
                        git.clone(),
                        &workdir,
                        &commits,
                        &target_commit,
                        last_retained.as_deref(),
                        &message,
                        on_progress,
                    );
                    let status = corvane_git::get_status(git, &workdir, None).ok();
                    (result, status)
                },
                move |(result, status), cx| {
                    Self::process_rebase_result(
                        id,
                        result,
                        status,
                        count,
                        Some(branch),
                        Some("squash commit".into()),
                        cx,
                    )
                },
            );
        };
        Self::maybe_warn_force_push(
            id,
            force_push_checked,
            last_retained_for_warn(&Self::mco(id, cx)),
            run,
            cx,
        );
    }

    /// Shared "warn about force push, else run" step for squash / reorder.
    fn maybe_warn_force_push(
        id: u64,
        force_push_checked: bool,
        oldest_commit_ref: Option<String>,
        run: impl FnOnce(&mut App) + 'static,
        cx: &mut App,
    ) {
        let confirm = Self::state(cx).read(cx).settings.confirm_force_push;
        if !confirm || force_push_checked {
            run(cx);
            return;
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let upstream = Self::current_branch_and_tip(id, cx)
            .and_then(|(name, _)| Self::branch_by_name(id, &name, cx))
            .and_then(|b| b.upstream);
        spawn_bg(
            cx,
            move || {
                Self::remote_commits_would_be_rewritten(
                    git,
                    &workdir,
                    upstream.as_deref(),
                    oldest_commit_ref.as_deref(),
                )
            },
            move |warn, cx| {
                if warn {
                    Self::set_mco_step(id, McoStep::WarnForcePush, cx);
                } else {
                    run(cx);
                }
            },
        );
    }

    /// `reorderCommits`: move `to_move` right before `before` (`None` = tip).
    pub fn reorder_commits(
        id: u64,
        to_move: Vec<String>,
        before: Option<String>,
        force_push_checked: bool,
        cx: &mut App,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        if to_move.is_empty() {
            return;
        }
        if Self::blocked_by_local_changes(
            id,
            RetryAction::Reorder {
                to_move: to_move.clone(),
                before: before.clone(),
            },
            cx,
        ) {
            return;
        }
        let Some((branch, tip)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        let (commits, before_commit, last_retained) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let commits: Vec<Commit> = rs
                .commits
                .iter()
                .filter(|c| to_move.contains(&c.sha))
                .cloned()
                .collect();
            let before_commit = before
                .as_ref()
                .and_then(|sha| rs.commits.iter().find(|c| &c.sha == sha).cloned());
            let mut involved = to_move.clone();
            involved.extend(before.iter().cloned());
            let Some(last_retained) = Self::last_retained_ref(rs, &involved) else {
                return;
            };
            (commits, before_commit, last_retained)
        };
        Self::init_mco(
            id,
            McoDetail::Reorder {
                commits: commits.clone(),
                before_commit: before_commit.clone(),
                last_retained_ref: last_retained.clone(),
            },
            Some(branch.clone()),
            tip.clone(),
            McoStep::ShowProgress,
            cx,
        );
        Self::show_mco_popup(id, cx);
        if let Some(tip) = tip.clone() {
            Self::state(cx).update(cx, |s, _| {
                s.repo_state_mut(id).mco_undo = Some(McoUndo {
                    sha: tip,
                    branch: branch.clone(),
                    kind: MultiCommitOperationKind::Reorder,
                    count: commits.len(),
                    source_branch: None,
                    branch_created: false,
                });
            });
        }
        let count = commits.len();
        let last_retained_for_run = last_retained.clone();
        let run = move |cx: &mut App| {
            let (git, workdir) = (git.clone(), workdir.clone());
            let branch = branch.clone();
            let (commits, before_commit, last_retained) = (
                commits.clone(),
                before_commit.clone(),
                last_retained_for_run.clone(),
            );
            Self::run_with_progress(
                id,
                cx,
                move |on_progress| {
                    let result = corvane_git::reorder(
                        git.clone(),
                        &workdir,
                        &commits,
                        before_commit.as_ref(),
                        last_retained.as_deref(),
                        on_progress,
                    );
                    let status = corvane_git::get_status(git, &workdir, None).ok();
                    (result, status)
                },
                move |(result, status), cx| {
                    Self::process_rebase_result(
                        id,
                        result,
                        status,
                        count,
                        Some(branch),
                        Some("reorder commit".into()),
                        cx,
                    )
                },
            );
        };
        Self::maybe_warn_force_push(id, force_push_checked, last_retained, run, cx);
    }

    /// Banner › Undo (`_undoMultiCommitOperation`).
    pub fn undo_mco(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(undo) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.mco_undo.clone())
        else {
            return;
        };
        let dirty = !Self::working_directory_files(id, cx).is_empty();
        let Some((branch, _)) = Self::current_branch_and_tip(id, cx) else {
            return;
        };
        if dirty {
            Self::show_error(
                format!("Could not undo {}", undo.kind.lower()),
                "This would delete the local changes that exist on the branch.",
                cx,
            );
            return;
        }
        if branch != undo.branch {
            Self::show_error(
                format!("Could not undo {}", undo.kind.lower()),
                format!(
                    "You are no longer on the branch the {} occurred on.",
                    undo.kind.lower()
                ),
                cx,
            );
            return;
        }
        Self::clear_banner(cx);
        let source = undo
            .source_branch
            .as_deref()
            .and_then(|n| Self::branch_by_name(id, n, cx));
        let undo_for_bg = undo.clone();
        spawn_bg(
            cx,
            move || -> corvane_git::error::Result<()> {
                let u = undo_for_bg;
                if u.kind == MultiCommitOperationKind::CherryPick && u.branch_created {
                    if let Some(source) = &source {
                        corvane_git::checkout_branch(git.clone(), &workdir, source)?;
                    }
                    return corvane_git::delete_local_branch(git, &workdir, &u.branch);
                }
                corvane_git::reset_to(git.clone(), &workdir, corvane_git::ResetMode::Hard, &u.sha)?;
                if u.kind == MultiCommitOperationKind::CherryPick
                    && let Some(source) = &source
                {
                    corvane_git::checkout_branch(git, &workdir, source)?;
                }
                Ok(())
            },
            move |result, cx| {
                match result {
                    Ok(()) => {
                        let banner = match undo.kind {
                            MultiCommitOperationKind::Squash => {
                                Banner::SquashUndone { count: undo.count }
                            }
                            MultiCommitOperationKind::Reorder => {
                                Banner::ReorderUndone { count: undo.count }
                            }
                            MultiCommitOperationKind::CherryPick => Banner::CherryPickUndone {
                                target_branch: undo.branch.clone(),
                                count: undo.count,
                            },
                            _ => return,
                        };
                        Self::set_banner(banner, cx);
                        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).mco_undo = None);
                    }
                    Err(err) => Self::show_error("Could not undo", err.to_string(), cx),
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    // ---- refresh integration ----

    /// After every status load (`_loadStatus` → `updateConflictState`,
    /// `initializeMultiCommitOperationIfConflictsFound`, `_triggerConflictsFlow`,
    /// `clearConflictsFlowVisuals`).
    pub(crate) fn sync_conflicts(
        id: u64,
        rebase_snapshot: Option<corvane_git::RebaseSnapshot>,
        cherry_pick_snapshot: Option<corvane_git::CherryPickSnapshot>,
        cx: &mut App,
    ) {
        let (conflict, mco, current, popup_open, banner_is_conflicts) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            (
                rs.and_then(|r| r.conflict_state.clone()),
                rs.and_then(|r| r.mco.clone()),
                rs.and_then(|r| r.info.as_ref())
                    .and_then(|i| i.current_branch())
                    .map(|b| (b.name.clone(), b.tip.clone())),
                s.popup.is_some(),
                matches!(s.banner, Some(Banner::ConflictsFound { .. })),
            )
        };
        let Some(conflict) = conflict else {
            // conflicts went away (resolved elsewhere, or aborted): drop the visuals
            if let Some(mco) = mco
                && mco.in_conflict_step()
            {
                Self::end_mco(id, cx);
                Self::clear_conflicts_banner(cx);
            }
            return;
        };
        if mco.is_none() {
            // an operation started outside Corvane (or before a restart)
            let (detail, target, original_tip, progress, our, their) = match &conflict.kind {
                ConflictKind::Merge { current_branch, .. } => (
                    McoDetail::Merge {
                        squash: Self::state(cx)
                            .read(cx)
                            .repo_states
                            .get(&id)
                            .and_then(|r| r.status.as_ref())
                            .is_some_and(|st| st.squash_msg_found),
                        source_branch: None,
                    },
                    Some(current_branch.clone()),
                    current.as_ref().and_then(|c| c.1.clone()),
                    None,
                    Some(current_branch.clone()),
                    None,
                ),
                ConflictKind::Rebase {
                    target_branch,
                    original_branch_tip,
                    ..
                } => {
                    let Some(snapshot) = rebase_snapshot else {
                        return;
                    };
                    (
                        McoDetail::Rebase {
                            base_branch: None,
                            commits: snapshot.commits,
                        },
                        Some(target_branch.clone()),
                        Some(original_branch_tip.clone()),
                        Some(snapshot.progress),
                        None,
                        Some(target_branch.clone()),
                    )
                }
                ConflictKind::CherryPick { target_branch } => {
                    let Some(snapshot) = cherry_pick_snapshot else {
                        return;
                    };
                    Self::state(cx).update(cx, |s, _| {
                        s.repo_state_mut(id).mco_undo = Some(McoUndo {
                            sha: snapshot.target_branch_undo_sha.clone(),
                            branch: target_branch.clone(),
                            kind: MultiCommitOperationKind::CherryPick,
                            count: snapshot.commits.len(),
                            source_branch: None,
                            branch_created: false,
                        });
                    });
                    (
                        McoDetail::CherryPick {
                            source_branch: None,
                            branch_created: false,
                            commits: snapshot.commits,
                        },
                        Some(target_branch.clone()),
                        None,
                        Some(snapshot.progress),
                        Some(target_branch.clone()),
                        None,
                    )
                }
            };
            Self::init_mco(id, detail, target, original_tip, McoStep::ShowConflicts, cx);
            Self::update_mco(id, cx, |m| {
                if let Some(p) = progress {
                    m.progress = p;
                }
                m.conflicts = McoConflicts {
                    our_branch: our,
                    their_branch: their,
                };
            });
            if !popup_open {
                Self::show_mco_popup(id, cx);
            }
            return;
        }
        // an operation we started: make sure the conflicts step is visible
        if let Some(mco) = mco
            && mco.step == McoStep::ShowProgress
            && !popup_open
            && !banner_is_conflicts
        {
            Self::set_mco_step(id, McoStep::ShowConflicts, cx);
            Self::show_mco_popup(id, cx);
        }
    }
}

fn last_retained_for_warn(mco: &Option<MultiCommitOperation>) -> Option<String> {
    match mco.as_ref().map(|m| &m.detail) {
        Some(McoDetail::Squash {
            last_retained_ref, ..
        })
        | Some(McoDetail::Reorder {
            last_retained_ref, ..
        }) => last_retained_ref.clone(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_state_follows_repository_markers() {
        let mut status = WorkingDirectoryStatus {
            branch: Some("main".into()),
            current_tip: Some("abc".into()),
            ..Default::default()
        };
        assert!(derive_conflict_state(&status, None).is_none());
        status.merge_head_found = true;
        let merge = derive_conflict_state(&status, None).unwrap();
        assert!(matches!(merge.kind, ConflictKind::Merge { .. }));
        // resolutions survive while the kind stays the same
        let mut with_resolution = merge.clone();
        with_resolution
            .manual_resolutions
            .insert("a.txt".into(), ManualConflictResolution::Ours);
        let again = derive_conflict_state(&status, Some(&with_resolution)).unwrap();
        assert_eq!(again.manual_resolutions.len(), 1);
        status.merge_head_found = false;
        status.cherry_pick_head_found = true;
        let pick = derive_conflict_state(&status, Some(&with_resolution)).unwrap();
        assert!(matches!(pick.kind, ConflictKind::CherryPick { .. }));
        assert!(pick.manual_resolutions.is_empty());
    }
}
