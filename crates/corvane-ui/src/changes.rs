//! Changes sidebar: filter header, "N changed files" row, file list, commit form.
//! `styles/ui/changes/{_changes-list,_commit-message}.scss`.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use corvane_core::filter::{filtered_files, no_results_message, option_count};
use corvane_core::{
    AppState, DiffSelectionType, Dispatcher, FileListFilter, FileStatusKind, FilterOption, Tip,
    WorkingDirectoryFileChange,
};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::{InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::Commit;
use crate::context_menu::{ContextMenu, MenuItem};
use crate::diff_view::status_icon;
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{avatar_placeholder, button, checkbox, primary_button, text_box};

pub struct ChangesSidebar {
    filter: Entity<InputState>,
    summary: Entity<InputState>,
    description: Entity<TextareaState>,
    state: Entity<AppState>,
    seen_commit_nonce: u64,
    context_menu: Option<Entity<ContextMenu>>,
    /// GHD `ChangesListFilterOptions` popover.
    filter_popover_open: bool,
    filter_button_bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl ChangesSidebar {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        // Clear the form after a successful commit (GHD resets `commitMessage`).
        cx.observe_in(&state, window, |this, state, window, cx| {
            let nonce = state
                .read(cx)
                .selected_state()
                .map(|rs| rs.commit_nonce)
                .unwrap_or(0);
            if nonce != this.seen_commit_nonce {
                this.seen_commit_nonce = nonce;
                this.summary.update(cx, |s, cx| s.set_value("", window, cx));
                this.description
                    .update(cx, |s, cx| s.set_value("", window, cx));
                cx.notify();
            }
        })
        .detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        let summary = cx.new(|cx| InputState::new(window, cx).placeholder("Summary (required)"));
        let description = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("Description")
        });
        Self {
            filter,
            summary,
            description,
            state,
            seen_commit_nonce: 0,
            context_menu: None,
            filter_popover_open: false,
            filter_button_bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }

    /// Files that pass the text + option filters, plus the unfiltered total.
    fn visible_files(&self, cx: &App) -> (Vec<WorkingDirectoryFileChange>, usize) {
        let text = self.filter.read(cx).value().to_string();
        let s = self.state.read(cx);
        let Some(rs) = s.selected_state() else {
            return (Vec::new(), 0);
        };
        let Some(status) = rs.status.as_ref() else {
            return (Vec::new(), 0);
        };
        let visible = filtered_files(&status.files, &text, &rs.file_list_filter)
            .into_iter()
            .cloned()
            .collect();
        (visible, status.files.len())
    }

    fn filter_options(&self, cx: &App) -> FileListFilter {
        self.state
            .read(cx)
            .selected_state()
            .map(|rs| rs.file_list_filter)
            .unwrap_or_default()
    }

    /// `ChangesListFilterOptions` popover: header, five option checkboxes,
    /// "Clear filters" when anything is active. Anchored under the button.
    fn filter_popover(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        if !self.filter_popover_open {
            return None;
        }
        let t = cx.ghd();
        let s = self.state.read(cx);
        let id = s.selected?;
        let files = s
            .selected_state()
            .and_then(|rs| rs.status.as_ref())
            .map(|st| st.files.clone())
            .unwrap_or_default();
        let filter = self.filter_options(cx);
        let text_active = !self.filter.read(cx).value().trim().is_empty();
        let active = filter.count_active() > 0 || text_active;
        let bounds = self.filter_button_bounds.get();
        let close =
            |this: &mut Self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>| {
                this.filter_popover_open = false;
                cx.notify();
            };
        let option_row = |option: FilterOption, label: &str| {
            let checked = filter.get(option);
            let count = option_count(option, &files);
            div()
                .id(SharedString::from(format!("filter-opt-{label}")))
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF)
                .py(px(3.))
                .cursor_pointer()
                // GHD closes the popover after every option change
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter_popover_open = false;
                    Dispatcher::toggle_filter_option(id, option, cx)
                }))
                .child(checkbox(
                    SharedString::from(format!("filter-check-{label}")),
                    checked,
                    false,
                    cx,
                ))
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .child(format!("{label} ({count})")),
                )
        };
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("filter-popover-overlay")
                        .relative()
                        .size_full()
                        .on_mouse_down(MouseButton::Left, cx.listener(close))
                        .on_mouse_down(MouseButton::Right, cx.listener(close))
                        .child(
                            div()
                                .id("filter-popover")
                                .absolute()
                                .left(bounds.origin.x)
                                .top(bounds.origin.y + bounds.size.height + px(8.))
                                .min_w(px(200.))
                                .flex()
                                .flex_col()
                                .px(SPACING)
                                .pt(SPACING)
                                .rounded(BORDER_RADIUS)
                                .bg(t.background)
                                .text_color(t.text)
                                .border_1()
                                .border_color(t.box_border)
                                .shadow(vec![BoxShadow {
                                    color: t.shadow,
                                    offset: point(px(0.), px(2.)),
                                    blur_radius: px(7.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }])
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(FONT_SIZE_MD)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child("Filter Options"),
                                        )
                                        .child(
                                            div()
                                                .id("filter-popover-close")
                                                .size(px(16.))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.filter_popover_open = false;
                                                    cx.notify();
                                                }))
                                                .child(octicon(Octicon::X, t.text_secondary)),
                                        ),
                                )
                                .child(
                                    div()
                                        .my(SPACING)
                                        .flex()
                                        .flex_col()
                                        .child(option_row(
                                            FilterOption::IncludedInCommit,
                                            "Included in commit",
                                        ))
                                        .child(option_row(
                                            FilterOption::ExcludedFromCommit,
                                            "Excluded from commit",
                                        ))
                                        .child(option_row(FilterOption::NewFiles, "New files"))
                                        .child(option_row(
                                            FilterOption::ModifiedFiles,
                                            "Modified files",
                                        ))
                                        .child(option_row(
                                            FilterOption::DeletedFiles,
                                            "Deleted files",
                                        )),
                                )
                                .when(active, |d| {
                                    d.child(div().pt(SPACING_HALF).pb(SPACING).child(
                                        button("filter-clear", "Clear filters", cx).on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.filter.update(cx, |s, cx| {
                                                    s.set_value("", window, cx)
                                                });
                                                this.filter_popover_open = false;
                                                Dispatcher::clear_filter_options(id, cx);
                                            }),
                                        ),
                                    ))
                                })
                                .when(!active, |d| d.pb(SPACING_HALF)),
                        ),
                ),
            )
            .with_priority(25),
        )
    }

    fn open_menu(
        &mut self,
        items: Vec<MenuItem>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = cx.new(|cx| ContextMenu::new(position, items, window, cx));
        cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.context_menu = None;
            cx.notify();
        })
        .detach();
        self.context_menu = Some(menu);
        cx.notify();
    }

    /// GHD `getDefaultContextMenu` for one file (multi-select variants come
    /// with multi-selection).
    fn open_file_menu(
        &mut self,
        file: WorkingDirectoryFileChange,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (id, confirm, repo_path) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            if s.selected_state().map(|r| r.committing).unwrap_or(false) {
                return;
            }
            let Some(repo) = s.repository(id) else { return };
            (id, s.settings.confirm_discard_changes, repo.path.clone())
        };
        let path = file.path.clone();
        let full = repo_path.join(&path);
        let deleted = file.status.kind == FileStatusKind::Deleted;
        let is_gitignore = file.file_name() == ".gitignore";
        let extension = Path::new(&path)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()));
        let parents: Vec<&str> = {
            let mut components: Vec<&str> = path.split('/').collect();
            components.pop();
            components
        };
        let mut items = vec![
            MenuItem::new(
                if confirm {
                    "Discard Changes…"
                } else {
                    "Discard Changes"
                },
                {
                    let p = path.clone();
                    move |_, cx| Dispatcher::request_discard_changes(id, vec![p.clone()], cx)
                },
            ),
            MenuItem::separator(),
            MenuItem::new("Ignore File (Add to .gitignore)", {
                let p = path.clone();
                move |_, cx| Dispatcher::ignore_files(id, vec![p.clone()], cx)
            })
            .enabled(!is_gitignore),
        ];
        if !parents.is_empty() {
            let folders: Vec<MenuItem> = (0..parents.len())
                .map(|index| {
                    let label = format!("/{}", parents[..parents.len() - index].join("/"));
                    let pattern = label.clone();
                    MenuItem::new(label, move |_, cx| {
                        Dispatcher::ignore_files(id, vec![pattern.clone()], cx)
                    })
                })
                .collect();
            items.push(
                MenuItem::submenu("Ignore Folder (Add to .gitignore)", folders)
                    .enabled(!is_gitignore),
            );
        }
        if let Some(ext) = extension {
            let pattern = format!("*{ext}");
            items.push(MenuItem::new(
                format!("Ignore All {ext} Files (Add to .gitignore)"),
                move |_, cx| Dispatcher::ignore_patterns(id, vec![pattern.clone()], cx),
            ));
        }
        items.push(MenuItem::separator());
        items.push(MenuItem::new("Copy File Path", {
            let text = full.to_string_lossy().into_owned();
            move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
        }));
        items.push(MenuItem::new("Copy Relative File Path", {
            let p = path.clone();
            move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(p.clone()))
        }));
        items.push(MenuItem::separator());
        items.push(
            MenuItem::new("Reveal in Finder", {
                let f = full.clone();
                move |_, cx| cx.reveal_path(&f)
            })
            .enabled(!deleted),
        );
        // TODO(M6): external editor detection; opens with the default program until then.
        items.push(
            MenuItem::new("Open in External Editor", {
                let f = full.clone();
                move |_, cx| cx.open_with_system(&f)
            })
            .enabled(!deleted),
        );
        items.push(
            MenuItem::new("Open with Default Program", {
                let f = full.clone();
                move |_, cx| cx.open_with_system(&f)
            })
            .enabled(!deleted),
        );
        self.open_menu(items, position, window, cx);
    }

    /// GHD `onContextMenu` on the list itself: Discard All / Stash All.
    fn open_list_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (id, confirm, paths) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            let Some(rs) = s.selected_state() else { return };
            if rs.committing {
                return;
            }
            let paths: Vec<String> = rs
                .status
                .as_ref()
                .map(|st| st.files.iter().map(|f| f.path.clone()).collect())
                .unwrap_or_default();
            (id, s.settings.confirm_discard_changes, paths)
        };
        let has_changes = !paths.is_empty();
        let items = vec![
            MenuItem::new(
                if confirm {
                    "Discard All Changes…"
                } else {
                    "Discard All Changes"
                },
                move |_, cx| Dispatcher::request_discard_changes(id, paths.clone(), cx),
            )
            .enabled(has_changes),
            // TODO(M4): stashes; disabled until then.
            MenuItem::new("Stash All Changes", |_, _| {}).enabled(false),
        ];
        self.open_menu(items, position, window, cx);
    }

    fn do_commit(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let summary = self.summary.read(cx).value().to_string();
        let description = self.description.read(cx).value().to_string();
        Dispatcher::commit(id, summary, description, cx);
    }

    /// `.filtered-changes-list .header`: filter row + check-all row.
    fn header(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .px(SPACING)
            .py(SPACING_HALF)
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .child(
                // Filter row: [Filter Options ▾][Filter…] as a joined button group
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(TEXT_FIELD_HEIGHT)
                    .child({
                        let active = self.filter_options(cx).count_active() > 0;
                        let bounds_cell = self.filter_button_bounds.clone();
                        div()
                            .id("filter-options")
                            .relative()
                            .h(TEXT_FIELD_HEIGHT)
                            .w(px(48.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(2.))
                            .border_1()
                            .border_color(t.secondary_button_border)
                            .rounded_l(BORDER_RADIUS)
                            .bg(t.secondary_button_background)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.filter_popover_open = !this.filter_popover_open;
                                cx.notify();
                            }))
                            .child(
                                canvas(move |b, _, _| bounds_cell.set(b), |_, _, _, _| {})
                                    .absolute()
                                    .size_full(),
                            )
                            // `.active span:first-child { color: box-selected-active-background }`
                            .child(octicon(
                                Octicon::Filter,
                                if active {
                                    t.box_selected_active_background
                                } else {
                                    t.secondary_button_text
                                },
                            ))
                            .child(
                                octicon(Octicon::TriangleDown, t.secondary_button_text)
                                    .size(px(12.)),
                            )
                            // `.active-badge`: 5 px dot with a 1 px ring, right 18 / top 4
                            .when(active, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .top(px(4.))
                                        .right(px(18.))
                                        .p(px(1.))
                                        .rounded_full()
                                        .bg(t.secondary_button_background)
                                        .child(
                                            div()
                                                .size(px(5.))
                                                .rounded_full()
                                                .bg(t.box_selected_active_background),
                                        ),
                                )
                            })
                    })
                    .child(
                        text_box("changes-filter", &self.filter, None, window, cx)
                            .rounded_l(px(0.))
                            .border_l_0(),
                    ),
            )
            .child(
                // "☑ N changed files"
                div()
                    .h(ROW_HEIGHT)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    // GHD shows the include-all box checked but disabled when there is nothing to commit.
                    .child({
                        let (visible, total, include_all, repo_id) = self.header_state(cx);
                        let paths: Vec<String> = visible.iter().map(|f| f.path.clone()).collect();
                        let disabled = total == 0 || visible.is_empty();
                        let include = include_all != Some(true);
                        checkbox("check-all", include_all != Some(false), disabled, cx)
                            .when(include_all.is_none(), |d| d.opacity(0.7))
                            .when_some(repo_id.filter(|_| !disabled), |d, id| {
                                d.on_click(move |_, _, cx| {
                                    Dispatcher::set_files_included(id, paths.clone(), include, cx)
                                })
                            })
                    })
                    .child({
                        let (visible, total, _, _) = self.header_state(cx);
                        // GHD: "3 of 10 changed files" while a filter hides some
                        let prefix = if visible.len() != total {
                            format!("{} of ", visible.len())
                        } else {
                            String::new()
                        };
                        div().text_size(FONT_SIZE).truncate().child(if total == 1 {
                            format!("{prefix}1 changed file")
                        } else {
                            format!("{prefix}{total} changed files")
                        })
                    }),
            )
    }

    fn branch_name(&self, cx: &App) -> SharedString {
        self.state
            .read(cx)
            .selected_state()
            .and_then(|s| s.info.as_ref())
            .and_then(|i| match &i.tip {
                Tip::Valid { branch } => Some(branch.name.clone()),
                Tip::Unborn { name } => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_default()
            .into()
    }

    /// (visible files, total, include-all tri-state of the visible files, repo id)
    fn header_state(
        &self,
        cx: &App,
    ) -> (
        Vec<WorkingDirectoryFileChange>,
        usize,
        Option<bool>,
        Option<u64>,
    ) {
        let (visible, total) = self.visible_files(cx);
        let id = self.state.read(cx).selected;
        // `getCheckAllValue`: the box reflects only the files passing the filter
        let include_all = if visible.is_empty() {
            Some(true)
        } else {
            let all = visible
                .iter()
                .all(|f| f.selection.kind() == DiffSelectionType::All);
            let none = visible
                .iter()
                .all(|f| f.selection.kind() == DiffSelectionType::None);
            if all {
                Some(true)
            } else if none {
                Some(false)
            } else {
                None
            }
        };
        (visible, total, include_all, id)
    }

    /// `ChangesList`: 29 px rows - checkbox, dimmed directory + bold name, status icon.
    fn list(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let repo_id = s.selected;
        let rs = s.selected_state();
        let (files, _) = self.visible_files(cx);
        let empty_message = if files.is_empty() {
            no_results_message(&self.filter.read(cx).value(), &self.filter_options(cx))
        } else {
            None
        };
        let selected = rs.and_then(|r| r.selected_file.clone());
        let hover_bg = t.list_item_hover_background;
        div()
            .id("changes-list")
            .flex_1()
            .min_h(px(100.))
            .overflow_y_scroll()
            .bg(t.background)
            .flex()
            .flex_col()
            .when_some(empty_message, |d, message| {
                d.child(
                    div()
                        .p(SPACING_DOUBLE)
                        .text_size(FONT_SIZE)
                        .text_color(t.text_secondary)
                        .child(message),
                )
            })
            .children(files.into_iter().map(|file| {
                let is_selected = selected.as_deref() == Some(file.path.as_str());
                let (icon, color) = status_icon(file.status.kind, t);
                let path_for_select = file.path.clone();
                let path_for_toggle = file.path.clone();
                let included = file.selection.kind() != DiffSelectionType::None;
                let file_for_menu = file.clone();
                div()
                    .id(SharedString::from(format!("file-{}", file.path)))
                    .h(ROW_HEIGHT)
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .px(SPACING)
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_file_menu(file_for_menu.clone(), ev.position, window, cx);
                        }),
                    )
                    .when(is_selected, |d| {
                        d.bg(t.box_selected_background)
                            .text_color(t.box_selected_text)
                    })
                    .when(!is_selected, move |d| d.hover(move |s| s.bg(hover_bg)))
                    .when_some(repo_id, move |d, id| {
                        d.on_click(move |_, _, cx| {
                            Dispatcher::select_file(id, path_for_select.clone(), cx)
                        })
                    })
                    .child(
                        checkbox(
                            SharedString::from(format!("include-{}", file.path)),
                            included,
                            false,
                            cx,
                        )
                        .when(file.selection.kind() == DiffSelectionType::Partial, |d| {
                            d.opacity(0.7)
                        })
                        .when_some(repo_id, move |d, id| {
                            d.on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                Dispatcher::toggle_file_included(id, path_for_toggle.clone(), cx)
                            })
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(FONT_SIZE)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .child(
                                        div()
                                            .text_color(t.text_secondary)
                                            .child(file.directory().to_string()),
                                    )
                                    .child(div().child(file.file_name().to_string())),
                            ),
                    )
                    .child(octicon(icon, color))
            }))
    }

    fn commit_disabled(&self, cx: &App) -> bool {
        let s = self.state.read(cx);
        let rs = s.selected_state();
        let any_included = rs
            .and_then(|r| r.status.as_ref())
            .map(|st| {
                st.files
                    .iter()
                    .any(|f| f.selection.kind() != DiffSelectionType::None)
            })
            .unwrap_or(false);
        let committing = rs.map(|r| r.committing).unwrap_or(false);
        self.summary.read(cx).value().trim().is_empty() || !any_included || committing
    }

    /// `#undo-commit`: "Committed N ago / summary" + Undo, after a commit.
    fn undo_bar(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let id = s.selected?;
        let last = s.selected_state()?.last_commit.clone()?;
        Some(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .mt(SPACING)
                .mx(px(-10.))
                .mb(px(-10.))
                .border_t_1()
                .border_color(t.box_border)
                .bg(t.box_alt_background)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .py(SPACING_HALF)
                        .pl(SPACING)
                        .pr(SPACING_HALF)
                        .text_size(FONT_SIZE_SM)
                        .child(
                            div()
                                .text_color(t.text_secondary)
                                .truncate()
                                .child(format!("Committed {}", relative(last.at))),
                        )
                        .child(div().truncate().child(last.summary.clone())),
                )
                .child(
                    div().p(SPACING).pl(px(0.)).child(
                        button("undo-commit", "Undo", cx)
                            .on_click(move |_, _, cx| Dispatcher::undo_commit(id, cx)),
                    ),
                ),
        )
    }

    /// `.commit-message-component`
    fn commit_form(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .id("commit-message")
            .key_context("CommitMessage")
            .on_action(cx.listener(|this, _: &Commit, _, cx| {
                if !this.commit_disabled(cx) {
                    this.do_commit(cx)
                }
            }))
            .flex_none()
            .flex()
            .flex_col()
            .p(SPACING)
            .bg(t.box_alt_background)
            .border_t_1()
            .border_color(t.box_border)
            .child(
                // `.summary`: avatar + summary field
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .mb(SPACING)
                    .child(avatar_placeholder(AVATAR_SIZE, cx))
                    .child(text_box("commit-summary", &self.summary, None, window, cx)),
            )
            .child(
                // `.description-focus-container`: textarea + action bar
                div()
                    .flex()
                    .flex_col()
                    .mb(SPACING)
                    .border_1()
                    .border_color(t.box_border_contrast)
                    .rounded(BORDER_RADIUS)
                    .bg(t.box_background)
                    .overflow_hidden()
                    .child(
                        Textarea::new(&self.description)
                            .appearance(false)
                            .small()
                            .h(px(80.)),
                    )
                    .child(
                        // `.action-bar`: add co-authors | commit options
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF)
                            .px(SPACING)
                            .pb(px(8.))
                            .child(octicon(Octicon::PersonAdd, t.text_secondary))
                            .child(div().w(px(1.)).h(px(16.)).bg(t.box_border_contrast))
                            .child(octicon(Octicon::Gear, t.text_secondary)),
                    ),
            )
            .child(
                primary_button(
                    "commit",
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.))
                        .child("Commit to")
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.branch_name(cx)),
                        ),
                    self.commit_disabled(cx),
                    cx,
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    if !this.commit_disabled(cx) {
                        this.do_commit(cx)
                    }
                })),
            )
            .when_some(self.undo_bar(cx), |d, bar| d.child(bar))
    }
}

impl Render for ChangesSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .id("changes-list-container")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                            this.open_list_menu(ev.position, window, cx)
                        }),
                    )
                    .child(self.header(window, cx))
                    .child(self.list(cx)),
            )
            .child(self.commit_form(window, cx))
            .children(self.context_menu.clone())
            .children(self.filter_popover(cx))
    }
}
