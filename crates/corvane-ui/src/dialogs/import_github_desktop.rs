//! Import Repositories from GitHub Desktop - a Corvane extra with no GHD
//! counterpart (flag `206-import-from-github-desktop`, see
//! `.docs/deviations.md`). Lists the repositories in GitHub
//! Desktop's list that are still on disk and that Corvane does not list yet
//! (`Dispatcher::find_ghd_repositories`), all ticked; "Add" adds the ticked
//! ones in GHD's order with their aliases.

use corvane_core::Dispatcher;
use corvane_core::ghd_import::GhdRepository;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog_loading};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::checkbox;

pub struct ImportGitHubDesktopDialog {
    /// `None` while GitHub Desktop's list is read.
    found: Option<Vec<(GhdRepository, bool)>>,
    /// GitHub Desktop's data directory exists on this machine.
    installed: bool,
}

impl ImportGitHubDesktopDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let weak = cx.weak_entity();
        Dispatcher::find_ghd_repositories(cx, move |repos, cx| {
            weak.update(cx, |this, cx| {
                this.found = Some(repos.into_iter().map(|r| (r, true)).collect());
                cx.notify();
            })
            .ok();
        });
        Self {
            found: None,
            installed: corvane_core::ghd_import::github_desktop_installed(),
        }
    }

    fn toggle(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some((_, ticked)) = self.found.as_mut().and_then(|f| f.get_mut(ix)) {
            *ticked = !*ticked;
            cx.notify();
        }
    }
}

impl Render for ImportGitHubDesktopDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let found = self.found.clone().unwrap_or_default();
        let picked: Vec<GhdRepository> = found
            .iter()
            .filter(|(_, ticked)| *ticked)
            .map(|(r, _)| r.clone())
            .collect();
        let content: AnyElement = if self.found.is_none() {
            div().into_any_element()
        } else if found.is_empty() {
            div()
                .max_w(zpx(400.))
                .child(if self.installed {
                    "Every repository in GitHub Desktop's list is already in Corvane, or its \
                     folder is gone."
                } else {
                    "GitHub Desktop's data wasn't found on this Mac."
                })
                .into_any_element()
        } else {
            let weak = cx.weak_entity();
            let hover_bg = t.list_item_hover_background;
            div()
                .w(zpx(460.))
                .flex()
                .flex_col()
                .gap(SPACING())
                .child("Choose the repositories to add. Aliases set in GitHub Desktop come along.")
                .child(
                    div()
                        .id("import-ghd-list")
                        .max_h(zpx(300.))
                        .overflow_y_scroll()
                        .border_1()
                        .border_color(t.box_border)
                        .rounded(BORDER_RADIUS())
                        .flex()
                        .flex_col()
                        .children(found.iter().enumerate().map(|(ix, (repo, ticked))| {
                            let weak = weak.clone();
                            let name = repo.alias.clone().unwrap_or_else(|| {
                                repo.path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| repo.path.display().to_string())
                            });
                            div()
                                .id(("import-ghd-row", ix))
                                .flex_none()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(SPACING_HALF())
                                .px(SPACING())
                                .py(SPACING_HALF())
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .on_click(move |_, _, cx| {
                                    weak.update(cx, |this, cx| this.toggle(ix, cx)).ok();
                                })
                                .child(checkbox(("import-ghd-check", ix), *ticked, false, cx))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(div().truncate().child(name))
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(FONT_SIZE_SM())
                                                .text_color(t.text_secondary)
                                                .child(repo.path.display().to_string()),
                                        ),
                                )
                        }))
                        .with_scrollbar(),
                )
                .into_any_element()
        };
        let mut buttons = vec![DialogButton {
            id: "import-ghd-cancel",
            label: if found.is_empty() && self.found.is_some() {
                "Close".into()
            } else {
                "Cancel".into()
            },
            primary: found.is_empty() && self.found.is_some(),
            disabled: false,
            on_click: Box::new(close),
        }];
        if !found.is_empty() {
            let count = picked.len();
            buttons.push(DialogButton {
                id: "import-ghd-add",
                label: match count {
                    1 => "Add 1 Repository".into(),
                    n => format!("Add {n} Repositories").into(),
                },
                primary: true,
                disabled: count == 0,
                on_click: Box::new(move |_, cx| {
                    if picked.is_empty() {
                        return;
                    }
                    Dispatcher::close_popup(cx);
                    Dispatcher::import_ghd_repositories(picked.clone(), cx);
                }),
            });
        }
        dialog_loading(
            "dialog-import-github-desktop",
            "Import from GitHub Desktop",
            self.found.is_none(),
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}
