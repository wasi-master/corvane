//! Repository foldout: filter + "Add ▾", then grouped 29 px rows
//! (`ui/repositories-list/*.tsx`, `styles/ui/_repository-list.scss`).

use corvane_core::{AppState, Dispatcher, Popup, Repository};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, text_box};

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
        let icon = match &repo.github {
            Some(gh) if gh.fork => Octicon::RepoForked,
            Some(gh) if gh.private => Octicon::Lock,
            Some(_) => Octicon::Repo,
            None => Octicon::DeviceDesktop,
        };
        let id = repo.id;
        let hover_bg = t.list_item_hover_background;
        let ahead_behind = self
            .state
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|s| s.ahead_behind)
            .filter(|ab| ab.ahead > 0 || ab.behind > 0);
        div()
            .id(("repo-row", id))
            .h(ROW_HEIGHT)
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px(SPACING)
            .cursor_pointer()
            .when(selected, |d| {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            })
            .when(!selected, move |d| d.hover(move |s| s.bg(hover_bg)))
            .on_click(move |_, _, cx| Dispatcher::select_repository(id, cx))
            .child(octicon(icon, t.text).mr(SPACING_HALF))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(FONT_SIZE)
                    .when(repo.alias.is_some(), |d| d.italic())
                    .child(repo.name()),
            )
            .when_some(ahead_behind, |d, ab| {
                d.child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(2.))
                        .px(px(5.))
                        .h(px(13.))
                        .rounded(px(8.))
                        .bg(t.list_item_badge_background)
                        .text_color(t.list_item_badge_text)
                        .text_size(FONT_SIZE_XS)
                        .line_height(px(11.))
                        .when(ab.ahead > 0, |d| {
                            d.child(format!("{}", ab.ahead)).child(
                                octicon(Octicon::ArrowUp, t.list_item_badge_text).size(px(9.)),
                            )
                        })
                        .when(ab.behind > 0, |d| {
                            d.child(format!("{}", ab.behind)).child(
                                octicon(Octicon::ArrowDown, t.list_item_badge_text).size(px(9.)),
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
                .h(ROW_HEIGHT)
                .px(SPACING)
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
            .top(SPACING + TEXT_FIELD_HEIGHT + px(4.))
            .right(SPACING)
            .w(px(240.))
            .py(px(4.))
            .flex()
            .flex_col()
            .bg(t.box_background)
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS)
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
                    .gap(SPACING)
                    .p(SPACING)
                    .child(text_box(
                        "repo-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .child(
                        button("add-repository", "Add", cx)
                            .flex_none()
                            .gap(px(5.))
                            .child(
                                octicon(Octicon::TriangleDown, t.secondary_button_text)
                                    .size(px(12.)),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_menu_open = !this.add_menu_open;
                                cx.stop_propagation();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("repository-list-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .when(!has_repos, |d| {
                        d.child(
                            div()
                                .p(SPACING)
                                .text_color(t.text_secondary)
                                .child("No repositories yet. Use Add to get started."),
                        )
                    })
                    .when(has_repos && groups.is_empty(), |d| {
                        d.child(
                            div()
                                .p(SPACING)
                                .w_full()
                                .text_center()
                                .text_color(t.text_secondary)
                                .child("Sorry, I can't find that repository"),
                        )
                    })
                    .children(groups.into_iter().map(|group| {
                        div()
                            .flex()
                            .flex_col()
                            .child(
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
                                    .child(group.title),
                            )
                            .children(
                                group
                                    .repos
                                    .iter()
                                    .map(|repo| self.row(repo, selected == Some(repo.id), cx)),
                            )
                    })),
            )
            .when(add_open, |d| d.child(self.add_menu(cx)))
    }
}
