//! Worktree foldout (GHD `ui/worktrees/worktree-list.tsx`, `worktree-list-item.tsx`,
//! `worktree-list-item-context-menu.ts`, `styles/ui/_worktrees.scss`): filter,
//! "New Worktree", the Main / Linked groups, and the context menus shared
//! with the toolbar button (`ui/toolbar/worktree-dropdown.tsx`).

use std::path::PathBuf;

use corvane_core::{AppState, Dispatcher, Popup, WorktreeEntry, WorktreeType};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{ContextMenu, MenuItem};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::ListRowA11y;
use crate::widgets::{button, text_box};

/// GHD `RowHeight` of the worktree list.
const WORKTREE_ROW_HEIGHT: Pixels = px(30.);

/// GHD `generateWorktreeContextMenuItems`.
pub fn worktree_menu_items(
    repo: u64,
    worktree: &WorktreeEntry,
    with_rename: bool,
    with_delete: bool,
) -> Vec<MenuItem> {
    let path = worktree.path.clone();
    let is_main = worktree.kind == WorktreeType::Main;
    let editable = !is_main && !worktree.is_locked;
    let name = worktree.display_name();
    let mut items = Vec::new();
    if with_rename {
        let p = path.clone();
        items.push(
            MenuItem::new("Rename…", move |_, cx| {
                Dispatcher::close_foldout(cx);
                Dispatcher::show_popup(
                    Popup::RenameWorktree {
                        repo,
                        path: p.clone(),
                    },
                    cx,
                );
            })
            .enabled(editable),
        );
    }
    items.push(MenuItem::new("Copy Worktree Name", move |_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(name.clone()))
    }));
    let path_text = path.display().to_string();
    items.push(MenuItem::new("Copy Worktree Path", move |_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(path_text.clone()))
    }));
    items.push(MenuItem::separator());
    if with_delete {
        items.push(
            MenuItem::new("Delete…", move |_, cx| {
                Dispatcher::close_foldout(cx);
                Dispatcher::request_delete_worktree(repo, path.clone(), cx);
            })
            .enabled(editable),
        );
    }
    items
}

/// The worktree the repository currently points at.
pub fn current_worktree(state: &AppState, repo: u64) -> Option<WorktreeEntry> {
    let path = &state.repository(repo)?.path;
    state
        .repo_states
        .get(&repo)?
        .worktrees
        .iter()
        .find(|w| &w.path == path)
        .cloned()
}

/// GHD `WorktreeDropdown.onContextMenu`: right-click on the toolbar button.
pub fn toolbar_button_menu(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let (repo, current) = {
        let s = AppState::global(cx).read(cx);
        let Some(id) = s.selected else { return };
        (id, current_worktree(s, id))
    };
    let Some(current) = current else { return };
    let is_main = current.kind == WorktreeType::Main;
    let mut items = vec![
        MenuItem::new("New Worktree…", move |_, cx| {
            Dispatcher::close_foldout(cx);
            Dispatcher::show_popup(
                Popup::AddWorktree {
                    repo,
                    initial_branch_name: None,
                    initial_worktree_name: None,
                },
                cx,
            );
        }),
        MenuItem::separator(),
    ];
    items.extend(worktree_menu_items(repo, &current, false, !is_main));
    show_menu(items, position, window, cx);
}

fn show_menu(items: Vec<MenuItem>, position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        crate::native_menu::show_context_menu(items, position, window, cx);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (items, position, window, cx);
    }
}

pub struct WorktreeFoldout {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    context_menu: Option<Entity<ContextMenu>>,
}

