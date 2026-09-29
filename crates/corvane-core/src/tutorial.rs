//! Onboarding tutorial - GHD `models/tutorial-step.ts`,
//! `lib/stores/helpers/tutorial-assessor.ts` (`OnboardingTutorialAssessor`),
//! `lib/stores/helpers/create-tutorial-repository.ts` and the tutorial
//! methods of `app-store.ts` (`_createTutorialRepository`,
//! `_addTutorialRepository`, `_resumeTutorial`, `_pauseTutorial`,
//! `_skipPickEditorTutorialStep`, `_markPullRequestTutorialStepAsComplete`,
//! `_markTutorialCompletionAsAnnounced`).
//!
//! The assessor's skip / pull-request / paused flags live in the settings
//! (GHD: localStorage); "announced" is per session as in GHD. The step is
//! computed from the repository state whenever it is read instead of being
//! cached in the app state, and a pull request seen for the branch counts as
//! the pull request step without persisting that.

use std::path::{Path, PathBuf};

use corvane_github::Client;
use corvane_models::{Account, Repository, Tip};
use gpui_kit::{App, AsyncApp};
use tracing::info;

use crate::dispatcher::Dispatcher;
use crate::state::{AppState, Foldout, Popup};

/// GHD `TutorialStep`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TutorialStep {
    NotApplicable,
    PickEditor,
    CreateBranch,
    EditFile,
    MakeCommit,
    PushBranch,
    OpenPullRequest,
    AllDone,
    Paused,
    Announced,
}

impl TutorialStep {
    /// `orderedTutorialSteps`
    pub const ORDERED: [TutorialStep; 8] = [
        TutorialStep::PickEditor,
        TutorialStep::CreateBranch,
        TutorialStep::EditFile,
        TutorialStep::MakeCommit,
        TutorialStep::PushBranch,
        TutorialStep::OpenPullRequest,
        TutorialStep::AllDone,
        TutorialStep::Announced,
    ];

    /// `isValidTutorialStep`: the tutorial panel is showing.
    pub fn is_valid(self) -> bool {
        !matches!(self, TutorialStep::NotApplicable | TutorialStep::Paused)
    }

    /// Position in [`Self::ORDERED`].
    pub fn index(self) -> Option<usize> {
        Self::ORDERED.iter().position(|s| *s == self)
    }

    /// `isStepComplete`: `step` comes before `self` (the current step).
    pub fn completes(self, step: TutorialStep) -> bool {
        matches!((step.index(), self.index()), (Some(a), Some(b)) if a < b)
    }

    /// Names as in GHD (`PickEditor`, `pick-editor` or `pickeditor`), for the
    /// `CORVANE_POPUP=tutorial:<step>` hook.
    pub fn parse(name: &str) -> Option<Self> {
        let key: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        Some(match key.as_str() {
            "notapplicable" => Self::NotApplicable,
            "pickeditor" => Self::PickEditor,
            "createbranch" => Self::CreateBranch,
            "editfile" => Self::EditFile,
            "makecommit" => Self::MakeCommit,
            "pushbranch" => Self::PushBranch,
            "openpullrequest" => Self::OpenPullRequest,
            "alldone" => Self::AllDone,
            "paused" => Self::Paused,
            "announced" => Self::Announced,
            _ => return None,
        })
    }
}

/// What `OnboardingTutorialAssessor.getCurrentStep` looks at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TutorialInput {
    pub is_tutorial_repository: bool,
    pub paused: bool,
    /// A resolved external editor, or "I have an editor" was clicked.
    pub editor_installed: bool,
    pub current_branch: Option<String>,
    pub default_branch: Option<String>,
    pub changed_files: usize,
    /// The tip commit has a parent (`hasMultipleCommits`).
    pub tip_has_parent: bool,
    /// `aheadBehind.ahead`, `None` without an upstream.
    pub ahead: Option<u32>,
    /// A pull request was seen for the branch, or the step was skipped.
    pub pull_request_done: bool,
    pub announced: bool,
}

