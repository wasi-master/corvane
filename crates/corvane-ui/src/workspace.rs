//! Root view: title bar, toolbar, resizable sidebar + content, foldouts, dialogs.

use corvane_core::{AppState, Dispatcher, Section};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::changes::ChangesSidebar;
use crate::cloning_view::cloning_view;
use crate::dialogs::DialogHost;
use crate::foldout::foldout_layer;
use crate::history::HistorySidebar;
use crate::no_changes::{SuggestedAction, no_changes};
use crate::no_repositories::no_repositories;
use crate::repository_list::RepositoryFoldout;
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::title_bar::title_bar;
use crate::toolbar::{toolbar, toolbar_models};
use crate::welcome::WelcomeView;

pub struct Workspace {
    focus_handle: FocusHandle,
    state: Entity<AppState>,
    section: Section,
    sidebar_width: Pixels,
    resizable: Entity<ResizableState>,
    changes: Entity<ChangesSidebar>,
    history: Entity<HistorySidebar>,
    repository_foldout: Entity<RepositoryFoldout>,
    dialogs: Entity<DialogHost>,
    welcome: Option<Entity<WelcomeView>>,
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
            if let Some(width) = state.read(cx).sizes().first().copied() {
                if width != this.sidebar_width {
                    this.sidebar_width = width;
                    Dispatcher::update_settings(cx, |s| s.sidebar_width = f32::from(width));
                    cx.notify();
                }
            }
        })
        .detach();

        let changes = cx.new(|cx| ChangesSidebar::new(state.clone(), window, cx));
        let history = cx.new(|cx| HistorySidebar::new(window, cx));
        let repository_foldout = cx.new(|cx| RepositoryFoldout::new(state.clone(), window, cx));
        let dialogs = cx.new(|cx| DialogHost::new(state.clone(), cx));
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
            repository_foldout,
            dialogs,
            welcome,
        }
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

    pub fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
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
                        count: None,
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
        match self.section {
            Section::Changes => {
                let mut actions = vec![
                    SuggestedAction {
                        id: "suggested-editor",
                        title: "Open the repository in your external editor".into(),
                        description: Some("Select your editor in Settings".into()),
                        hint: "Repository menu or".into(),
                        keys: &["⌘", "⇧", "A"],
                        button_label: "Open in Editor".into(),
                        primary: false,
                    },
                    SuggestedAction {
                        id: "suggested-finder",
                        title: "View the files of your repository in Finder".into(),
                        description: None,
                        hint: "Repository menu or".into(),
                        keys: &["⌘", "⇧", "F"],
                        button_label: "Show in Finder".into(),
                        primary: false,
                    },
                ];
                if has_github {
                    actions.push(SuggestedAction {
                        id: "suggested-github",
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
            Section::History => div().size_full().bg(cx.ghd().background).into_any_element(),
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
                            .child(self.sidebar(cx)),
                    )
                    .child(resizable_panel().child(self.content(cx))),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let welcome_done = self.state.read(cx).settings.welcome_completed;
        if welcome_done {
            self.welcome = None;
        }
        let (buttons, foldout, popup, has_repos, cloning) = {
            let state = self.state.read(cx);
            (
                toolbar_models(state, self.sidebar_width),
                state.foldout,
                state.popup.is_some(),
                !state.repositories.is_empty(),
                state.cloning.clone(),
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
            .when(self.welcome.is_none(), |d| d.child(toolbar(buttons, cx)))
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
                let (x, width) = match foldout {
                    corvane_core::Foldout::Repository => (px(0.), self.sidebar_width),
                    corvane_core::Foldout::Branch => (self.sidebar_width, px(365.)),
                    corvane_core::Foldout::PushPull => (
                        self.sidebar_width + TOOLBAR_BUTTON_WIDTH,
                        TOOLBAR_BUTTON_WIDTH,
                    ),
                };
                d.child(foldout_layer(
                    foldout,
                    x,
                    width,
                    &self.repository_foldout,
                    window,
                    cx,
                ))
            })
            .when(popup, |d| d.child(self.dialogs.clone()))
    }
}