impl WorktreeFoldout {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            filter,
            context_menu: None,
        }
    }

    pub fn focus_filter(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.filter.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        cx.notify();
    }

    fn open_item_menu(
        &mut self,
        repo: u64,
        worktree: &WorktreeEntry,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = worktree_menu_items(repo, worktree, true, true);
        #[cfg(target_os = "macos")]
        {
            crate::native_menu::show_context_menu(items, position, window, cx);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let menu = cx.new(|cx| ContextMenu::new(items, position, window, cx));
            self.context_menu = Some(menu);
            cx.notify();
        }
    }

    /// `.worktrees-list-item`: icon, name (match in bold), branch / sha.
    fn row(
        &self,
        repo: u64,
        worktree: &WorktreeEntry,
        current: bool,
        query: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = cx.ghd();
        let name = worktree.display_name();
        let description = worktree.description();
        let hover_bg = t.box_selected_active_background;
        let hover_text = t.box_selected_active_text;
        let path = worktree.path.clone();
        let for_menu = worktree.clone();
        let title: AnyElement = match (!query.is_empty())
            .then(|| name.to_lowercase().find(&query.to_lowercase()))
            .flatten()
        {
            Some(ix) => {
                let end = (ix + query.len()).min(name.len());
                div()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(name[..ix].to_string())
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(name[ix..end].to_string()),
                    )
                    .child(name[end..].to_string())
                    .into_any_element()
            }
            None => div().truncate().child(name.clone()).into_any_element(),
        };
        div()
            .id(SharedString::from(format!("worktree-{}", path.display())))
            .a11y_row(format!("{name}, {description}"), current)
            .h(WORKTREE_ROW_HEIGHT)
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px(SPACING)
            .cursor_pointer()
            .text_size(FONT_SIZE)
            .hover(move |s| s.bg(hover_bg).text_color(hover_text))
            .on_click(move |_, _, cx| {
                Dispatcher::close_foldout(cx);
                Dispatcher::switch_worktree(repo, path.clone(), cx);
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    this.open_item_menu(repo, &for_menu, ev.position, window, cx)
                }),
            )
            .child(
                octicon(
                    if current {
                        Octicon::Check
                    } else {
                        Octicon::FileDirectory
                    },
                    if current { t.text } else { t.text_secondary },
                )
                .flex_none()
                .mr(SPACING_HALF),
            )
            .child(
                div()
                    .flex_grow(2.)
                    .min_w_0()
                    .max_w(gpui_kit::relative(0.65))
                    .mr(SPACING_HALF)
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .mr(SPACING_HALF)
                    .text_right()
                    .whitespace_nowrap()
                    .truncate()
                    .text_size(FONT_SIZE_SM)
                    .text_color(t.text_secondary)
                    .child(description),
            )
            .into_any_element()
    }
}

impl Render for WorktreeFoldout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let query = self.filter.read(cx).value().trim().to_string();
        let (id, worktrees, current) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else {
                return div().into_any_element();
            };
            let worktrees = s
                .repo_states
                .get(&id)
                .map(|rs| rs.worktrees.clone())
                .unwrap_or_default();
            (id, worktrees, current_worktree(s, id).map(|w| w.path))
        };
        let matches = |w: &WorktreeEntry| {
            query.is_empty()
                || w.display_name()
                    .to_lowercase()
                    .contains(&query.to_lowercase())
        };
        let main: Vec<&WorktreeEntry> = worktrees
            .iter()
            .filter(|w| w.kind == WorktreeType::Main && matches(w))
            .collect();
        let linked: Vec<&WorktreeEntry> = worktrees
            .iter()
            .filter(|w| w.kind == WorktreeType::Linked && matches(w))
            .collect();
        let group_header = |label: &str| {
            // `.filter-list-group-header`
            div()
                .h(ROW_HEIGHT)
                .pt(SPACING)
                .px(SPACING)
                .flex()
                .items_center()
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(FONT_SIZE)
                .truncate()
                .child(label.to_string())
        };
        let mut groups: Vec<AnyElement> = Vec::new();
        for (label, items) in [("Main Worktree", main), ("Linked Worktrees", linked)] {
            if items.is_empty() {
                continue;
            }
            groups.push(
                div()
                    .flex()
                    .flex_col()
                    .child(group_header(label))
                    .children(items.into_iter().map(|w| {
                        let is_current = current.as_ref() == Some(&w.path);
                        self.row(id, w, is_current, &query, cx)
                    }))
                    .into_any_element(),
            );
        }
        div()
            .id("worktrees-container")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                // `.filter-field-row`: [🔍 Filter][New Worktree]
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING)
                    .p(SPACING)
                    .child(text_box(
                        "worktree-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .child(
                        button("new-worktree", "New Worktree", cx)
                            .flex_none()
                            .on_click(move |_, _, cx| {
                                Dispatcher::close_foldout(cx);
                                Dispatcher::show_popup(
                                    Popup::AddWorktree {
                                        repo: id,
                                        initial_branch_name: None,
                                        initial_worktree_name: None,
                                    },
                                    cx,
                                );
                            }),
                    ),
            )
            .child(if groups.is_empty() {
                // `.no-items-found`
                div()
                    .p(SPACING)
                    .text_center()
                    .text_size(FONT_SIZE)
                    .text_color(t.text_secondary)
                    .child("No worktrees found")
                    .into_any_element()
            } else {
                div()
                    .id("worktrees-list")
                    .role(Role::List)
                    .aria_label("Worktrees")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(groups)
                    .with_scrollbar()
                    .into_any_element()
            })
            .children(self.context_menu.clone())
            .into_any_element()
    }
}

/// Where a new worktree goes by default (GHD `RepositoryPath` → clone dir).
pub fn default_worktree_dir(state: &AppState) -> PathBuf {
    state
        .settings
        .clone_dir
        .clone()
        .unwrap_or_else(corvane_platform::paths::default_clone_dir)
}