/// `getCurrentStep`
pub fn assess(i: &TutorialInput) -> TutorialStep {
    if !i.is_tutorial_repository {
        TutorialStep::NotApplicable
    } else if i.paused {
        TutorialStep::Paused
    } else if !i.editor_installed {
        TutorialStep::PickEditor
    } else if !matches!(
        (&i.current_branch, &i.default_branch),
        (Some(current), Some(default)) if current != default
    ) {
        TutorialStep::CreateBranch
    } else if !(i.tip_has_parent || i.changed_files > 0) {
        TutorialStep::EditFile
    } else if !i.tip_has_parent {
        TutorialStep::MakeCommit
    } else if i.ahead != Some(0) {
        TutorialStep::PushBranch
    } else if !i.pull_request_done {
        TutorialStep::OpenPullRequest
    } else if !i.announced {
        TutorialStep::AllDone
    } else {
        TutorialStep::Announced
    }
}

/// `InitialReadmeContents`, with Corvane's name.
pub const INITIAL_README: &str = "# Welcome to Corvane!\n\n\
     This is your README. READMEs are where you can communicate what your project is and how to \
     use it.\n\n\
     Write your name on line 6, save it, and then head back to Corvane.\n";

/// GHD's tutorial repository name.
pub const TUTORIAL_REPOSITORY_NAME: &str = "desktop-tutorial";

impl AppState {
    /// The editor the tutorial and "Open in …" use (`resolvedExternalEditor`).
    pub fn resolved_editor_name(&self) -> Option<String> {
        if self.settings.use_custom_editor && self.settings.custom_editor.is_some() {
            return Some("Custom Editor".to_string());
        }
        self.settings
            .external_editor
            .clone()
            .or_else(|| self.editors.first().map(|e| e.name.clone()))
    }

    /// `currentOnboardingTutorialStep` for repository `id`.
    pub fn tutorial_step(&self, id: u64) -> TutorialStep {
        let Some(repo) = self.repository(id) else {
            return TutorialStep::NotApplicable;
        };
        if !repo.is_tutorial_repository {
            return TutorialStep::NotApplicable;
        }
        if let Some(step) = self.tutorial_step_override {
            return step;
        }
        let rs = self.repo_states.get(&id);
        let info = rs.and_then(|rs| rs.info.as_ref());
        let branch = info.and_then(|i| match &i.tip {
            Tip::Valid { branch } => Some(branch),
            _ => None,
        });
        let tip_has_parent = branch.and_then(|b| b.tip.as_ref()).is_some_and(|tip| {
            rs.is_some_and(|rs| {
                rs.commits
                    .iter()
                    .find(|c| &c.sha == tip)
                    .is_some_and(|c| c.parents.iter().any(|p| !p.is_empty()))
            })
        });
        assess(&TutorialInput {
            is_tutorial_repository: true,
            paused: self.settings.tutorial_paused,
            editor_installed: self.settings.tutorial_install_editor_skipped
                || self.resolved_editor_name().is_some(),
            current_branch: branch.map(|b| b.name.clone()),
            default_branch: rs.and_then(|rs| rs.default_branch.clone()),
            changed_files: rs
                .and_then(|rs| rs.status.as_ref())
                .map_or(0, |st| st.files.len()),
            tip_has_parent,
            ahead: rs.and_then(|rs| rs.ahead_behind).map(|ab| ab.ahead),
            pull_request_done: self.settings.tutorial_pull_request_step_complete
                || self.current_pull_request(id).is_some(),
            announced: self.tutorial_announced,
        })
    }

    /// The step for the selected repository.
    pub fn selected_tutorial_step(&self) -> TutorialStep {
        self.selected
            .map_or(TutorialStep::NotApplicable, |id| self.tutorial_step(id))
    }
}

impl Dispatcher {
    /// `showCreateTutorialRepositoryPopup`: GitHub.com's account, else the first.
    pub fn show_create_tutorial_repository(cx: &mut App) {
        let account = {
            let s = Self::state(cx).read(cx);
            s.accounts
                .iter()
                .find(|a| a.endpoint == "https://api.github.com")
                .or(s.accounts.first())
                .cloned()
        };
        if let Some(account) = account {
            Self::show_popup(
                Popup::CreateTutorialRepository {
                    account,
                    progress: None,
                },
                cx,
            );
        }
    }

