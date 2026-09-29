//! "Clone a Repository" (`ui/clone-repository/clone-repository.tsx`):
//! tabs GitHub.com | GitHub Enterprise | URL. The account tabs list the
//! account's repositories (`clone-github-repository.tsx`,
//! `cloneable-repository-filter-list.tsx`, grouped as in
//! `group-repositories.ts`); the URL tab takes any clone URL. The local path
//! is validated like `validateEmptyFolder` and errors show as a `DialogError`.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use corvane_core::{Account, AppState, Dispatcher, GitHubRepository, Popup};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, dialog_error_banner, labeled, link_button, primary_button, text_box};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    DotCom,
    Enterprise,
    Url,
}

/// `RowHeight` of the cloneable repository list.
const LIST_ROW_HEIGHT: Pixels = px(31.);
const LIST_HEIGHT: Pixels = px(290.);

/// One row of the flattened, filtered repository list.
#[derive(Clone)]
enum CloneRow {
    Header(String),
    Item(GitHubRepository),
}

pub struct CloneRepositoryDialog {
    state: Entity<AppState>,
    tab: Tab,
    url: Entity<InputState>,
    path: Entity<InputState>,
    filter: Entity<InputState>,
    /// The user edited the path by hand; stop deriving it from the URL.
    path_edited: bool,
    last_derived: String,
    initial_path: String,
    /// `clone_url` of the list item picked on an account tab.
    selected_repo: Option<String>,
    /// `validateEmptyFolder` result for the current path.
    path_error: Option<&'static str>,
}

