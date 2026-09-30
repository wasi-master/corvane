//! Root view: title bar, toolbar, resizable sidebar + content, foldouts, dialogs.

use std::cell::Cell;
use std::rc::Rc;

use corvane_core::{AppState, Dispatcher, Section};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::banner::{banner_bar, update_banner};
use crate::branch_list::BranchFoldout;
use crate::changes::ChangesSidebar;
use crate::ci_check_popover::CiCheckPopover;
use crate::cloning_view::cloning_view;
use crate::dialogs::DialogHost;
use crate::diff_view::{DiffSource, DiffView, diff_header};
use crate::foldout::{FoldoutPanels, foldout_layer};
use crate::history::HistorySidebar;
use crate::no_changes::{SuggestedAction, no_changes};
use crate::no_repositories::NoRepositoriesView;
use crate::repository_list::RepositoryFoldout;
use crate::selected_commit::SelectedCommitView;
use crate::stash_view::StashDiffViewer;
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::title_bar::{light_title_bar, title_bar};
use crate::toolbar::{
    ToolbarResize, toolbar, toolbar_models, toolbar_widths, worktree_button_visible,
};
use crate::welcome::WelcomeView;
use crate::worktree_list::WorktreeFoldout;
use corvane_core::tutorial::TutorialStep;

/// Which `MissingRepository` variant replaces the repository view.
enum MissingRepository {
    Unsafe {
        id: u64,
        name: String,
        path: std::path::PathBuf,
        trusting: bool,
    },
    NotFound {
        id: u64,
        name: String,
        path: std::path::PathBuf,
        can_clone_again: bool,
    },
}