    /// `_createTutorialRepository`: create `desktop-tutorial` on the account,
    /// initialize it next to the other clones with the README commit, push
    /// it, then add it as the tutorial repository.
    pub fn create_tutorial_repository(account: Account, cx: &mut App) {
        let Some(git) = Self::state(cx).read(cx).git.clone() else {
            Self::show_error("Git is not available", "Install git and retry.", cx);
            return;
        };
        let Some(token) = corvane_platform::keychain::token(&account.host(), &account.login)
            .ok()
            .flatten()
        else {
            Self::close_popup(cx);
            Self::show_error(
                "Failed creating the tutorial repository.",
                "The account's token is missing from the keychain. Sign in again.",
                cx,
            );
            return;
        };
        let dir = Self::state(cx)
            .read(cx)
            .settings
            .clone_dir
            .clone()
            .unwrap_or_else(corvane_platform::paths::default_clone_dir);
        let path = dir.join(TUTORIAL_REPOSITORY_NAME);
        let askpass = Self::askpass_env(cx);
        let (tx, rx) = async_channel::unbounded::<(String, u8, Option<String>)>();
        let friendly = account.host();
        let endpoint = corvane_github::Endpoint::from_api_base(&account.endpoint);
        let work_path = path.clone();
        let task = cx.background_executor().spawn(async move {
            let progress = |title: &str, value: f32, detail: Option<String>| {
                let _ = tx.send_blocking((title.to_string(), (value * 100.) as u8, detail));
            };
            create_tutorial_repository(
                &git,
                Client::new(endpoint, token),
                &friendly,
                &work_path,
                askpass.as_ref(),
                &progress,
            )
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(progress) = rx.recv().await {
                cx.update(|cx| {
                    Self::state(cx).update(cx, |s, cx| {
                        if let Some(Popup::CreateTutorialRepository { progress: p, .. }) =
                            &mut s.popup
                        {
                            *p = Some(progress);
                            cx.notify();
                        }
                    })
                });
            }
            let result = task.await;
            cx.update(|cx| {
                // `finally { _closePopup(CreateTutorialRepository) }`
                if matches!(
                    Self::state(cx).read(cx).popup,
                    Some(Popup::CreateTutorialRepository { .. })
                ) {
                    Self::close_popup(cx);
                }
                match result {
                    Ok(github) => Self::add_tutorial_repository(path, github, cx),
                    Err(message) => {
                        Self::show_error("Failed creating the tutorial repository.", message, cx)
                    }
                }
            });
        })
        .detach();
    }