impl CloneRepositoryDialog {
    pub fn new(
        state: Entity<AppState>,
        initial_url: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let clone_dir = state
            .read(cx)
            .settings
            .clone_dir
            .clone()
            .unwrap_or_else(corvane_platform::paths::default_clone_dir);
        let url = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("URL or username/repository");
            if let Some(u) = &initial_url {
                s = s.default_value(u.clone());
            }
            s
        });
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("repository path")
                .default_value(clone_dir.display().to_string())
        });
        let filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter your repositories"));
        cx.observe_in(&url, window, |this, _, window, cx| {
            this.derive_path(window, cx);
            this.validate(cx);
            cx.notify()
        })
        .detach();
        cx.observe(&path, |this, _, cx| {
            this.validate(cx);
            cx.notify()
        })
        .detach();
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let tab = if initial_url.is_some() {
            Tab::Url
        } else {
            Tab::DotCom
        };
        let handle = if tab == Tab::Url {
            url.read(cx).focus_handle(cx)
        } else {
            filter.read(cx).focus_handle(cx)
        };
        window.focus(&handle, cx);
        let initial_path = clone_dir.display().to_string();
        let this = Self {
            state,
            tab,
            url,
            path,
            filter,
            path_edited: false,
            last_derived: initial_path.clone(),
            initial_path,
            selected_repo: None,
            path_error: None,
        };
        this.ensure_loaded(cx);
        this
    }

    fn account(&self, cx: &App) -> Option<Account> {
        let s = self.state.read(cx);
        match self.tab {
            Tab::DotCom => s.dotcom_account().cloned(),
            Tab::Enterprise => s.accounts.iter().find(|a| !a.is_dotcom()).cloned(),
            Tab::Url => None,
        }
    }

    /// Fetch the account's repositories once per dialog tab.
    fn ensure_loaded(&self, cx: &mut Context<Self>) {
        let Some(account) = self.account(cx) else {
            return;
        };
        let loaded = {
            let s = self.state.read(cx);
            s.api_repositories.contains_key(&account.endpoint)
                || s.api_repositories_loading.contains(&account.endpoint)
        };
        if !loaded {
            Dispatcher::load_api_repositories(account, cx);
        }
    }

    fn set_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        let handle = if tab == Tab::Url {
            self.url.read(cx).focus_handle(cx)
        } else {
            self.filter.read(cx).focus_handle(cx)
        };
        window.focus(&handle, cx);
        self.ensure_loaded(cx);
        cx.notify();
    }

    /// GHD keeps `<clone dir>/<repo name>` in sync with the URL until edited.
    fn derive_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.path.read(cx).value().to_string();
        if current != self.last_derived {
            self.path_edited = true;
        }
        if self.path_edited {
            return;
        }
        let base = self
            .state
            .read(cx)
            .settings
            .clone_dir
            .clone()
            .unwrap_or_else(corvane_platform::paths::default_clone_dir);
        let url = self.url.read(cx).value().to_string();
        let derived = match corvane_git::normalize_clone_url(&url)
            .and_then(|u| corvane_git::repository_name_from_url(&u))
        {
            Some(name) => base.join(name),
            None => base,
        }
        .display()
        .to_string();
        if derived != current {
            self.last_derived = derived.clone();
            self.path
                .update(cx, |s, cx| s.set_value(derived, window, cx));
        }
    }

    /// `validatePath`: nothing to say while the path is still the default and
    /// no URL was entered; otherwise `validateEmptyFolder`.
    fn validate(&mut self, cx: &App) {
        let path = self.path.read(cx).value().trim().to_string();
        let url_empty = self.url.read(cx).value().trim().is_empty();
        self.path_error = if path == self.initial_path && url_empty {
            None
        } else {
            validate_empty_folder(Path::new(&path))
        };
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Clone".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(p) = paths.into_iter().next()
            {
                this.update_in(cx, |d, window, cx| {
                    d.path_edited = true;
                    d.path
                        .update(cx, |s, cx| s.set_value(p.display().to_string(), window, cx));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn clone_target(&self, cx: &App) -> Option<(String, PathBuf)> {
        if self.tab != Tab::Url && self.selected_repo.is_none() {
            return None;
        }
        let url = corvane_git::normalize_clone_url(&self.url.read(cx).value())?;
        let path = self.path.read(cx).value().trim().to_string();
        if path.is_empty() {
            return None;
        }
        Some((url, PathBuf::from(path)))
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.path_error.is_some() {
            return;
        }
        if let Some((url, path)) = self.clone_target(cx) {
            Dispatcher::clone_repository(url, path, cx);
        }
    }

    /// `onSelectionChanged` on the repository list: the clone URL drives the
    /// path like a typed URL would.
    fn select_repository(
        &mut self,
        repo: &GitHubRepository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_repo = Some(repo.clone_url.clone());
        let url = repo.clone_url.clone();
        self.url.update(cx, |s, cx| s.set_value(url, window, cx));
        cx.notify();
    }

    /// `groupRepositories` + the filter: "Your Repositories" first, then the
    /// other owners alphabetically, items by name.
    fn rows(&self, account: &Account, cx: &App) -> Vec<CloneRow> {
        let s = self.state.read(cx);
        let Some(repos) = s.api_repositories.get(&account.endpoint) else {
            return Vec::new();
        };
        let query = self.filter.read(cx).value().trim().to_lowercase();
        let mut mine: Vec<&GitHubRepository> = Vec::new();
        let mut others: std::collections::BTreeMap<String, Vec<&GitHubRepository>> =
            std::collections::BTreeMap::new();
        for repo in repos {
            if !query.is_empty() && !repo.full_name().to_lowercase().contains(&query) {
                continue;
            }
            if repo.owner.eq_ignore_ascii_case(&account.login) {
                mine.push(repo);
            } else {
                others
                    .entry(repo.owner.to_lowercase())
                    .or_default()
                    .push(repo);
            }
        }
        let mut rows = Vec::new();
        let mut push_group = |title: String, mut items: Vec<&GitHubRepository>| {
            if items.is_empty() {
                return;
            }
            items.sort_by_key(|r| r.name.to_lowercase());
            rows.push(CloneRow::Header(title));
            rows.extend(items.into_iter().map(|r| CloneRow::Item(r.clone())));
        };
        push_group("Your Repositories".to_string(), mine);
        for (_, items) in others {
            let owner = items.first().map(|r| r.owner.clone()).unwrap_or_default();
            push_group(owner, items);
        }
        rows
    }

    fn url_tab(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_THIRD)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child("Repository URL or GitHub username and repository")
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .child("(")
                                    .child(
                                        div()
                                            .font_family(crate::theme::MONO_FONT)
                                            .px(px(3.))
                                            .rounded(px(3.))
                                            .bg(t.box_alt_background)
                                            .child("hubot/cool-repo"),
                                    )
                                    .child(")"),
                            ),
                    )
                    .child(text_box("clone-url", &self.url, None, window, cx)),
            )
            .child(self.path_row(window, cx))
    }

    /// `.local-path-field`: Local Path + Choose…
    fn path_row(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_end()
            .gap(SPACING)
            .child(labeled(
                "Local Path",
                text_box("clone-path", &self.path, None, window, cx),
                cx,
            ))
            .child(
                button("clone-choose", "Choose…", cx)
                    .flex_none()
                    .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
            )
    }

    fn account_tab(&self, enterprise: bool, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let host = if enterprise {
            "GitHub Enterprise"
        } else {
            "GitHub.com"
        };
        let Some(account) = self.account(cx) else {
            let enterprise_flag = enterprise;
            return div()
                .flex()
                .flex_col()
                .items_start()
                .gap(SPACING)
                .py(SPACING)
                .child(format!(
                    "Sign in to your {host} account to access your repositories."
                ))
                .child(
                    primary_button("clone-sign-in", "Sign In", false, cx).on_click(
                        move |_, _, cx| {
                            Dispatcher::show_popup(
                                Popup::SignIn {
                                    enterprise: enterprise_flag,
                                },
                                cx,
                            )
                        },
                    ),
                )
                .into_any_element();
        };
        let loading = self
            .state
            .read(cx)
            .api_repositories_loading
            .contains(&account.endpoint);
        let loaded = self
            .state
            .read(cx)
            .api_repositories
            .contains_key(&account.endpoint);
        let query = self.filter.read(cx).value().trim().to_string();
        let rows = Rc::new(self.rows(&account, cx));
        let friendly = account.host();
        let refresh_account = account.clone();
        let list: AnyElement = if rows.is_empty() {
            // `renderNoItems`
            let message: AnyElement = if loading && !loaded {
                div()
                    .text_color(t.text_secondary)
                    .child(format!("Loading repositories from {friendly}…"))
                    .into_any_element()
            } else if !query.is_empty() {
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .justify_center()
                    .child("Sorry, I can't find any repository matching\u{a0}")
                    .child(
                        div()
                            .font_family(crate::theme::MONO_FONT)
                            .px(px(3.))
                            .rounded(px(3.))
                            .bg(t.box_alt_background)
                            .child(query.clone()),
                    )
                    .into_any_element()
            } else {
                let account_for_link = account.clone();
                crate::widgets::paragraph(vec![
                    "Looks like there are no repositories for ".into(),
                    div()
                        .font_family(crate::theme::MONO_FONT)
                        .px(px(3.))
                        .rounded(px(3.))
                        .bg(t.box_alt_background)
                        .child(account.login.clone())
                        .into_any_element()
                        .into(),
                    format!(" on {friendly}. ").into(),
                    link_button("clone-refresh-link", "Refresh this list", cx)
                        .on_click(move |_, _, cx| {
                            Dispatcher::load_api_repositories(account_for_link.clone(), cx)
                        })
                        .into_any_element()
                        .into(),
                    " if you've created a repository recently.".into(),
                ])
                .justify_center()
                .into_any_element()
            };
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(SPACING_DOUBLE)
                .text_size(FONT_SIZE)
                .text_align(TextAlign::Center)
                .child(message)
                .into_any_element()
        } else {
            let weak = cx.weak_entity();
            let selected = self.selected_repo.clone();
            let query_lower = query.to_lowercase();
            uniform_list(
                "clone-repository-list",
                rows.len(),
                move |range, _window, cx| {
                    let t = cx.ghd();
                    range
                        .map(|ix| match &rows[ix] {
                            CloneRow::Header(title) => div()
                                .id(ix)
                                .h(LIST_ROW_HEIGHT)
                                .px(SPACING)
                                .flex()
                                .items_center()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(FONT_SIZE_SM)
                                .text_color(t.text_secondary)
                                .child(title.clone())
                                .into_any_element(),
                            CloneRow::Item(repo) => {
                                let is_selected =
                                    selected.as_deref() == Some(repo.clone_url.as_str());
                                let icon = if repo.private {
                                    Octicon::Lock
                                } else if repo.fork {
                                    Octicon::RepoForked
                                } else {
                                    Octicon::Repo
                                };
                                let weak = weak.clone();
                                let repo_for_click = repo.clone();
                                let hover_bg = t.list_item_hover_background;
                                let text = repo.full_name();
                                div()
                                    .id(ix)
                                    .h(LIST_ROW_HEIGHT)
                                    .px(SPACING)
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(SPACING_HALF)
                                    .cursor_pointer()
                                    .text_size(FONT_SIZE)
                                    .when(is_selected, |d| {
                                        d.bg(t.box_selected_active_background)
                                            .text_color(t.box_selected_active_text)
                                    })
                                    .when(!is_selected, move |d| d.hover(move |s| s.bg(hover_bg)))
                                    .on_click(move |_, window, cx| {
                                        let repo = repo_for_click.clone();
                                        weak.update(cx, |this, cx| {
                                            this.select_repository(&repo, window, cx)
                                        })
                                        .ok();
                                    })
                                    .child(octicon(
                                        icon,
                                        if is_selected {
                                            t.box_selected_active_text
                                        } else {
                                            t.text
                                        },
                                    ))
                                    .child(highlighted(&text, &query_lower))
                                    .when(repo.archived, |d| {
                                        // `.archived` badge
                                        d.child(
                                            div()
                                                .flex_none()
                                                .ml(SPACING_HALF)
                                                .px(px(3.))
                                                .py(px(1.))
                                                .rounded(BORDER_RADIUS)
                                                .border_1()
                                                .border_color(t.box_border_contrast)
                                                .text_size(FONT_SIZE_XS)
                                                .child("ARCHIVED"),
                                        )
                                    })
                                    .into_any_element()
                            }
                        })
                        .collect()
                },
            )
            .size_full()
            .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .child(
                // filter row: text box + refresh (`renderPostFilter`)
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING)
                    .child(text_box(
                        "clone-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .child(
                        button("clone-refresh", "", cx)
                            .flex_none()
                            .px(SPACING_HALF)
                            .when(loading, |d| d.opacity(0.6))
                            .on_click(move |_, _, cx| {
                                if !loading {
                                    Dispatcher::load_api_repositories(refresh_account.clone(), cx)
                                }
                            })
                            .child(octicon(Octicon::Sync, t.secondary_button_text)),
                    ),
            )
            .child(
                div()
                    .h(LIST_HEIGHT)
                    .w_full()
                    .border_1()
                    .border_color(t.box_border)
                    .rounded(BORDER_RADIUS)
                    .overflow_hidden()
                    .child(list),
            )
            .child(self.path_row(window, cx))
            .into_any_element()
    }
}

