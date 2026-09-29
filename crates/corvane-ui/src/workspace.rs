//! Root view: title bar, toolbar, resizable sidebar + content, foldouts, dialogs.

use std::cell::Cell;
use std::rc::Rc;

use corvane_core::{AppState, Dispatcher, Section};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::banner::banner_bar;
use crate::branch_list::BranchFoldout;
use crate::changes::ChangesSidebar;
use crate::ci_check_popover::CiCheckPopover;
use crate::cloning_view::cloning_view;
use crate::dialogs::DialogHost;
use crate::diff_view::{DiffSource, DiffView, diff_header};
use crate::foldout::{FoldoutPanels, foldout_layer};
use crate::history::HistorySidebar;
use crate::no_changes::{SuggestedAction, no_changes};
use crate::no_repositories::no_repositories;
use crate::repository_list::RepositoryFoldout;
use crate::selected_commit::SelectedCommitView;
use crate::stash_view::StashDiffViewer;
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::title_bar::title_bar;
use crate::toolbar::{
    ToolbarResize, toolbar, toolbar_models, toolbar_widths, worktree_button_visible,
};
use crate::welcome::WelcomeView;
use crate::worktree_list::WorktreeFoldout;

pub struct Workspace {
    focus_handle: FocusHandle,
    state: Entity<AppState>,
    section: Section,
    sidebar_width: Pixels,
    resizable: Entity<ResizableState>,
    changes: Entity<ChangesSidebar>,
    history: Entity<HistorySidebar>,
    selected_commit: Entity<SelectedCommitView>,
    stash_view: Entity<StashDiffViewer>,
    repository_foldout: Entity<RepositoryFoldout>,
    branch_foldout: Entity<BranchFoldout>,
    worktree_foldout: Entity<WorktreeFoldout>,
    dialogs: Entity<DialogHost>,
    diff_view: Entity<DiffView>,
    welcome: Option<Entity<WelcomeView>>,
    /// The branch button's PR badge rectangle (anchor of the CI popover).
    pr_badge_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Resize handles of the worktree and branch buttons.
    toolbar_resize: Rc<ToolbarResize>,
    ci_popover: Entity<CiCheckPopover>,
}

impl Workspace {
    pub fn new(
        state: Entity<AppState>,
        sidebar_width: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        // GHD: `ipcRenderer.on('focus')` → refreshRepository.
        // `_setAppFocusState` pauses the pull request updater while blurred.
        cx.observe_window_activation(window, |_, window, cx| {
            let active = window.is_window_active();
            Dispatcher::set_app_focus_state(active, cx);
            if active {
                Dispatcher::refresh_selected(cx);
            }
        })
        .detach();
        // When a dialog or foldout closes, put keyboard focus back on the root so
        // menu actions stay available (GPUI disables items whose action has no handler
        // in the focus path).
        cx.observe_in(&state, window, |this, state, window, cx| {
            let s = state.read(cx);
            let overlay_open = s.popup.is_some() || s.foldout.is_some();
            if !overlay_open && !this.focus_handle.contains_focused(window, cx) {
                window.focus(&this.focus_handle, cx);
            }
        })
        .detach();

        let resizable = cx.new(|_| ResizableState::default());
        cx.subscribe(&resizable, |this, state, _: &ResizablePanelEvent, cx| {
            if let Some(width) = state.read(cx).sizes().first().copied()
                && width != this.sidebar_width
            {
                this.sidebar_width = width;
                Dispatcher::update_settings(cx, |s| s.sidebar_width = f32::from(width));
                cx.notify();
            }
        })
        .detach();

        let changes = cx.new(|cx| ChangesSidebar::new(state.clone(), window, cx));
        let history = cx.new(|cx| HistorySidebar::new(state.clone(), window, cx));
        let selected_commit = cx.new(|cx| SelectedCommitView::new(state.clone(), cx));
        let stash_view = cx.new(|cx| StashDiffViewer::new(state.clone(), cx));
        let repository_foldout = cx.new(|cx| RepositoryFoldout::new(state.clone(), window, cx));
        let branch_foldout = cx.new(|cx| BranchFoldout::new(state.clone(), window, cx));
        let worktree_foldout = cx.new(|cx| WorktreeFoldout::new(state.clone(), window, cx));
        let diff_view = cx.new(|cx| DiffView::new(state.clone(), DiffSource::WorkingDirectory, cx));
        let dialogs = cx.new(|cx| DialogHost::new(state.clone(), cx));
        let pr_badge_bounds: Rc<Cell<Bounds<Pixels>>> = Rc::new(Cell::new(Bounds::default()));
        let ci_popover =
            cx.new(|cx| CiCheckPopover::new(state.clone(), pr_badge_bounds.clone(), cx));
        let welcome = (!state.read(cx).settings.welcome_completed)
            .then(|| cx.new(|cx| WelcomeView::new(state.clone(), window, cx)));
        window.focus(&focus_handle, cx);

        Self {
            focus_handle,
            state,
            section: Section::Changes,
            sidebar_width: sidebar_width.max(SIDEBAR_MIN_WIDTH),
            resizable,
            changes,
            history,
            selected_commit,
            stash_view,
            repository_foldout,
            branch_foldout,
            worktree_foldout,
            pr_badge_bounds,
            toolbar_resize: Rc::new(ToolbarResize::default()),
            ci_popover,
            dialogs,
            diff_view,
            welcome,
        }
    }

