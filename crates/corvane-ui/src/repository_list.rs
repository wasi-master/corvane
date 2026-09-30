//! Repository foldout: filter + "Add ▾", then grouped 29 px rows
//! (`ui/repositories-list/*.tsx`, `styles/ui/_repository-list.scss`).

use std::collections::HashMap;

use corvane_core::{AppState, Dispatcher, Popup, Repository};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{FilterListPick, SelectNextFile, SelectPreviousFile};
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
    /// Corvane (`207-repository-status-filter`): only repositories with
    /// uncommitted changes / commits to push or pull.
    only_changed: bool,
    only_ahead_behind: bool,
    /// Corvane (`208-repository-fork-filter`): only forks / only the rest.
    only_forks: bool,
    only_sources: bool,
    /// GHD `FilterList` keyboard selection: the row ↓ / ↑ moved to from
    /// the filter box (an index into the rows as shown, groups flattened).
    highlighted: Option<usize>,
    scroll: ScrollHandle,
}

struct Group {
    title: SharedString,
    repos: Vec<Repository>,
}

impl RepositoryFoldout {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.observe(&filter, |this: &mut Self, _, cx| {
            this.highlighted = None;
            cx.notify()
        })
        .detach();
        Self {
            state,
            filter,
            add_menu_open: false,
            only_changed: false,
            only_ahead_behind: false,
            only_forks: false,
            only_sources: false,
            highlighted: None,
            scroll: ScrollHandle::new(),
        }
    }

    pub fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.highlighted = None;
        let handle = self.filter.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        // Corvane (`210-repository-filter-selects-text`): the remembered
        // filter text is selected, so typing replaces it
        if self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::REPOSITORY_FILTER_SELECTS_TEXT)
        {
            self.filter
                .update(cx, |input, cx| input.select_all(window, cx));
        }
    }

    /// GHD `FilterList`: ↓ / ↑ in the filter box move through the rows (↑
    /// from the filter starts at the last), clamped at the ends.
    fn move_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let groups = self.groups(cx);
        let count: usize = groups.iter().map(|g| g.repos.len()).sum();
        if count == 0 {
            return;
        }
        let ix = match self.highlighted {
            Some(ix) => (ix as isize + delta).clamp(0, count as isize - 1) as usize,
            None if delta < 0 => count - 1,
            None => 0,
        };
        self.highlighted = Some(ix);
        // rows and group headers are all `ROW_HEIGHT`: scroll the row in
        let mut headers = 0;
        let mut before = 0;
        for group in &groups {
            if !group.title.is_empty() {
                headers += 1;
            }
            if ix < before + group.repos.len() {
                break;
            }
            before += group.repos.len();
        }
        let top = ROW_HEIGHT() * (headers + ix) as f32;
        let bottom = top + ROW_HEIGHT();
        let height = self.scroll.bounds().size.height;
        let mut offset = self.scroll.offset();
        if top < -offset.y {
            offset.y = -top;
        } else if bottom > -offset.y + height {
            offset.y = height - bottom;
        }
        self.scroll.set_offset(offset);
        cx.notify();
    }

    /// Enter in the filter box: the highlighted row, else the first one.
    fn pick_highlighted(&mut self, cx: &mut Context<Self>) {
        let ix = self.highlighted.unwrap_or(0);
        let id = self
            .groups(cx)
            .into_iter()
            .flat_map(|g| g.repos)
            .nth(ix)
            .map(|r| r.id);
        if let Some(id) = id {
            Dispatcher::select_repository(id, cx);
        }
    }

    /// GHD `groupRepositories`: Recent, then one group per GitHub owner, then Other.
    fn groups(&self, cx: &App) -> Vec<Group> {
        let state = self.state.read(cx);
        let raw_query = self.filter.read(cx).value().trim().to_string();
        let query = raw_query.to_lowercase();
        // Corvane (`211-regex-repository-filter`): `/pattern/`
        let regex = state
            .flags
            .bool(corvane_core::flags::ids::REGEX_REPOSITORY_FILTER)
            .then(|| corvane_core::filter::regex_query(&raw_query))
            .flatten();
        // Corvane (`207-repository-status-filter`)
        let status_filter = state
            .flags
            .bool(corvane_core::flags::ids::REPOSITORY_STATUS_FILTER)
            && (self.only_changed || self.only_ahead_behind);
        // Corvane (`208-repository-fork-filter`)
        let fork_filter = state
            .flags
            .bool(corvane_core::flags::ids::REPOSITORY_FORK_FILTER)
            && (self.only_forks || self.only_sources);
        let matches = |r: &Repository| {
            (query.is_empty()
                || match &regex {
                    Some(re) => re.is_match(&r.name()),
                    None => r.name().to_lowercase().contains(&query),
                })
                && (!status_filter || {
                    let (ahead_behind, has_changes) = indicators(state, r.id);
                    (self.only_changed && has_changes)
                        || (self.only_ahead_behind && ahead_behind.is_some())
                })
                && (!fork_filter || {
                    let fork = r.github.as_ref().is_some_and(|gh| gh.fork);
                    (self.only_forks && fork) || (self.only_sources && !fork)
                })
        };

        let mut groups: Vec<Group> = Vec::new();
        if query.is_empty() && !status_filter && !fork_filter {
            // Corvane (`209-recent-repositories-count`; GHD shows 3)
            let shown = usize::try_from(
                state
                    .flags
                    .number(corvane_core::flags::ids::RECENT_REPOSITORIES_COUNT),
            )
            .unwrap_or(3);
            let recent: Vec<Repository> = state
                .recent
                .iter()
                .take(shown)
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
        // Corvane (`212-flat-repository-results`): while a query is typed,
        // one list without group headers, best match first
        if !query.is_empty()
            && state
                .flags
                .bool(corvane_core::flags::ids::FLAT_REPOSITORY_RESULTS)
        {
            let mut repos: Vec<Repository> = groups.into_iter().flat_map(|g| g.repos).collect();
            let score = |r: &Repository| {
                corvane_core::filter::fuzzy_score(&query, &r.name()).unwrap_or(0.0)
            };
            // stable: equal scores keep the grouped order
            repos.sort_by(|a, b| score(b).total_cmp(&score(a)));
            return vec![Group {
                title: SharedString::default(),
                repos,
            }];
        }
        groups
    }

    fn row(
        &self,
        repo: &Repository,
        selected: bool,
        highlighted: bool,
        detail: Option<String>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
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
        let behind_accent = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::REPOSITORY_LIST_BEHIND_ACCENT);
        let (ahead_behind, has_changes) = indicators(self.state.read(cx), id);
        // Corvane (`214-repository-list-branch`): the checked-out branch
        // (the loaded state for an opened repository, else the background
        // indicator refresh) joins the dimmed detail
        let branch = {
            let s = self.state.read(cx);
            s.flags
                .bool(corvane_core::flags::ids::REPOSITORY_LIST_BRANCH)
                .then(|| {
                    s.repo_states
                        .get(&id)
                        .and_then(|rs| rs.info.as_ref())
                        .and_then(|i| i.current_branch())
                        .map(|b| b.name.clone())
                        .or_else(|| s.indicators.get(&id).and_then(|i| i.branch.clone()))
                })
                .flatten()
        };
        let detail = match (detail, branch) {
            (Some(folder), Some(branch)) => Some(format!("{folder} · {branch}")),
            (folder, branch) => folder.or(branch),
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
            // the keyboard row (GHD's focused-list selection)
            .when(highlighted, |d| {
                d.bg(t.box_selected_active_background)
                    .text_color(t.box_selected_active_text)
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
                    .child({
                        // Corvane (`213-duplicate-names-show-path`): the
                        // telling folders, dimmed, after the name
                        let name = repo.name();
                        match detail {
                            Some(detail) => {
                                let start = name.len() + 2;
                                let text = format!("{name}  {detail}");
                                let end = text.len();
                                let dim = HighlightStyle {
                                    color: Some(if selected || highlighted {
                                        t.box_selected_text
                                    } else {
                                        t.text_secondary
                                    }),
                                    ..Default::default()
                                };
                                StyledText::new(text)
                                    .with_highlights([(start..end, dim)])
                                    .into_any_element()
                            }
                            None => name.into_any_element(),
                        }
                    }),
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
                                        // flag `215-repository-list-behind-accent`:
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

    /// Corvane (`207-repository-status-filter`): the filter options menu.
    fn open_filter_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::context_menu::MenuItem;
        let this = cx.entity().downgrade();
        let toggle = move |pick: fn(&mut Self) -> &mut bool| {
            let this = this.clone();
            move |_: &mut Window, cx: &mut App| {
                this.update(cx, |f, cx| {
                    let flag = pick(f);
                    *flag = !*flag;
                    cx.notify();
                })
                .ok();
            }
        };
        let flags = &self.state.read(cx).flags;
        let mut items = Vec::new();
        if flags.bool(corvane_core::flags::ids::REPOSITORY_STATUS_FILTER) {
            items.extend([
                MenuItem::checkbox(
                    "Uncommitted changes",
                    self.only_changed,
                    toggle(|f| &mut f.only_changed),
                ),
                MenuItem::checkbox(
                    "Commits to push or pull",
                    self.only_ahead_behind,
                    toggle(|f| &mut f.only_ahead_behind),
                ),
            ]);
        }
        // Corvane (`208-repository-fork-filter`)
        if flags.bool(corvane_core::flags::ids::REPOSITORY_FORK_FILTER) {
            if !items.is_empty() {
                items.push(MenuItem::separator());
            }
            items.extend([
                MenuItem::checkbox("Forks", self.only_forks, toggle(|f| &mut f.only_forks)),
                MenuItem::checkbox(
                    "Not forks",
                    self.only_sources,
                    toggle(|f| &mut f.only_sources),
                ),
            ]);
        }
        #[cfg(target_os = "macos")]
        crate::native_menu::show_context_menu(items, position, window, cx);
        #[cfg(not(target_os = "macos"))]
        let _ = (items, position, window);
    }

    /// Corvane (`225-clone-prefills-filter`): the filter text that Add ›
    /// Clone Repository… puts in the clone dialog's filter box.
    fn clone_filter(&self, cx: &App) -> Option<String> {
        let text = self.filter.read(cx).value().trim().to_string();
        let enabled = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::CLONE_PREFILLS_FILTER);
        (enabled && !text.is_empty()).then_some(text)
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
        // `262-view-on-remote`: "View on Remote" for other hosts
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
    // Corvane (`216-remove-all-missing-repositories`): on a missing row,
    // remove every repository Corvane cannot find (without confirmation,
    // as GHD removes one missing repository)
    let missing_ids: Vec<u64> = state
        .repositories
        .iter()
        .filter(|r| r.missing)
        .map(|r| r.id)
        .collect();
    if missing
        && missing_ids.len() > 1
        && state
            .flags
            .bool(corvane_core::flags::ids::REMOVE_ALL_MISSING_REPOSITORIES)
    {
        items.push(MenuItem::new(
            format!("Remove All {} Missing Repositories", missing_ids.len()),
            move |_, cx| {
                Dispatcher::close_foldout(cx);
                for id in &missing_ids {
                    Dispatcher::remove_repository(*id, cx);
                }
            },
        ));
    }
    items
}

/// The Add button's items (`onNewRepositoryButtonClick`).
#[cfg(target_os = "macos")]
fn add_menu_items(clone_filter: Option<String>) -> Vec<crate::context_menu::MenuItem> {
    use crate::context_menu::MenuItem;
    vec![
        MenuItem::new("Clone Repository…", move |_, cx| {
            // Corvane (`225-clone-prefills-filter`)
            if let Some(text) = &clone_filter {
                crate::dialogs::clone_repository::prefill_filter(text.clone());
            }
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
        // the filter button: `207-repository-status-filter` or
        // `208-repository-fork-filter`
        let (status_filter, filtering) = {
            let flags = &self.state.read(cx).flags;
            let status = flags.bool(corvane_core::flags::ids::REPOSITORY_STATUS_FILTER);
            let fork = flags.bool(corvane_core::flags::ids::REPOSITORY_FORK_FILTER);
            (
                status || fork,
                (status && (self.only_changed || self.only_ahead_behind))
                    || (fork && (self.only_forks || self.only_sources)),
            )
        };
        let highlighted = self.highlighted;
        let mut row_ix = 0;
        let show_paths = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::DUPLICATE_NAMES_SHOW_PATH);

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
                    .key_context("RepositoryFilter")
                    .on_action(
                        cx.listener(|this, _: &SelectNextFile, _, cx| this.move_highlight(1, cx)),
                    )
                    .on_action(cx.listener(|this, _: &SelectPreviousFile, _, cx| {
                        this.move_highlight(-1, cx)
                    }))
                    .on_action(
                        cx.listener(|this, _: &FilterListPick, _, cx| this.pick_highlighted(cx)),
                    )
                    .child(crate::widgets::filter_text_box(
                        "repo-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    // Corvane (`207-repository-status-filter`): a menu of
                    // status filters, blue while one is on
                    .when(status_filter, |d| {
                        d.child(
                            button("repository-filter-options", "", cx)
                                .flex_none()
                                .ghd_tooltip("Filter options")
                                .child(octicon(
                                    Octicon::Filter,
                                    if filtering {
                                        t.tab_bar_active
                                    } else {
                                        t.secondary_button_text
                                    },
                                ))
                                .on_click(cx.listener(|this, ev: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.open_filter_menu(ev.position(), window, cx);
                                })),
                        )
                    })
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
                                    let clone_filter = this.clone_filter(cx);
                                    crate::native_menu::show_context_menu(
                                        add_menu_items(clone_filter),
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
                        let first = row_ix;
                        row_ix += group.repos.len();
                        let mut details = if show_paths {
                            duplicate_name_paths(&group.repos)
                        } else {
                            HashMap::new()
                        };
                        // a repository can be listed under Recent and its
                        // owner: the group id keeps the rows' ids (and a11y
                        // nodes) unique
                        div()
                            .id(("repo-group", group_ix))
                            .flex()
                            .flex_col()
                            // a flat result list (`212-flat-repository-results`)
                            // has no header
                            .when(!group.title.is_empty(), |d| {
                                d.child(
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
                                        .child(group.title.clone()),
                                )
                            })
                            .children(group.repos.iter().enumerate().map(|(ix, repo)| {
                                self.row(
                                    repo,
                                    selected == Some(repo.id),
                                    highlighted == Some(first + ix),
                                    details.remove(&repo.id),
                                    cx,
                                )
                            }))
                    }))
                    .with_scrollbar_handle(&self.scroll),
            )
            .when(add_open, |d| d.child(self.add_menu(cx)))
    }
}

/// The row's indicators: ahead / behind (when either is non-zero) and
/// whether there are uncommitted changes, from the loaded state or the
/// background indicator refresh.
fn indicators(s: &AppState, id: u64) -> (Option<corvane_core::AheadBehind>, bool) {
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
    (ab, changes)
}

/// Corvane (`213-duplicate-names-show-path`): for repositories whose names
/// repeat within `repos`, the trailing directories of their parent paths
/// that tell them apart (`fork-a` for `~/fork-a/app` beside `~/fork-b/app`).
fn duplicate_name_paths(repos: &[Repository]) -> HashMap<u64, String> {
    let parents = |r: &Repository| -> Vec<String> {
        r.path
            .parent()
            .map(|p| {
                p.components()
                    .rev()
                    .filter_map(|c| match c {
                        std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut out = HashMap::new();
    for repo in repos {
        let name = repo.name().to_lowercase();
        let others: Vec<Vec<String>> = repos
            .iter()
            .filter(|r| r.id != repo.id && r.name().to_lowercase() == name)
            .map(parents)
            .collect();
        if others.is_empty() {
            continue;
        }
        let mine = parents(repo);
        for k in 1..=mine.len() {
            let unique = others.iter().all(|p| p.len() < k || p[..k] != mine[..k]);
            if unique || k == mine.len() {
                let shown: Vec<&str> = mine[..k].iter().rev().map(String::as_str).collect();
                out.insert(repo.id, shown.join("/"));
                break;
            }
        }
    }
    out
}

/// Corvane (`612-navigation-shortcuts`): the repositories in the list's
/// order without the Recent group (owner groups by owner, then Other; by
/// name within a group), for ⇧⌘] / ⇧⌘[.
pub fn list_order(state: &AppState) -> Vec<u64> {
    let mut repos = state.sorted_repositories();
    repos.sort_by_key(|r| match &r.github {
        Some(gh) => (0, gh.owner.to_lowercase()),
        None => (1, String::new()),
    });
    repos.iter().map(|r| r.id).collect()
}

/// The repository `step` places after `current` in `order`, wrapping.
pub fn step_repository(order: &[u64], current: Option<u64>, step: isize) -> Option<u64> {
    if order.is_empty() {
        return None;
    }
    let n = order.len() as isize;
    let next = match current.and_then(|id| order.iter().position(|r| *r == id)) {
        Some(ix) => (ix as isize + step).rem_euclid(n),
        None if step < 0 => n - 1,
        None => 0,
    };
    order.get(next as usize).copied()
}

/// GHD `commitGrammar`: "1 commit" / "N commits".
fn commit_grammar(n: u32) -> String {
    if n == 1 {
        "1 commit".to_string()
    } else {
        format!("{n} commits")
    }
}

#[cfg(test)]
mod tests {
    use super::{duplicate_name_paths, step_repository};

    fn repo(id: u64, path: &str) -> corvane_core::Repository {
        corvane_core::Repository::new(id, path)
    }

    #[test]
    fn duplicate_names_get_the_parent_dirs_that_differ() {
        let repos = [
            repo(1, "/w/fork-a/app"),
            repo(2, "/w/fork-b/app"),
            repo(3, "/x/src/lib"),
            repo(4, "/y/src/lib"),
            repo(5, "/w/solo"),
        ];
        let paths = duplicate_name_paths(&repos);
        assert_eq!(paths.get(&1).map(String::as_str), Some("fork-a"));
        assert_eq!(paths.get(&2).map(String::as_str), Some("fork-b"));
        assert_eq!(paths.get(&3).map(String::as_str), Some("x/src"));
        assert_eq!(paths.get(&4).map(String::as_str), Some("y/src"));
        assert!(!paths.contains_key(&5));
    }

    #[test]
    fn steps_wrap_around_the_list() {
        let order = [3, 1, 2];
        assert_eq!(step_repository(&order, Some(1), 1), Some(2));
        assert_eq!(step_repository(&order, Some(2), 1), Some(3));
        assert_eq!(step_repository(&order, Some(3), -1), Some(2));
        assert_eq!(step_repository(&order, None, 1), Some(3));
        assert_eq!(step_repository(&order, None, -1), Some(2));
        assert_eq!(step_repository(&[], Some(1), 1), None);
    }
}