/// `validateEmptyFolder`: the destination must be missing or an empty folder.
fn validate_empty_folder(path: &Path) -> Option<&'static str> {
    if path.as_os_str().is_empty() {
        return None;
    }
    match std::fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                Some("This folder contains files. Git can only clone to empty folders.")
            } else {
                None
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) if err.kind() == std::io::ErrorKind::NotADirectory => {
            Some("There is already a file with this name. Git can only clone to a folder.")
        }
        Err(_) => Some("Unable to read path on disk. Please check the path and try again."),
    }
}

/// `HighlightText`: the matched part of the name in bold.
fn highlighted(text: &str, query_lower: &str) -> Div {
    let row = div().flex_1().min_w_0().truncate().flex().flex_row();
    if query_lower.is_empty() {
        return row.child(text.to_string());
    }
    match text.to_lowercase().find(query_lower) {
        Some(start)
            if text.is_char_boundary(start) && text.is_char_boundary(start + query_lower.len()) =>
        {
            let end = start + query_lower.len();
            row.child(text[..start].to_string())
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(text[start..end].to_string()),
                )
                .child(text[end..].to_string())
        }
        _ => row.child(text.to_string()),
    }
}

impl Render for CloneRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_clone = self.path_error.is_none() && self.clone_target(cx).is_some();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let this = cx.entity();
        let selected = match self.tab {
            Tab::DotCom => 0,
            Tab::Enterprise => 1,
            Tab::Url => 2,
        };
        let tabs_entity = cx.entity();
        let body: AnyElement = match self.tab {
            Tab::DotCom => self.account_tab(false, window, cx),
            Tab::Enterprise => self.account_tab(true, window, cx),
            Tab::Url => self.url_tab(window, cx).into_any_element(),
        };
        let error = self.path_error;

        dialog(
            "clone-repository",
            "Clone a Repository",
            div()
                .flex()
                .flex_col()
                .w(px(560.))
                .when_some(error, |d, message| {
                    d.child(dialog_error_banner(message, cx))
                })
                .child(div().mb(SPACING).child(tab_bar(
                    vec![
                        TabModel {
                            id: "clone-tab-dotcom",
                            label: "GitHub.com".into(),
                            count: None,
                        },
                        TabModel {
                            id: "clone-tab-enterprise",
                            label: "GitHub Enterprise".into(),
                            count: None,
                        },
                        TabModel {
                            id: "clone-tab-url",
                            label: "URL".into(),
                            count: None,
                        },
                    ],
                    selected,
                    move |ix, window, cx| {
                        tabs_entity.update(cx, |d, cx| {
                            let tab = match ix {
                                0 => Tab::DotCom,
                                1 => Tab::Enterprise,
                                _ => Tab::Url,
                            };
                            d.set_tab(tab, window, cx);
                        })
                    },
                    cx,
                )))
                .child(body),
            vec![
                DialogButton {
                    id: "clone-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "clone-ok",
                    label: "Clone".into(),
                    primary: true,
                    disabled: !can_clone,
                    on_click: Box::new(move |_, cx| {
                        if can_clone {
                            this.update(cx, |d, cx| d.submit(cx));
                        }
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