    /// View › Go to Summary (`focusCommitSummary`).
    pub fn focus_commit_summary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_section(Section::Changes, cx);
        self.changes
            .update(cx, |changes, cx| changes.focus_summary(window, cx));
    }

    /// Edit › Find: focus the changes filter (`selectAllInput` in GHD).
    pub fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.section() == Section::Changes {
            self.changes
                .update(cx, |changes, cx| changes.focus_filter(window, cx));
        }
    }

    /// View › Show/Hide Changes Filter.
    pub fn toggle_changes_filter(&mut self, cx: &mut Context<Self>) {
        self.changes
            .update(cx, |changes, cx| changes.toggle_filter(cx));
    }

    pub fn section(&self) -> Section {
        self.section
    }

    /// `View › Show Repository List` (⌘T): open the foldout and focus its filter.
    pub fn show_repository_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Dispatcher::toggle_foldout(corvane_core::Foldout::Repository, cx);
        if self.state.read(cx).foldout == Some(corvane_core::Foldout::Repository) {
            self.repository_foldout
                .update(cx, |f, cx| f.focus_filter(window, cx));
        }
    }

    /// `View › Show Branches List` (⌘B): open the foldout and focus its filter.
    pub fn show_branches_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Dispatcher::toggle_foldout(corvane_core::Foldout::Branch, cx);
        if self.state.read(cx).foldout == Some(corvane_core::Foldout::Branch) {
            self.branch_foldout
                .update(cx, |f, cx| f.focus_filter(window, cx));
        }
    }

    /// `View › Show Worktrees List` (⌥⌘W): open the foldout and focus its filter.
    pub fn show_worktrees_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Dispatcher::toggle_foldout(corvane_core::Foldout::Worktree, cx);
        if self.state.read(cx).foldout == Some(corvane_core::Foldout::Worktree) {
            self.worktree_foldout
                .update(cx, |f, cx| f.focus_filter(window, cx));
        }
    }

    /// Branch › Compare to Branch (⇧⌘B): History tab with the compare box focused.
    pub fn show_compare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_section(Section::History, cx);
        Dispatcher::close_foldout(cx);
        self.history.update(cx, |h, cx| h.focus_compare(window, cx));
    }

    pub fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            if let Some(id) = self.state.read(cx).selected {
                Dispatcher::show_section(id, section, cx);
            }
            cx.notify();
        }
    }

    fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let selected = match self.section {
            Section::Changes => 0,
            Section::History => 1,
        };
        let this = cx.entity();
        div()
            .id("repository-sidebar")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .border_r_1()
            .border_color(t.box_border)
            .bg(t.background)
            .child(tab_bar(
                vec![
                    TabModel {
                        id: "tab-changes",
                        label: "Changes".into(),
                        count: self
                            .state
                            .read(cx)
                            .selected_state()
                            .map(|rs| rs.changed_files())
                            .filter(|n| *n > 0),
                    },
                    TabModel {
                        id: "tab-history",
                        label: "History".into(),
                        count: None,
                    },
                ],
                selected,
                move |ix, _, cx| {
                    this.update(cx, |ws, cx| {
                        ws.set_section(
                            if ix == 0 {
                                Section::Changes
                            } else {
                                Section::History
                            },
                            cx,
                        )
                    });
                },
                cx,
            ))
            .child(div().flex_1().min_h_0().child(match self.section {
                Section::Changes => self.changes.clone().into_any_element(),
                Section::History => self.history.clone().into_any_element(),
            }))
    }

    fn content(&self, cx: &Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let repo = state.selected_repository();
        let has_github = repo.and_then(|r| r.github.as_ref()).is_some();
        let rs = state.selected_state();
        let selected_change = rs.and_then(|r| {
            let path = r.selected_file.as_ref()?;
            r.status
                .as_ref()?
                .files
                .iter()
                .find(|f| &f.path == path)
                .cloned()
        });
        let showing_stash = rs.is_some_and(|r| r.showing_stash);
        let multi_selected = rs.map(|r| r.selected_files.len()).unwrap_or(0);
        match self.section {
            Section::Changes if showing_stash => self.stash_view.clone().into_any_element(),
            Section::Changes if multi_selected > 1 => {
                crate::no_changes::multiple_selection(multi_selected, cx).into_any_element()
            }
            Section::Changes if selected_change.is_some() => {
                let file = selected_change.unwrap();
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .child(diff_header(
                        &file.path,
                        file.status.kind,
                        &self.diff_view,
                        cx,
                    ))
                    .child(self.diff_view.clone())
                    .into_any_element()
            }
            Section::Changes => {
                let (repo_id, repo_path, editor_label) = {
                    let s = self.state.read(cx);
                    let repo = s.selected_repository();
                    (
                        repo.map(|r| r.id),
                        repo.map(|r| r.path.clone()),
                        s.editor_label(),
                    )
                };
                let path = repo_path.clone().unwrap_or_default();
                let mut actions = vec![
                    SuggestedAction {
                        id: "suggested-editor",
                        on_click: std::rc::Rc::new({
                            let path = path.clone();
                            move |_, cx| Dispatcher::open_in_editor(path.clone(), cx)
                        }),
                        title: format!("Open the repository in {editor_label}").into(),
                        description: Some("Select your editor in Settings".into()),
                        hint: "Repository menu or".into(),
                        keys: &["⌘", "⇧", "A"],
                        button_label: format!("Open in {editor_label}").into(),
                        primary: false,
                    },
                    SuggestedAction {
                        id: "suggested-finder",
                        on_click: std::rc::Rc::new({
                            let path = path.clone();
                            move |_, cx| Dispatcher::show_in_finder(&path, cx)
                        }),
                        title: "View the files of your repository in Finder".into(),
                        description: None,
                        hint: "Repository menu or".into(),
                        keys: &["⌘", "⇧", "F"],
                        button_label: "Show in Finder".into(),
                        primary: false,
                    },
                ];
                if has_github && let Some(id) = repo_id {
                    actions.push(SuggestedAction {
                        id: "suggested-github",
                        on_click: std::rc::Rc::new(move |_, cx| Dispatcher::view_on_github(id, cx)),
                        title: "Open the repository page on GitHub in your browser".into(),
                        description: None,
                        hint: "Repository menu or".into(),
                        keys: &["⌘", "⇧", "G"],
                        button_label: "View on GitHub".into(),
                        primary: false,
                    });
                }
                no_changes(actions, cx).into_any_element()
            }
            Section::History => self.selected_commit.clone().into_any_element(),
        }
    }

    fn repository_view(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .border_t_1()
            .border_color(t.box_border)
            .child(
                h_resizable("repository")
                    .with_state(&self.resizable)
                    // GHD's 6 px handle is invisible; the sidebar's own border is the seam.
                    .with_handle_appearance(std::rc::Rc::new(|_, _, _| {
                        Some(div().into_any_element())
                    }))
                    .child(
                        resizable_panel()
                            .size(self.sidebar_width)
                            .size_range(SIDEBAR_MIN_WIDTH..px(900.))
                            .child(crate::active_resizable::active_resizable(
                                "repository-sidebar-resizable",
                                &self.resizable,
                                None,
                                self.sidebar(cx),
                            )),
                    )
                    .child(resizable_panel().child(self.content(cx))),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The section is per repository (`repositoryState.selectedSection`), so
        // dispatcher-driven switches (Amend Commit…, undo) and repository
        // changes land here.
        if let Some(section) = self.state.read(cx).selected_state().map(|rs| rs.section)
            && section != self.section
        {
            self.section = section;
        }
        let t = cx.ghd();
        let welcome_done = self.state.read(cx).settings.welcome_completed;
        if welcome_done {
            self.welcome = None;
        }
        let (
            buttons,
            foldout,
            popup,
            has_repos,
            cloning,
            banner,
            worktree_button,
            ci_popover,
            (_, worktree_width, branch_width),
        ) = {
            let state = self.state.read(cx);
            let widths = toolbar_widths(
                state,
                window.viewport_size().width,
                self.sidebar_width,
                &self.toolbar_resize,
            );
            (
                toolbar_models(state, self.sidebar_width, widths, &self.pr_badge_bounds),
                state.foldout,
                state.popup.is_some(),
                !state.repositories.is_empty(),
                state.cloning.clone(),
                state.banner.clone(),
                worktree_button_visible(state),
                state.show_ci_status_popover
                    && state
                        .selected
                        .is_some_and(|id| state.current_pull_request(id).is_some()),
                widths,
            )
        };

        div()
            .id("workspace")
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.background)
            .text_color(t.text)
            .text_size(FONT_SIZE)
            .font_family(crate::theme::UI_FONT)
            .child(title_bar(cx))
            .when_some(self.welcome.clone(), |d, welcome| {
                d.child(div().flex_1().min_h_0().w_full().child(welcome))
            })
            .when(self.welcome.is_none(), |d| {
                d.child(toolbar(buttons, &self.toolbar_resize, cx))
            })
            .when(self.welcome.is_none(), |d| {
                d.when_some(banner.as_ref(), |d, banner| d.child(banner_bar(banner, cx)))
            })
            .when(self.welcome.is_none(), |d| {
                d.child(if let Some(clone) = cloning.as_ref() {
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .border_t_1()
                        .border_color(t.box_border)
                        .child(cloning_view(clone, cx))
                        .into_any_element()
                } else if has_repos {
                    self.repository_view(cx).into_any_element()
                } else {
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .border_t_1()
                        .border_color(t.box_border)
                        .child(no_repositories(cx))
                        .into_any_element()
                })
            })
            .when_some(foldout, |d, foldout| {
                // the worktree button sits between the repository and branch buttons
                let shift = if worktree_button {
                    worktree_width
                } else {
                    px(0.)
                };
                // `foldoutStyleOverrides`: as wide as the resized button,
                // at least 365 px
                let foldout_width = |width: Pixels| width.max(px(365.));
                let (x, width) = match foldout {
                    corvane_core::Foldout::Repository => (px(0.), self.sidebar_width),
                    corvane_core::Foldout::Worktree => {
                        (self.sidebar_width, foldout_width(worktree_width))
                    }
                    corvane_core::Foldout::Branch => {
                        (self.sidebar_width + shift, foldout_width(branch_width))
                    }
                    corvane_core::Foldout::PushPull => (
                        self.sidebar_width + shift + branch_width,
                        TOOLBAR_BUTTON_WIDTH,
                    ),
                };
                d.child(foldout_layer(
                    foldout,
                    x,
                    width,
                    FoldoutPanels {
                        repository: &self.repository_foldout,
                        branch: &self.branch_foldout,
                        worktree: &self.worktree_foldout,
                    },
                    window,
                    cx,
                ))
            })
            .when(ci_popover, |d| d.child(self.ci_popover.clone()))
            .when(popup, |d| d.child(self.dialogs.clone()))
            // the open dialog's title is the window title (`dialog.rs`);
            // without one it is the app's again
            .when(!popup, |d| {
                d.child(crate::dialog::window_title(crate::dialog::APP_WINDOW_TITLE))
            })
    }
}
