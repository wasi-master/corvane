//! Repository foldout: filter + "Add ▾", then grouped 29 px rows
//! (`ui/repositories-list/*.tsx`, `styles/ui/_repository-list.scss`).

use corvane_core::{AppState, Dispatcher, Popup, Repository};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::GhdTooltip;
use crate::widgets::ListRowA11y;
use crate::widgets::button;

pub struct RepositoryFoldout {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    add_menu_open: bool,
}

struct Group {
    title: SharedString,
    repos: Vec<Repository>,
}

impl RepositoryFoldout {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            filter,
            add_menu_open: false,
        }
    }

    pub fn focus_filter(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.filter.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    /// GHD `groupRepositories`: Recent, then one group per GitHub owner, then Other.
    fn groups(&self, cx: &App) -> Vec<Group> {
        let state = self.state.read(cx);
        let query = self.filter.read(cx).value().trim().to_lowercase();
        let matches = |r: &Repository| query.is_empty() || r.name().to_lowercase().contains(&query);

        let mut groups: Vec<Group> = Vec::new();
        if query.is_empty() {
            let recent: Vec<Repository> = state
                .recent
                .iter()
                .filter_map(|id| state.repository(*id).cloned())
                .collect();
            if !recent.is_empty() && state.repositories.len() > 1 {
                groups.push(Group {
                    title: "Recent".into(),
                    repos: recent,
                });
            }
        }

        let mut owners: Vec<(String, Vec<Repository>)> = Vec::new();
        let mut other: Vec<Repository> = Vec::new();
        for repo in state.sorted_repositories() {
            if !matches(repo) {
                continue;
            }
            match &repo.github {
                Some(gh) => match owners.iter_mut().find(|(o, _)| *o == gh.owner) {
                    Some((_, list)) => list.push(repo.clone()),
                    None => owners.push((gh.owner.clone(), vec![repo.clone()])),
                },
                None => other.push(repo.clone()),
            }
        }
        owners.sort_by_key(|(o, _)| o.to_lowercase());
        for (owner, repos) in owners {
            groups.push(Group {
                title: owner.into(),
                repos,
            });
        }
        if !other.is_empty() {
            groups.push(Group {
                title: "Other".into(),
                repos: other,
            });
        }
        groups
    }

    fn row(&self, repo: &Repository, selected: bool, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        // GHD `iconForRepository`
        let icon = match &repo.github {
            _ if repo.missing => Octicon::Alert,
            Some(gh) if gh.private => Octicon::Lock,
            Some(gh) if gh.fork => Octicon::RepoForked,
            Some(_) => Octicon::Repo,
            None => Octicon::DeviceDesktop,
        };
        let id = repo.id;
        let hover_bg = t.list_item_hover_background;
        let (ahead_behind, has_changes, behind_accent) = {
            let s = self.state.read(cx);
            let indicator = s.indicators.get(&id);
            let rs = s.repo_states.get(&id);
            let ab = rs
                .and_then(|r| r.ahead_behind)
                .or_else(|| indicator.and_then(|i| i.ahead_behind))
                .filter(|ab| ab.ahead > 0 || ab.behind > 0);
            let changes = rs
                .and_then(|r| r.status.as_ref())
                .map(|st| !st.files.is_empty())
                .or_else(|| indicator.map(|i| i.changed_files > 0))
                .unwrap_or(false);
            (
                ab,
                changes,
                s.flags
                    .bool(corvane_core::flags::ids::REPOSITORY_LIST_BEHIND_ACCENT),
            )
        };
        // GHD `RepositoryListItem` aria label: name, changes, ahead/behind
        let mut label = repo.name();
        if has_changes {
            label.push_str(", uncommitted changes");
        }
        if let Some(ab) = ahead_behind {
            label.push_str(&format!(", {} ahead, {} behind", ab.ahead, ab.behind));
        }
        div()
            .id(("repo-row", id))
            .a11y_row(label, selected)
            .h(ROW_HEIGHT())
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px(SPACING())
            .cursor_pointer()
            .when(selected, |d| {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            })
            // `.list-item:hover` outranks `.list-item.selected` (flag 104 keeps it)
            .when(
                !(selected && crate::widgets::selection_keeps_colour_on_hover(cx)),
                move |d| d.hover(move |s| s.bg(hover_bg)),
            )
            // `renderTooltip`: the GitHub full name (or name) in bold, the
            // alias in parentheses, then the path
            .tooltip({
                let real = repo
                    .github
                    .as_ref()
                    .map(|gh| format!("{}/{}", gh.owner, gh.name))
                    .unwrap_or_else(|| repo.name());
                let bold = 0..real.len();
                let mut text = real;
                if let Some(alias) = &repo.alias {
                    text.push_str(&format!(" ({alias})"));
                }
                text.push('\n');
                text.push_str(&repo.path.to_string_lossy());
                crate::widgets::rich_tooltip(text, bold)
            })
            .tooltip_show_delay(crate::widgets::TOOLTIP_DELAY)
            .on_click(move |_, _, cx| Dispatcher::select_repository(id, cx))
            .on_mouse_down(MouseButton::Right, {
                let repo = repo.clone();
                move |ev: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    #[cfg(target_os = "macos")]
                    crate::native_menu::show_context_menu(
                        repository_menu_items(&repo, cx),
                        ev.position,
                        window,
                        cx,
                    );
                    #[cfg(not(target_os = "macos"))]
                    let _ = (ev, window, &repo);
                }
            })
            .child(octicon(icon, t.text).mr(SPACING_HALF()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(FONT_SIZE())
                    .when(repo.alias.is_some(), |d| d.italic())
                    .child(repo.name()),
            )
            // `.repo-indicators`: ahead / behind arrows, then the changes dot
            .when(has_changes || ahead_behind.is_some(), |d| {
                let (badge_bg, badge_text) = if selected {
                    (
                        t.list_item_selected_badge_background,
                        t.list_item_selected_badge_text,
                    )
                } else {
                    (t.list_item_badge_background, t.list_item_badge_text)
                };
                d.child(
                    div()
                        .flex_none()
                        .ml_auto()
                        .mr(SPACING_HALF())
                        .flex()
                        .flex_row()
                        .items_center()
                        .when_some(ahead_behind, |d, ab| {
                            // `renderAheadBehindIndicator`: arrows only, 12 px tall
                            let tooltip = format!(
                                "The currently checked out branch is{}{}{}its tracked branch.",
                                if ab.behind > 0 {
                                    format!(" {} behind ", commit_grammar(ab.behind))
                                } else {
                                    String::new()
                                },
                                if ab.behind > 0 && ab.ahead > 0 {
                                    "and"
                                } else {
                                    ""
                                },
                                if ab.ahead > 0 {
                                    format!(" {} ahead of ", commit_grammar(ab.ahead))
                                } else {
                                    String::new()
                                },
                            );
                            d.child(
                                div()
                                    .id(("repo-ahead-behind", id))
                                    .ghd_tooltip(tooltip)
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .h(zpx(12.))
                                    .px(zpx(6.))
                                    .rounded(zpx(8.))
                                    .bg(badge_bg)
                                    .when(ab.ahead > 0, |d| {
                                        d.child(
                                            octicon(Octicon::ArrowUp, badge_text).size(zpx(12.)),
                                        )
                                    })
                                    .when(ab.behind > 0, |d| {
                                        // flag `186-repository-list-behind-accent`:
                                        // commits to pull show in the success colour
                                        let color = if behind_accent && !selected {
                                            t.status_success
                                        } else {
                                            badge_text
                                        };
                                        d.child(octicon(Octicon::ArrowDown, color).size(zpx(12.)))
                                    }),
                            )
                        })
                        .when(has_changes, |d| {
                            // `.change-indicator-wrapper`: 5 px in, at least 12 px wide
                            d.child(
                                div()
                                    .id(("repo-changes", id))
                                    .ghd_tooltip("There are uncommitted changes in this repository")
                                    .ml(SPACING_HALF())
                                    .min_w(zpx(12.))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .child(octicon(Octicon::DotFill, t.tab_bar_active)),
                            )
                        }),
                )
            })
    }

    fn add_menu(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let item = |id: &'static str, label: &'static str, on_click: fn(&mut Window, &mut App)| {
            let hover_bg = t.box_hover_background;
            div()
                .id(id)
                .h(ROW_HEIGHT())
                .px(SPACING())
                .flex()
                .items_center()
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(move |_, window, cx| on_click(window, cx))
                .child(label)
        };
        div()
            .id("add-menu")
            .absolute()
            .top(SPACING() + TEXT_FIELD_HEIGHT() + zpx(4.))
            .right(SPACING())
            .w(zpx(240.))
            .py(zpx(4.))
            .flex()
            .flex_col()
            .bg(t.box_background)
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS())
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(item("add-clone", "Clone Repository…", |_, cx| {
                Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx)
            }))
            .child(item("add-create", "Create New Repository…", |_, cx| {
                Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx)
            }))
            .child(item(
                "add-existing",
                "Add Existing Repository…",
                |_, cx| {
                    Dispatcher::close_foldout(cx);
                    Dispatcher::prompt_add_repository(cx);
                },
            ))
    }
}

