//! Compare to branch - GHD `ICompareState` (`lib/app-state.ts`) and
//! `_executeCompare` / `updateCompareToBranch` / `_updateCompareForm` in
//! `app-store.ts`: the History tab either shows the branch's history or the
//! commits the current branch is behind / ahead of another branch, with the
//! merge call to action.

use std::collections::HashMap;

use corvane_models::{AheadBehind, Commit, Mergeability};
use gpui_kit::{App, AsyncApp};
use tracing::warn;

use crate::dispatcher::Dispatcher;

/// `ComparisonMode`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonMode {
    /// Commits on the compared branch that are not on the current one.
    Behind,
    /// Commits on the current branch that are not on the compared one.
    Ahead,
}

/// `IDisplayHistory | ICompareBranch`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompareForm {
    History,
    Branch {
        branch: String,
        mode: ComparisonMode,
        ahead_behind: AheadBehind,
    },
}

/// `ICompareState`
#[derive(Clone, Debug, PartialEq)]
pub struct CompareState {
    pub form: CompareForm,
    /// `showBranchList`: the compare box is expanded into the branch list.
    pub show_branch_list: bool,
    /// The comparison commits (`commitSHAs` while in `Branch` mode).
    pub commits: Vec<Commit>,
    pub loading: bool,
    /// `mergeStatus` for the merge call to action; `None` while loading.
    pub merge_status: Option<Mergeability>,
    /// Ahead/behind of every other branch relative to the current one
    /// (`AheadBehindStore`), filled while the list is open.
    pub branch_counts: HashMap<String, AheadBehind>,
    pub counts_loaded: bool,
    /// Flag `825`: the repository's tags, loaded with the counts, offered
    /// in the list while filtering.
    pub tags: Vec<String>,
}

impl Default for CompareState {
    fn default() -> Self {
        Self {
            form: CompareForm::History,
            show_branch_list: false,
            commits: Vec::new(),
            loading: false,
            merge_status: None,
            branch_counts: HashMap::new(),
            counts_loaded: false,
            tags: Vec::new(),
        }
    }
}

impl CompareState {
    pub fn is_comparing(&self) -> bool {
        matches!(self.form, CompareForm::Branch { .. })
    }

    pub fn branch(&self) -> Option<&str> {
        match &self.form {
            CompareForm::Branch { branch, .. } => Some(branch),
            CompareForm::History => None,
        }
    }
}