pub struct Workspace {
    focus_handle: FocusHandle,
    state: Entity<AppState>,
    section: Section,
    sidebar_width: Pixels,
    resizable: Entity<ResizableState>,
    /// `#window-zoom-info`: the factor to show and when it was set
    /// (GHD `ZoomInfo`: 750 ms hold after a 100 ms transition).
    zoom_info: Option<(f32, std::time::Instant)>,
    zoom_info_nonce: u64,
    changes: Entity<ChangesSidebar>,
    history: Entity<HistorySidebar>,
    selected_commit: Entity<SelectedCommitView>,
    stash_view: Entity<StashDiffViewer>,
    /// The onboarding tutorial's right-hand panel.
    tutorial_panel: Entity<crate::tutorial_panel::TutorialPanel>,
    repository_foldout: Entity<RepositoryFoldout>,
    branch_foldout: Entity<BranchFoldout>,
    worktree_foldout: Entity<WorktreeFoldout>,
    dialogs: Entity<DialogHost>,
    diff_view: Entity<DiffView>,
    welcome: Option<Entity<WelcomeView>>,
    no_repositories: Entity<NoRepositoriesView>,
    /// The branch button's PR badge rectangle (anchor of the CI popover).
    pr_badge_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Resize handles of the worktree and branch buttons.
    toolbar_resize: Rc<ToolbarResize>,
    ci_popover: Entity<CiCheckPopover>,
    /// The open foldout as of the last state change (to focus its filter
    /// once when it opens).
    last_foldout: Option<corvane_core::Foldout>,
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
            let foldout = s.foldout;
            if !overlay_open && !this.focus_handle.contains_focused(window, cx) {
                window.focus(&this.focus_handle, cx);
            }
            // GHD foldouts put the caret in their filter box when they open,
            // however they were opened (`FilterList` autoFocus)
            if foldout != this.last_foldout {
                this.last_foldout = foldout;
                match foldout {
                    Some(corvane_core::Foldout::Repository) => this
                        .repository_foldout
                        .update(cx, |f, cx| f.focus_filter(window, cx)),
                    Some(corvane_core::Foldout::Branch) => this
                        .branch_foldout
                        .update(cx, |f, cx| f.focus_filter(window, cx)),
                    Some(corvane_core::Foldout::Worktree) => this
                        .worktree_foldout
                        .update(cx, |f, cx| f.focus_filter(window, cx)),
                    _ => {}
                }
            }
        })
        .detach();

        let resizable = cx.new(|_| ResizableState::default());
        cx.subscribe(&resizable, |this, state, _: &ResizablePanelEvent, cx| {
            if let Some(width) = state.read(cx).sizes().first().copied()
                && width != this.sidebar_width
            {
                this.sidebar_width = width;
                Dispatcher::update_settings(cx, |s| s.sidebar_width = unzoom(width));
                cx.notify();
            }
        })
        .detach();

        let changes = cx.new(|cx| ChangesSidebar::new(state.clone(), window, cx));
        let history = cx.new(|cx| HistorySidebar::new(state.clone(), window, cx));
        let selected_commit = cx.new(|cx| SelectedCommitView::new(state.clone(), cx));
        let stash_view = cx.new(|cx| StashDiffViewer::new(state.clone(), cx));
        let tutorial_panel =
            cx.new(|cx| crate::tutorial_panel::TutorialPanel::new(state.clone(), cx));
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
        let no_repositories = cx.new(|cx| NoRepositoriesView::new(state.clone(), window, cx));
        window.focus(&focus_handle, cx);

        Self {
            focus_handle,
            state,
            section: Section::Changes,
            sidebar_width: sidebar_width.max(SIDEBAR_MIN_WIDTH()),
            resizable,
            zoom_info: None,
            zoom_info_nonce: 0,
            changes,
            history,
            selected_commit,
            stash_view,
            tutorial_panel,
            repository_foldout,
            branch_foldout,
            worktree_foldout,
            pr_badge_bounds,
            toolbar_resize: Rc::new(ToolbarResize::default()),
            ci_popover,
            last_foldout: None,
            dialogs,
            diff_view,
            welcome,
            no_repositories,
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
            // `renderTutorialPane` in place of "No local changes"
            Section::Changes if state.selected_tutorial_step().is_valid() => {
                let step = state.selected_tutorial_step();
                if matches!(step, TutorialStep::AllDone | TutorialStep::Announced) {
                    div()
                        .size_full()
                        .relative()
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |_, _, _, cx| {
                                    // `onTutorialCompletionAnnounced`, deferred:
                                    // it notifies AppState
                                    if step == TutorialStep::AllDone {
                                        cx.defer(Dispatcher::mark_tutorial_completion_announced);
                                    }
                                },
                            )
                            .absolute()
                            .size_0(),
                        )
                        .child(crate::tutorial_panel::tutorial_done(cx))
                        .into_any_element()
                } else {
                    crate::tutorial_panel::tutorial_welcome(cx).into_any_element()
                }
            }
            Section::Changes => {
                let (repo_id, repo_path, editor_label, shell_label) = {
                    let s = self.state.read(cx);
                    let repo = s.selected_repository();
                    (
                        repo.map(|r| r.id),
                        repo.map(|r| r.path.clone()),
                        s.editor_label(),
                        // flag `285-no-changes-open-in-shell` (Corvane
                        // addition; GHD `NoChanges` has no shell action)
                        s.flags
                            .bool(corvane_core::flags::ids::NO_CHANGES_OPEN_IN_SHELL)
                            .then(|| s.shell_label()),
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
                if let Some(shell) = shell_label {
                    actions.push(SuggestedAction {
                        id: "suggested-shell",
                        on_click: std::rc::Rc::new({
                            let path = path.clone();
                            move |_, cx| Dispatcher::open_in_shell(&path, cx)
                        }),
                        title: format!("Open the repository in {shell}").into(),
                        description: Some("Select your shell in Settings".into()),
                        hint: "Repository menu or".into(),
                        keys: &["⌃", "`"],
                        button_label: format!("Open in {shell}").into(),
                        primary: false,
                    });
                }
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
                            .size_range(SIDEBAR_MIN_WIDTH()..zpx(900.))
                            .child(crate::active_resizable::active_resizable(
                                "repository-sidebar-resizable",
                                &self.resizable,
                                None,
                                crate::active_resizable::ResizableDescription::new(
                                    "Repository sidebar",
                                    SIDEBAR_MIN_WIDTH()..zpx(900.),
                                ),
                                self.sidebar(cx),
                            )),
                    )
                    .child(resizable_panel().child(self.content(cx))),
            )
    }

    /// `maybeRenderTutorialPanel`: the repository view with the tutorial
    /// panel on the right while a tutorial step is showing.
    fn repository_view_with_tutorial(&self, cx: &Context<Self>) -> AnyElement {
        if !self.state.read(cx).selected_tutorial_step().is_valid() {
            return self.repository_view(cx).into_any_element();
        }
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_row()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.repository_view(cx)),
            )
            .child(self.tutorial_panel.clone())
            .into_any_element()
    }
}

