//! Dialog host: turns `AppState::popup` into a live dialog view, recreating
//! it only when the popup value changes.

mod add_existing;
mod clone_repository;
mod create_repository;
mod discard_changes;
mod sign_in;
mod simple;

use corvane_core::{AppState, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

pub use add_existing::AddExistingRepositoryDialog;
pub use clone_repository::CloneRepositoryDialog;
pub use create_repository::CreateRepositoryDialog;
pub use discard_changes::DiscardChangesDialog;
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