    /// `_addTutorialRepository` + `onNewTutorialRepository`.
    fn add_tutorial_repository(
        path: PathBuf,
        github: corvane_models::GitHubRepository,
        cx: &mut App,
    ) {
        let id = Self::state(cx).update(cx, |s, cx| {
            let existing = s.repositories.iter().position(|r| r.path == path);
            let id = match existing {
                Some(ix) => s.repositories[ix].id,
                None => {
                    let id = s.store_next_repository_id();
                    s.repositories.push(Repository::new(id, path.clone()));
                    id
                }
            };
            if let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) {
                repo.is_tutorial_repository = true;
                repo.github = Some(github);
            }
            crate::dispatcher::persist_repositories(s);
            s.tutorial_announced = false;
            cx.notify();
            id
        });
        info!(id, path = %path.display(), "added the tutorial repository");
        Self::update_settings(cx, |s| {
            s.tutorial_install_editor_skipped = false;
            s.tutorial_pull_request_step_complete = false;
            s.tutorial_paused = false;
        });
        Self::select_repository(id, cx);
    }

    /// `_resumeTutorial`
    pub fn resume_tutorial(cx: &mut App) {
        Self::state(cx).update(cx, |s, _| {
            if s.tutorial_step_override == Some(TutorialStep::Paused) {
                s.tutorial_step_override = None;
            }
        });
        Self::update_settings(cx, |s| s.tutorial_paused = false);
    }

    /// `_pauseTutorial` (Exit Tutorial → back to the blank slate).
    pub fn pause_tutorial(cx: &mut App) {
        Self::state(cx).update(cx, |s, _| s.tutorial_step_override = None);
        Self::update_settings(cx, |s| s.tutorial_paused = true);
    }

    /// `_skipPickEditorTutorialStep` ("I have an editor", Skip).
    pub fn skip_pick_editor_tutorial_step(cx: &mut App) {
        Self::update_settings(cx, |s| s.tutorial_install_editor_skipped = true);
    }

    /// `_markPullRequestTutorialStepAsComplete`
    pub fn mark_pull_request_tutorial_step_complete(cx: &mut App) {
        Self::update_settings(cx, |s| s.tutorial_pull_request_step_complete = true);
    }

    /// `_markTutorialCompletionAsAnnounced`
    pub fn mark_tutorial_completion_announced(cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if !s.tutorial_announced {
                s.tutorial_announced = true;
                cx.notify();
            }
        });
    }

    /// App `onExitTutorial`: with only the tutorial repository, confirm and
    /// go back to the blank slate; otherwise open the repository list.
    pub fn exit_tutorial(cx: &mut App) {
        let (only_repository, step) = {
            let s = Self::state(cx).read(cx);
            (s.repositories.len() == 1, s.selected_tutorial_step())
        };
        if only_repository && step.is_valid() {
            Self::show_popup(Popup::ConfirmExitTutorial, cx);
        } else {
            Self::toggle_foldout(Foldout::Repository, cx);
        }
    }

    /// `getCurrentStep`'s side effect: selecting another repository while
    /// the tutorial is paused un-pauses it (GHD #8341).
    pub(crate) fn resume_tutorial_on_other_repository(id: u64, cx: &mut App) {
        let (paused, tutorial) = {
            let s = Self::state(cx).read(cx);
            (
                s.settings.tutorial_paused,
                s.repository(id).is_some_and(|r| r.is_tutorial_repository),
            )
        };
        if paused && !tutorial {
            Self::resume_tutorial(cx);
        }
    }
}

impl AppState {
    /// The next repository id from the store (monotonic), else max + 1.
    fn store_next_repository_id(&self) -> u64 {
        use crate::persistence::StoreExt;
        self.store
            .next_repository_id()
            .unwrap_or_else(|_| self.repositories.iter().map(|r| r.id).max().unwrap_or(0) + 1)
    }
}