/// GHD `generateRepositoryListContextMenu`.
#[cfg(target_os = "macos")]
fn repository_menu_items(repo: &Repository, cx: &App) -> Vec<crate::context_menu::MenuItem> {
    use crate::context_menu::MenuItem;
    let state = AppState::global(cx).read(cx);
    let (editor, shell) = (state.editor_label(), state.shell_label());
    let confirm = state.settings.confirm_repository_removal;
    let id = repo.id;
    let remote_page = Dispatcher::non_github_remote_web_url(id, cx).is_some();
    let missing = repo.missing;
    let path = repo.path.clone();
    let (name, copy_path, shell_path, reveal, editor_path) = (
        repo.name(),
        path.to_string_lossy().to_string(),
        path.clone(),
        path.clone(),
        path,
    );
    let verb = if repo.alias.is_some() {
        "Change"
    } else {
        "Create"
    };
    let mut items = vec![MenuItem::new(format!("{verb} Alias"), move |_, cx| {
        Dispatcher::close_foldout(cx);
        Dispatcher::show_popup(Popup::ChangeRepositoryAlias { repo: id }, cx)
    })];
    if repo.alias.is_some() {
        items.push(MenuItem::new("Remove Alias", move |_, cx| {
            Dispatcher::change_repository_alias(id, None, cx)
        }));
    }
    items.extend([
        // `buildWorktreeMenuItems` (worktree support is on)
        MenuItem::new("Show Worktrees", move |_, cx| {
            Dispatcher::select_repository(id, cx);
            Dispatcher::toggle_foldout(corvane_core::Foldout::Worktree, cx);
        }),
        MenuItem::new("New Worktree…", move |_, cx| {
            Dispatcher::close_foldout(cx);
            Dispatcher::show_popup(
                Popup::AddWorktree {
                    repo: id,
                    initial_branch_name: None,
                    initial_worktree_name: None,
                },
                cx,
            )
        }),
        MenuItem::new("Copy Repo Name", move |_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(name.clone()))
        }),
        MenuItem::new("Copy Repo Path", move |_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()))
        }),
        MenuItem::separator(),
        // `425-view-on-remote`: "View on Remote" for other hosts
        MenuItem::new(
            if repo.github.is_none() && remote_page {
                "View on Remote"
            } else {
                "View on GitHub"
            },
            move |_, cx| Dispatcher::view_on_github(id, cx),
        )
        .enabled(repo.github.is_some() || remote_page),
        MenuItem::new(format!("Open in {shell}"), move |_, cx| {
            Dispatcher::open_in_shell(&shell_path, cx)
        })
        .enabled(!missing),
        MenuItem::new("Reveal in Finder", move |_, cx| {
            Dispatcher::show_in_finder(&reveal, cx)
        })
        .enabled(!missing),
        MenuItem::new(format!("Open in {editor}"), move |_, cx| {
            Dispatcher::open_in_editor(editor_path.clone(), cx)
        })
        .enabled(!missing),
        MenuItem::separator(),
        MenuItem::new(
            if confirm { "Remove…" } else { "Remove" },
            move |_, cx| {
                Dispatcher::close_foldout(cx);
                Dispatcher::request_remove_repository(id, cx)
            },
        ),
    ]);
    items
}