impl Workspace {
    /// View › Zoom In (+1) / Zoom Out (-1) / Reset Zoom (0): GHD's
    /// `zoom(ZoomDirection)` steps through `ZoomInFactors`, persists the
    /// factor (Electron keeps `zoomFactor`) and shows `#window-zoom-info`.
    pub fn zoom(&mut self, direction: i32, cx: &mut Context<Self>) {
        use crate::theme::sizes::{next_zoom_factor, set_zoom_factor, zoom_factor};
        let current = zoom_factor();
        let next = if direction == 0 {
            1.0
        } else {
            next_zoom_factor(current, direction)
        };
        set_zoom_factor(next);
        tracing::info!(from = current, to = next, "zoom changed");
        // sizes read the factor at render; the kit's font size and radius
        // are copied at apply time
        crate::theme::apply(cx.ghd().clone(), cx);
        Dispatcher::update_settings(cx, |s| s.window_zoom_factor = next);
        let settings_sidebar = self.state.read(cx).settings.sidebar_width;
        self.sidebar_width = zpx(settings_sidebar).max(SIDEBAR_MIN_WIDTH());
        // the panel group keeps screen-pixel sizes: drop them so the next
        // layout takes the sidebar's zoomed width again
        self.resizable.update(cx, |state, _| state.clear());
        self.zoom_info_nonce += 1;
        let nonce = self.zoom_info_nonce;
        self.zoom_info = Some((next, std::time::Instant::now()));
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(850))
                .await;
            this.update(cx, |this, cx| {
                if this.zoom_info_nonce == nonce {
                    this.zoom_info = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.refresh_windows();
        cx.notify();
    }

    /// `#window-zoom-info` (`styles/ui/window/_zoom-info.scss`): a pill
    /// with the percentage, centred over the content, ignoring the mouse.
    fn zoom_info_overlay(&self, cx: &App) -> Option<AnyElement> {
        let (factor, _) = self.zoom_info?;
        let t = cx.ghd();
        Some(
            div()
                .id("window-zoom-info")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .p(SPACING())
                        .min_w(zpx(100.))
                        .rounded(zpx(100.))
                        .bg(t.tooltip_background)
                        .text_color(t.tooltip_text)
                        .text_size(FONT_SIZE_MD())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_align(TextAlign::Center)
                        .child(format!("{}%", (factor * 100.).round() as i32)),
                )
                .into_any_element(),
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
        // `inNoRepositoriesViewState`: a paused tutorial shows the blank slate
        let tutorial_paused = self.state.read(cx).selected_tutorial_step() == TutorialStep::Paused;
        if welcome_done {
            self.welcome = None;
        }
        // GHD `SelectionType.MissingRepository`: the unsafe variant when git
        // named an unsafe directory, else "Can't find"
        let missing_repository = {
            let state = self.state.read(cx);
            state.selected_repository().and_then(|repo| {
                let rs = state.repo_states.get(&repo.id);
                match rs.and_then(|rs| rs.unsafe_path.clone()) {
                    Some(path) => Some(MissingRepository::Unsafe {
                        id: repo.id,
                        name: repo.name(),
                        path,
                        trusting: rs.is_some_and(|rs| rs.trusting_path),
                    }),
                    None if repo.missing => Some(MissingRepository::NotFound {
                        id: repo.id,
                        name: repo.name(),
                        path: repo.path.clone(),
                        can_clone_again: repo
                            .github
                            .as_ref()
                            .is_some_and(|gh| !gh.clone_url.is_empty()),
                    }),
                    None => None,
                }
            })
        };
        let update_available = {
            let state = self.state.read(cx);
            if state.update.banner_visible {
                state.update.status.available().cloned().map(|u| {
                    (
                        u,
                        matches!(
                            state.update.status,
                            corvane_core::UpdateStatus::AvailableViaHomebrew { .. }
                        ),
                    )
                })
            } else {
                None
            }
        };
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

        // GHD `inNoRepositoriesViewState` (repositories.length === 0, or a
        // paused tutorial): no toolbar (`renderToolbar`) and, like the welcome
        // flow, the transparent `light-title-bar` laid over the content
        let blank_slate = tutorial_paused || (!has_repos && cloning.is_none());
        let bare = self.welcome.is_some() || blank_slate;
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
            .text_size(FONT_SIZE())
            .font_family(crate::theme::UI_FONT)
            .when(!bare, |d| d.child(title_bar(cx)))
            .when_some(self.welcome.clone(), |d, welcome| {
                d.child(div().flex_1().min_h_0().w_full().child(welcome))
            })
            .when(!bare, |d| {
                d.child(toolbar(buttons, &self.toolbar_resize, cx))
            })
            .when(self.welcome.is_none(), |d| {
                d.when_some(banner.as_ref(), |d, banner| d.child(banner_bar(banner, cx)))
            })
            // GHD shows the update banner only while no other banner is up
            .when(self.welcome.is_none() && banner.is_none(), |d| {
                d.when_some(update_available.as_ref(), |d, (update, homebrew)| {
                    d.child(update_banner(update, *homebrew, cx))
                })
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
                } else if let Some(missing) = missing_repository.as_ref() {
                    // GHD `SelectionType.MissingRepository` replaces the
                    // whole repository view
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .border_t_1()
                        .border_color(t.box_border)
                        .child(match missing {
                            MissingRepository::Unsafe {
                                id,
                                name,
                                path,
                                trusting,
                            } => crate::missing_repository::unsafe_repository_view(
                                *id, name, path, *trusting, cx,
                            )
                            .into_any_element(),
                            MissingRepository::NotFound {
                                id,
                                name,
                                path,
                                can_clone_again,
                            } => crate::missing_repository::missing_repository_view(
                                *id,
                                name,
                                path,
                                *can_clone_again,
                                cx,
                            )
                            .into_any_element(),
                        })
                        .into_any_element()
                } else if has_repos && !tutorial_paused {
                    self.repository_view_with_tutorial(cx)
                } else {
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .child(self.no_repositories.clone())
                        .into_any_element()
                })
            })
            .when(bare, |d| d.child(light_title_bar()))
            .when_some(foldout, |d, foldout| {
                // the worktree button sits between the repository and branch buttons
                let shift = if worktree_button {
                    worktree_width
                } else {
                    zpx(0.)
                };
                // `foldoutStyleOverrides`: as wide as the resized button,
                // at least 365 px
                let foldout_width = |width: Pixels| width.max(zpx(365.));
                let (x, width) = match foldout {
                    corvane_core::Foldout::Repository => (zpx(0.), self.sidebar_width),
                    corvane_core::Foldout::Worktree => {
                        (self.sidebar_width, foldout_width(worktree_width))
                    }
                    corvane_core::Foldout::Branch => {
                        (self.sidebar_width + shift, foldout_width(branch_width))
                    }
                    corvane_core::Foldout::PushPull => (
                        self.sidebar_width + shift + branch_width,
                        TOOLBAR_BUTTON_WIDTH(),
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
            .children(self.zoom_info_overlay(cx))
            .when(popup, |d| d.child(self.dialogs.clone()))
            // the open dialog's title is the window title (`dialog.rs`);
            // without one it is the app's again
            .when(!popup, |d| {
                d.child(crate::dialog::window_title(crate::dialog::APP_WINDOW_TITLE))
            })
    }
}