impl Dispatcher {
    /// `updateCompareForm({ showBranchList })`
    pub fn set_compare_branch_list_visible(id: u64, visible: bool, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.compare.show_branch_list != visible {
                rs.compare.show_branch_list = visible;
                cx.notify();
            }
        });
        if visible {
            Self::load_compare_counts(id, cx);
        }
    }

    /// Ahead/behind counters for the compare branch list, one `rev-list
    /// --left-right --count` per branch.
    fn load_compare_counts(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (current, branches, loaded, with_tags) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let Some(info) = rs.info.as_ref() else { return };
            let Some(current) = info.current_branch() else {
                return;
            };
            (
                current.full_name.clone(),
                info.branches
                    .iter()
                    .filter(|b| b.full_name != current.full_name)
                    .map(|b| (b.name.clone(), b.full_name.clone()))
                    .collect::<Vec<_>>(),
                rs.compare.counts_loaded,
                s.flags.bool(crate::flags::ids::COMPARE_TAGS),
            )
        };
        if loaded {
            return;
        }
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).compare.counts_loaded = true);
        let task = cx.background_executor().spawn(async move {
            let tags = if with_tags {
                corvane_git::tag_names(&workdir).unwrap_or_else(|err| {
                    warn!(%err, "compare: could not list tags");
                    Vec::new()
                })
            } else {
                Vec::new()
            };
            let counts = branches
                .into_iter()
                .filter_map(|(name, full)| {
                    corvane_git::symmetric_ahead_behind(git.clone(), &workdir, &current, &full)
                        .ok()
                        .flatten()
                        .map(|ab| (name, ab))
                })
                .collect::<HashMap<String, AheadBehind>>();
            (counts, tags)
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let (counts, tags) = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let compare = &mut s.repo_state_mut(id).compare;
                    compare.branch_counts = counts;
                    compare.tags = tags;
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// `executeCompare({ kind: Compare, branch, comparisonMode })`
    pub fn compare_to_branch(id: u64, branch: String, mode: ComparisonMode, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some((current, _)) = Self::current_branch_and_tip_pub(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            rs.compare.loading = true;
            rs.compare.show_branch_list = false;
            cx.notify();
        });
        let branch_for_task = branch.clone();
        let current_for_task = current.clone();
        let task = cx.background_executor().spawn(async move {
            let ahead_behind = corvane_git::symmetric_ahead_behind(
                git.clone(),
                &workdir,
                &current_for_task,
                &branch_for_task,
            )?;
            let Some(ahead_behind) = ahead_behind else {
                return Ok(None);
            };
            let (from, to, count) = match mode {
                ComparisonMode::Behind => {
                    (&current_for_task, &branch_for_task, ahead_behind.behind)
                }
                ComparisonMode::Ahead => (&branch_for_task, &current_for_task, ahead_behind.ahead),
            };
            let commits = corvane_git::get_commits_in_range(&workdir, from, to, count as usize)?;
            let merge_status = if mode == ComparisonMode::Behind && ahead_behind.behind > 0 {
                corvane_git::determine_mergeability(
                    git,
                    &workdir,
                    &current_for_task,
                    &branch_for_task,
                )
                .ok()
            } else {
                None
            };
            Ok::<_, corvane_git::GitError>(Some((ahead_behind, commits, merge_status)))
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                let select = Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    rs.compare.loading = false;
                    match result {
                        Ok(Some((ahead_behind, commits, merge_status))) => {
                            rs.compare.form = CompareForm::Branch {
                                branch: branch.clone(),
                                mode,
                                ahead_behind,
                            };
                            rs.compare.commits = commits;
                            rs.compare.merge_status = merge_status;
                            let first = rs.compare.commits.first().map(|c| c.sha.clone());
                            let keep = rs
                                .selected_commits
                                .iter()
                                .all(|sha| rs.compare.commits.iter().any(|c| &c.sha == sha))
                                && !rs.selected_commits.is_empty();
                            cx.notify();
                            if keep { None } else { Some(first) }
                        }
                        Ok(None) => {
                            warn!(id, %branch, "compare: branch could not be resolved");
                            cx.notify();
                            None
                        }
                        Err(err) => {
                            warn!(id, %err, "compare failed");
                            cx.notify();
                            None
                        }
                    }
                });
                match select {
                    Some(Some(sha)) => Self::select_commits(id, vec![sha], cx),
                    Some(None) => Self::select_commits(id, Vec::new(), cx),
                    None => {}
                }
            });
        })
        .detach();
    }

    /// The Behind / Ahead tabs.
    pub fn set_comparison_mode(id: u64, mode: ComparisonMode, cx: &mut App) {
        let branch = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.compare.branch().map(str::to_string));
        if let Some(branch) = branch {
            Self::compare_to_branch(id, branch, mode, cx);
        }
    }

    /// `executeCompare({ kind: History })`: back to the branch's history.
    pub fn exit_compare(id: u64, cx: &mut App) {
        let was_comparing = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            let was = rs.compare.is_comparing();
            rs.compare.form = CompareForm::History;
            rs.compare.commits.clear();
            rs.compare.merge_status = None;
            rs.compare.show_branch_list = false;
            cx.notify();
            was
        });
        if was_comparing {
            let first = Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .and_then(|r| r.commits.first().map(|c| c.sha.clone()));
            Self::select_commits(id, first.into_iter().collect(), cx);
        }
    }

    /// After a refresh: re-run an active comparison and forget the cached counters.
    pub(crate) fn refresh_compare(id: u64, cx: &mut App) {
        let active = Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_state_mut(id);
            rs.compare.counts_loaded = false;
            match &rs.compare.form {
                CompareForm::Branch { branch, mode, .. } => Some((branch.clone(), *mode)),
                CompareForm::History => None,
            }
        });
        if let Some((branch, mode)) = active {
            Self::compare_to_branch(id, branch, mode, cx);
        }
    }

    /// Merge call to action: merge / squash-merge / rebase onto the compared branch.
    pub fn compare_merge_action(
        id: u64,
        kind: corvane_models::MultiCommitOperationKind,
        cx: &mut App,
    ) {
        if Self::refuse_merge_while_conflicted(id, cx) {
            return;
        }
        let Some(branch) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.compare.branch().map(str::to_string))
        else {
            return;
        };
        Self::exit_compare(id, cx);
        match kind {
            corvane_models::MultiCommitOperationKind::Rebase => {
                Self::start_rebase_flow(id, cx);
                Self::preview_rebase(id, branch.clone(), cx);
                Self::start_rebase(id, branch, false, cx);
            }
            corvane_models::MultiCommitOperationKind::Squash => {
                Self::merge_branch(id, branch, true, cx)
            }
            _ => Self::merge_branch(id, branch, false, cx),
        }
    }
}