/// `createTutorialRepository` (background): API repository, `git init`,
/// README commit, `origin`, `push -u`. Returns the GitHub repository.
fn create_tutorial_repository(
    git: &std::sync::Arc<corvane_git::GitBinary>,
    client: Client,
    friendly_endpoint: &str,
    path: &Path,
    askpass: Option<&corvane_git::remote_ops::AskpassEnv>,
    progress: &dyn Fn(&str, f32, Option<String>),
) -> Result<corvane_models::GitHubRepository, String> {
    progress(
        &format!("Creating repository on {friendly_endpoint}"),
        0.,
        None,
    );
    if path.exists() {
        return Err(format!(
            "The path '{}' already exists. Please move it out of the way, or remove it, and \
             then try again.",
            path.display()
        ));
    }
    let repo = client
        .create_repository(
            None,
            TUTORIAL_REPOSITORY_NAME,
            "GitHub Desktop tutorial repository",
            true,
        )
        .map_err(|err| {
            let text = err.to_string();
            if text.contains("name already exists") {
                format!(
                    "You already have a repository named \"{TUTORIAL_REPOSITORY_NAME}\" on your \
                     account at {friendly_endpoint}.\n\nPlease delete the repository and try again."
                )
            } else {
                text
            }
        })?;
    let branch = repo
        .default_branch
        .clone()
        .unwrap_or_else(|| corvane_git::configured_default_branch(git.clone()));
    progress("Initializing local repository", 0.2, None);
    corvane_git::init_repository(
        git.clone(),
        corvane_git::InitOptions {
            path: path.to_path_buf(),
            default_branch: Some(branch.clone()),
            description: None,
            readme: false,
            gitignore: None,
            license: None,
            git_attributes: None,
        },
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(path.join("README.md"), INITIAL_README).map_err(|e| e.to_string())?;
    corvane_git::add_paths(git.clone(), path, &["README.md"]).map_err(|e| e.to_string())?;
    corvane_git::commit(
        git.clone(),
        path,
        "Initial commit",
        &corvane_git::CommitOptions::default(),
    )
    .map_err(|e| e.to_string())?;
    corvane_git::add_remote(git.clone(), path, "origin", &repo.clone_url)
        .map_err(|e| e.to_string())?;
    let title = format!("Pushing repository to {friendly_endpoint}");
    progress(&title, 0.3, None);
    corvane_git::push(
        git.clone(),
        path,
        "origin",
        &branch,
        None,
        &[],
        false,
        askpass,
        &mut |percent, text| progress(&title, 0.3 + percent * 0.6, Some(text)),
    )
    .map_err(|e| e.to_string())?;
    progress("Finalizing tutorial repository", 0.9, None);
    Ok(repo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tutorial() -> TutorialInput {
        TutorialInput {
            is_tutorial_repository: true,
            editor_installed: true,
            current_branch: Some("main".into()),
            default_branch: Some("main".into()),
            ..Default::default()
        }
    }

    #[test]
    fn steps_follow_the_assessor_order() {
        assert_eq!(
            assess(&TutorialInput::default()),
            TutorialStep::NotApplicable
        );
        let mut i = tutorial();
        i.paused = true;
        assert_eq!(assess(&i), TutorialStep::Paused);
        i.paused = false;
        i.editor_installed = false;
        assert_eq!(assess(&i), TutorialStep::PickEditor);
        i.editor_installed = true;
        assert_eq!(assess(&i), TutorialStep::CreateBranch);
        i.current_branch = Some("my-branch".into());
        assert_eq!(assess(&i), TutorialStep::EditFile);
        i.changed_files = 1;
        assert_eq!(assess(&i), TutorialStep::MakeCommit);
        i.tip_has_parent = true;
        i.changed_files = 0;
        // committed, never pushed (no upstream)
        assert_eq!(assess(&i), TutorialStep::PushBranch);
        i.ahead = Some(1);
        assert_eq!(assess(&i), TutorialStep::PushBranch);
        i.ahead = Some(0);
        assert_eq!(assess(&i), TutorialStep::OpenPullRequest);
        i.pull_request_done = true;
        assert_eq!(assess(&i), TutorialStep::AllDone);
        i.announced = true;
        assert_eq!(assess(&i), TutorialStep::Announced);
    }

    #[test]
    fn detached_or_unknown_default_branch_asks_for_a_branch() {
        let mut i = tutorial();
        i.current_branch = None;
        assert_eq!(assess(&i), TutorialStep::CreateBranch);
        i.current_branch = Some("x".into());
        i.default_branch = None;
        assert_eq!(assess(&i), TutorialStep::CreateBranch);
    }

    #[test]
    fn completion_and_validity() {
        assert!(TutorialStep::MakeCommit.completes(TutorialStep::PickEditor));
        assert!(!TutorialStep::MakeCommit.completes(TutorialStep::MakeCommit));
        assert!(!TutorialStep::PickEditor.completes(TutorialStep::OpenPullRequest));
        assert!(TutorialStep::AllDone.is_valid());
        assert!(!TutorialStep::Paused.is_valid());
        assert!(!TutorialStep::NotApplicable.is_valid());
        assert_eq!(
            TutorialStep::parse("edit-file"),
            Some(TutorialStep::EditFile)
        );
        assert_eq!(TutorialStep::parse("AllDone"), Some(TutorialStep::AllDone));
        assert_eq!(TutorialStep::parse("nope"), None);
    }
}
