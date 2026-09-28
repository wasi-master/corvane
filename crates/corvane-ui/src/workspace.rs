//! Root view: title bar, toolbar, resizable sidebar + content.

use corvane_core::Section;
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::*;
use crate::changes::ChangesSidebar;
use crate::history::HistorySidebar;
use crate::icons::Octicon;
use crate::no_changes::{SuggestedAction, no_changes};
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::title_bar::title_bar;
use crate::toolbar::{ToolbarButtonModel, toolbar};

pub struct Workspace {
    focus_handle: FocusHandle,
    section: Section,
    sidebar_width: Pixels,
    resizable: Entity<ResizableState>,
    changes: Entity<ChangesSidebar>,
    history: Entity<HistorySidebar>,
}

impl Workspace {
    pub fn new(sidebar_width: Pixels, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let resizable = cx.new(|_| ResizableState::default());
        cx.subscribe(&resizable, |this, state, _: &ResizablePanelEvent, cx| {
            if let Some(width) = state.read(cx).sizes().first().copied() {
                this.sidebar_width = width;
                cx.notify();
            }
        })
        .detach();

        let changes = cx.new(|cx| ChangesSidebar::new("main".into(), window, cx));
        let history = cx.new(|cx| HistorySidebar::new(window, cx));
        window.focus(&focus_handle, cx);

        Self {
            focus_handle,
            section: Section::Changes,
            sidebar_width: sidebar_width.max(SIDEBAR_MIN_WIDTH),
            resizable,
            changes,
            history,
        }
    }

    pub fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            cx.notify();
        }
    }

    fn toolbar_buttons(&self) -> Vec<ToolbarButtonModel> {
        vec![
            ToolbarButtonModel {
                id: "toolbar-repository",
                icon: Octicon::Repo,
                description: "Current Repository".into(),
                title: "corvane".into(),
                width: Some(self.sidebar_width),
                dropdown: true,
            },
            ToolbarButtonModel {
                id: "toolbar-branch",
                icon: Octicon::GitBranch,
                description: "Current Branch".into(),
                title: "main".into(),
                width: Some(TOOLBAR_BUTTON_WIDTH),
                dropdown: true,
            },
            ToolbarButtonModel {
                id: "toolbar-push-pull",
                icon: Octicon::Sync,
                description: "Never fetched".into(),
                title: "Fetch origin".into(),
                width: Some(TOOLBAR_BUTTON_WIDTH),
                dropdown: false,
            },
        ]
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
        match self.section {
            Section::Changes => no_changes(
                vec![
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
                    SuggestedAction {
                        id: "suggested-github",
                        title: "Open the repository page on GitHub in your browser".into(),
                        description: None,
                        hint: "Repository menu or".into(),
                        keys: &["⌘", "⇧", "G"],
                        button_label: "View on GitHub".into(),
                        primary: false,
                    },
                ],
                cx,
            )
            .into_any_element(),
            Section::History => div().size_full().bg(cx.ghd().background).into_any_element(),
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .id("workspace")
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|this, _: &ShowChanges, _, cx| this.set_section(Section::Changes, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ShowHistory, _, cx| this.set_section(Section::History, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleSection, _, cx| {
                let next = match this.section {
                    Section::Changes => Section::History,
                    Section::History => Section::Changes,
                };
                this.set_section(next, cx)
            }))
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .on_action(|_: &ToggleFullScreen, window, _| window.toggle_fullscreen())
            .size_full()
            .flex()
            .flex_col()
            .bg(t.background)
            .text_color(t.text)
            .text_size(FONT_SIZE)
            .font_family(crate::theme::UI_FONT)
            .child(title_bar(cx))
            .child(toolbar(self.toolbar_buttons(), cx))
            .child(
                // `#repository`: border-top + sidebar | content
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
                    ),
            )
    }
}