/// The Add button's items (`onNewRepositoryButtonClick`).
#[cfg(target_os = "macos")]
fn add_menu_items() -> Vec<crate::context_menu::MenuItem> {
    use crate::context_menu::MenuItem;
    vec![
        MenuItem::new("Clone Repository…", |_, cx| {
            Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx)
        }),
        MenuItem::new("Create New Repository…", |_, cx| {
            Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx)
        }),
        MenuItem::new("Add Existing Repository…", |_, cx| {
            Dispatcher::close_foldout(cx);
            Dispatcher::prompt_add_repository(cx);
        }),
    ]
}

impl Render for RepositoryFoldout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let selected = self.state.read(cx).selected;
        let groups = self.groups(cx);
        let has_repos = !self.state.read(cx).repositories.is_empty();
        let add_open = self.add_menu_open;

        div()
            .id("repository-list")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.add_menu_open {
                        this.add_menu_open = false;
                        cx.notify();
                    }
                }),
            )
            .child(
                // `.filter-field-row`: [🔍 Filter][Add ▾]
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING())
                    .p(SPACING())
                    .child(crate::widgets::filter_text_box(
                        "repo-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .child(
                        button("add-repository", "Add", cx)
                            .flex_none()
                            .gap(zpx(5.))
                            .child(
                                octicon(Octicon::TriangleDown, t.secondary_button_text)
                                    .size(zpx(12.)),
                            )
                            .on_click(cx.listener(|this, ev: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                // GHD `onNewRepositoryButtonClick`: a native
                                // contextual menu at the pointer
                                #[cfg(target_os = "macos")]
                                {
                                    let _ = this;
                                    crate::native_menu::show_context_menu(
                                        add_menu_items(),
                                        ev.position(),
                                        window,
                                        cx,
                                    );
                                }
                                #[cfg(not(target_os = "macos"))]
                                {
                                    let _ = (ev, window);
                                    this.add_menu_open = !this.add_menu_open;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .id("repository-list-scroll")
                    // a `List` node owning the repository rows
                    .role(Role::List)
                    .aria_label("Repositories")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .when(!has_repos, |d| {
                        d.child(
                            div()
                                .p(SPACING())
                                .text_color(t.text_secondary)
                                .child("No repositories yet. Use Add to get started."),
                        )
                    })
                    .when(has_repos && groups.is_empty(), |d| {
                        d.child(
                            div()
                                .p(SPACING())
                                .w_full()
                                .text_center()
                                .text_color(t.text_secondary)
                                .child("Sorry, I can't find that repository"),
                        )
                    })
                    .children(groups.into_iter().enumerate().map(|(group_ix, group)| {
                        // a repository can be listed under Recent and its
                        // owner: the group id keeps the rows' ids (and a11y
                        // nodes) unique
                        div()
                            .id(("repo-group", group_ix))
                            .flex()
                            .flex_col()
                            .child(
                                // `.filter-list-group-header`
                                div()
                                    .h(ROW_HEIGHT())
                                    .pt(SPACING())
                                    .px(SPACING())
                                    .flex()
                                    .items_center()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(FONT_SIZE())
                                    .truncate()
                                    .child(group.title),
                            )
                            .children(
                                group
                                    .repos
                                    .iter()
                                    .map(|repo| self.row(repo, selected == Some(repo.id), cx)),
                            )
                    }))
                    .with_scrollbar(),
            )
            .when(add_open, |d| d.child(self.add_menu(cx)))
    }
}

/// GHD `commitGrammar`: "1 commit" / "N commits".
fn commit_grammar(n: u32) -> String {
    if n == 1 {
        "1 commit".to_string()
    } else {
        format!("{n} commits")
    }
}
