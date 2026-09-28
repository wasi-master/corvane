//! Dialog host: turns `AppState::popup` into a live dialog view, recreating
//! it only when the popup value changes.

mod add_existing;
mod app_dialogs;
pub(crate) mod branch_dialogs;
mod clone_repository;
mod create_repository;
mod discard_changes;
mod history_dialogs;
mod mco_dialogs;
mod preferences;
mod remote_dialogs;
mod repository_settings;
mod sign_in;
mod simple;

use corvane_core::{AppState, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

pub use add_existing::AddExistingRepositoryDialog;
pub use app_dialogs::{AboutDialog, ConfirmRemoveRepositoryDialog, IntegrationErrorDialog};
pub use branch_dialogs::{
    ConfirmOverwriteStashDialog, CreateBranchDialog, DeleteBranchDialog, MergeBranchDialog,
    RenameBranchDialog, StashAndSwitchBranchDialog,
};
pub use clone_repository::CloneRepositoryDialog;
pub use create_repository::CreateRepositoryDialog;
pub use discard_changes::DiscardChangesDialog;
pub use history_dialogs::{
    CheckoutCommitDialog, ConfirmDiscardStashDialog, CreateTagDialog, ResetToCommitDialog,
    WarnLocalChangesBeforeUndoDialog,
};
pub use mco_dialogs::{LocalChangesOverwrittenDialog, McoDialog, SquashCommitMessageDialog};
pub use preferences::PreferencesDialog;
pub use remote_dialogs::{
    ConfirmForcePushDialog, GenericGitAuthDialog, InitializeLfsDialog, PublishRepositoryDialog,
    PushNeedsPullDialog,
};
pub use repository_settings::RepositorySettingsDialog;
pub use sign_in::SignInDialog;
pub use simple::SimpleDialog;

pub struct DialogHost {
    state: Entity<AppState>,
    current: Option<(Popup, AnyView)>,
}

impl DialogHost {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            current: None,
        }
    }

    fn build(&self, popup: &Popup, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        let state = self.state.clone();
        match popup {
            Popup::Error { .. } | Popup::InstallGit { .. } => {
                cx.new(|_| SimpleDialog::new(popup.clone())).into()
            }
            Popup::AddExistingRepository { path } => cx
                .new(|cx| AddExistingRepositoryDialog::new(state, path.clone(), window, cx))
                .into(),
            Popup::CreateRepository { path } => cx
                .new(|cx| CreateRepositoryDialog::new(state, path.clone(), window, cx))
                .into(),
            Popup::CloneRepository { url } => cx
                .new(|cx| CloneRepositoryDialog::new(state, url.clone(), window, cx))
                .into(),
            Popup::SignIn { enterprise } => cx
                .new(|cx| SignInDialog::new(state, *enterprise, window, cx))
                .into(),
            Popup::DiscardChanges { repo, paths, all } => cx
                .new(|_| DiscardChangesDialog::new(*repo, paths.clone(), *all))
                .into(),
            Popup::ResetToCommit { repo, sha } => cx
                .new(|_| ResetToCommitDialog::new(*repo, sha.clone()))
                .into(),
            Popup::CheckoutCommit { repo, sha } => cx
                .new(|_| CheckoutCommitDialog::new(*repo, sha.clone()))
                .into(),
            Popup::CreateTag { repo, sha } => cx
                .new(|cx| CreateTagDialog::new(*repo, sha.clone(), window, cx))
                .into(),
            Popup::WarnLocalChangesBeforeUndo { repo } => cx
                .new(|_| WarnLocalChangesBeforeUndoDialog::new(*repo))
                .into(),
            Popup::CreateBranch {
                repo,
                target_sha,
                initial_name,
            } => cx
                .new(|cx| {
                    CreateBranchDialog::new(
                        state,
                        *repo,
                        target_sha.clone(),
                        initial_name.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::RenameBranch { repo, name } => cx
                .new(|cx| RenameBranchDialog::new(state, *repo, name.clone(), window, cx))
                .into(),
            Popup::DeleteBranch { repo, name } => cx
                .new(|_| DeleteBranchDialog::new(state, *repo, name.clone()))
                .into(),
            Popup::StashAndSwitchBranch { repo, branch } => cx
                .new(|_| StashAndSwitchBranchDialog::new(state, *repo, branch.clone()))
                .into(),
            Popup::ConfirmOverwriteStash { repo, branch } => cx
                .new(|_| ConfirmOverwriteStashDialog::new(*repo, branch.clone()))
                .into(),
            Popup::MergeBranch { repo, squash } => cx
                .new(|cx| MergeBranchDialog::new(state, *repo, *squash, window, cx))
                .into(),
            Popup::ConfirmDiscardStash { repo } => {
                cx.new(|_| ConfirmDiscardStashDialog::new(*repo)).into()
            }
            Popup::PublishRepository { repo } => cx
                .new(|cx| PublishRepositoryDialog::new(state, *repo, window, cx))
                .into(),
            Popup::PushNeedsPull { repo } => cx.new(|_| PushNeedsPullDialog::new(*repo)).into(),
            Popup::ConfirmForcePush {
                repo,
                upstream_branch,
            } => cx
                .new(|_| ConfirmForcePushDialog::new(*repo, upstream_branch.clone()))
                .into(),
            Popup::GenericGitAuthentication {
                repo,
                remote_url,
                host,
                username,
                retry,
            } => cx
                .new(|cx| {
                    GenericGitAuthDialog::new(
                        *repo,
                        remote_url.clone(),
                        host.clone(),
                        username.clone(),
                        retry.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::InitializeLFS { repos } => cx
                .new(|_| InitializeLfsDialog::new(state, repos.clone()))
                .into(),
            Popup::MultiCommitOperation { repo } => {
                cx.new(|cx| McoDialog::new(state, *repo, window, cx)).into()
            }
            Popup::LocalChangesOverwritten { repo, retry, files } => cx
                .new(|_| {
                    LocalChangesOverwrittenDialog::new(state, *repo, retry.clone(), files.clone())
                })
                .into(),
            Popup::SquashCommitMessage {
                repo,
                to_squash,
                onto,
                summary,
                description,
                count,
            } => cx
                .new(|cx| {
                    SquashCommitMessageDialog::new(
                        *repo,
                        to_squash.clone(),
                        onto.clone(),
                        summary.clone(),
                        description.clone(),
                        *count,
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::Preferences { tab } => cx
                .new(|cx| PreferencesDialog::new(state, *tab, window, cx))
                .into(),
            Popup::RepositorySettings { repo, tab } => cx
                .new(|cx| RepositorySettingsDialog::new(state, *repo, *tab, window, cx))
                .into(),
            Popup::ConfirmRemoveRepository { repo } => cx
                .new(|_| ConfirmRemoveRepositoryDialog::new(state, *repo))
                .into(),
            Popup::About { version } => cx.new(|_| AboutDialog::new(version.clone())).into(),
            Popup::ExternalEditorError { .. } | Popup::ShellError { .. } => cx
                .new(|_| IntegrationErrorDialog::new(popup.clone()))
                .into(),
        }
    }
}

impl Render for DialogHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let popup = self.state.read(cx).popup.clone();
        match popup {
            None => {
                self.current = None;
                div()
            }
            Some(popup) => {
                let stale = self
                    .current
                    .as_ref()
                    .map(|(p, _)| *p != popup)
                    .unwrap_or(true);
                if stale {
                    let view = self.build(&popup, window, cx);
                    self.current = Some((popup, view));
                }
                let view = self.current.as_ref().map(|(_, v)| v.clone());
                div().children(view)
            }
        }
    }
}
