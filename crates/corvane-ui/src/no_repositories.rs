//! `#no-repositories` - first-launch blankslate ("Let's get started!").
//! `styles/ui/_no-repositories.scss`, `ui/no-repositories/no-repositories-view.tsx`.
//! Signed in, the first button creates the tutorial repository, or returns
//! to a paused tutorial (`renderTutorialRepositoryButton`).
//!
//! Layout: a column centred in the window at its content width (the action
//! pane's max-content, which the pane's `width: 50%` then halves), the
//! Welcome's two graphics pinned top-right / bottom-right behind it, all
//! zoomed 1.2–1.5 by window width (`@media … { zoom }`).
//!
//! Signed in, the account's cloneable repositories fill a second pane on the
//! left (`renderRepositoryList`): the `AccountPicker` with 2+ accounts, the
//! `CloneableRepositoryFilterList` and, once one is selected, "Clone
//! *owner/name*", which opens Clone a Repository on that URL. The section
//! then spans the window and each pane takes half; the list keeps the
//! height left under the header and the Clone button hangs below it,
//! stretching the actions pane with it, as GHD lays it out.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use corvane_core::{Account, AppState, Dispatcher, GitHubRepository, Popup};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::cloneable_repositories::{
    AccountPickerState, ListStyle, account_picker, account_popover, group_rows, no_items,
    refresh_button, repository_list,
};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// The Clone button's `autoFocus` ring, until a mouse press takes focus
/// without `:focus-visible`.
static CLONE_FOCUS_VISIBLE: AtomicBool = AtomicBool::new(true);

/// `#no-repositories { zoom }` for the window width.
fn zoom(viewport_width: Pixels) -> f32 {
    let w = unzoom(viewport_width);
    if w >= 1800. {
        1.5
    } else if w >= 1600. {
        1.4
    } else if w >= 1400. {
        1.3
    } else if w >= 1366. {
        1.2
    } else {
        1.
    }
}

fn big_button(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    z: f32,
    cx: &App,
) -> Stateful<Div> {
    button_impl(id, icon, label, on_click, false, z, cx)
}

/// `type="submit"`: the blue variant of [`big_button`].
fn primary_big_button(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    z: f32,
    cx: &App,
) -> Stateful<Div> {
    button_impl(id, icon, label, on_click, true, z, cx)
}

fn button_impl(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    primary: bool,
    z: f32,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let s = |v: f32| zpx(v * z);
    let (bg, border, text, hover_bg, hover_border) = if primary {
        (
            t.button_background,
            t.button_background,
            t.button_text,
            t.button_hover_background,
            t.button_hover_background,
        )
    } else {
        (
            t.secondary_button_background,
            t.secondary_button_border,
            t.secondary_button_text,
            t.secondary_button_hover_background,
            t.secondary_button_hover_border,
        )
    };
    // `.button-group span .button-component`: 10 px padding, a 24 px icon
    // with 5 / 10 / 5 / 5 px margins, the title wrapping beside it
    div()
        .id(id)
        .w_full()
        .p(s(10.))
        .flex()
        .flex_row()
        .items_center()
        .border_1()
        .rounded(s(6.))
        .bg(bg)
        .border_color(border)
        .text_color(text)
        .text_size(s(12.))
        .line_height(s(14.17))
        .cursor_pointer()
        .hover(move |st| st.bg(hover_bg).border_color(hover_border))
        .on_mouse_down(MouseButton::Left, |_, _, _| {
            CLONE_FOCUS_VISIBLE.store(false, Ordering::Relaxed)
        })
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(
            octicon(icon, text)
                .flex_none()
                .size(s(24.))
                .mt(s(5.))
                .mb(s(5.))
                .ml(s(5.))
                .mr(s(10.)),
        )
        .child(div().flex_1().min_w_0().child(label))
}

/// A `.button-group span`: 10 px below each button; `ring` draws the
/// `autoFocus` outline 2 px outside the border.
fn slot(button: Stateful<Div>, ring: bool, z: f32, cx: &App) -> Div {
    let s = |v: f32| zpx(v * z);
    div()
        .relative()
        .mb(s(10.))
        .when(ring, |d| {
            d.child(
                div()
                    .absolute()
                    .top(-s(4.))
                    .left(-s(4.))
                    .right(-s(4.))
                    .bottom(-s(4.))
                    .border(s(2.))
                    .border_color(cx.ghd().focus)
                    .rounded(s(10.)),
            )
        })
        .child(button)
}

/// `renderTutorialRepositoryButton`: only when signed in.
fn tutorial_button(z: f32, cx: &App) -> Option<Stateful<Div>> {
    let s = corvane_core::AppState::try_global(cx)?.read(cx);
    if s.accounts.is_empty() {
        return None;
    }
    Some(
        if s.selected_tutorial_step() == corvane_core::tutorial::TutorialStep::Paused {
            primary_big_button(
                "nr-tutorial",
                Octicon::MortarBoard,
                "Return to In Progress Tutorial",
                |_, cx| Dispatcher::resume_tutorial(cx),
                z,
                cx,
            )
        } else {
            primary_big_button(
                "nr-tutorial",
                Octicon::MortarBoard,
                "Create a Tutorial Repository…",
                |_, cx| Dispatcher::show_create_tutorial_repository(cx),
                z,
                cx,
            )
        },
    )
}

/// `NoRepositoriesView`: the filter text, selected repository and picked
/// account of the signed-in list.
pub struct NoRepositoriesView {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    /// `selectedRepository`.
    selected: Option<GitHubRepository>,
    /// `selectedAccount` as `(endpoint, login)`; the first account otherwise.
    picked: Option<(String, String)>,
    picker: AccountPickerState,
    /// The list takes focus from the filter when a row is clicked.
    list_focus: FocusHandle,
    /// The filter's `autoFocus` is due: the list pane has just appeared.
    autofocus: bool,
    /// GitHub Desktop's data directory exists (the import button, flag 206).
    ghd_installed: bool,
    /// Endpoints whose list this view asked for (GHD asks while the
    /// account's state is undefined; after a failed load it has one).
    requested: std::collections::HashSet<String>,
}

impl NoRepositoriesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter your repositories"));
        cx.observe(&filter, |this, _, cx| {
            this.filter_changed(cx);
            cx.notify()
        })
        .detach();
        let picker = AccountPickerState::new(window, cx);
        cx.observe(&picker.filter, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            filter,
            selected: None,
            picked: None,
            picker,
            list_focus: cx.focus_handle(),
            autofocus: true,
            ghd_installed: corvane_core::ghd_import::github_desktop_installed(),
            requested: Default::default(),
        }
    }

    /// `selectedAccount ?? accounts[0]`.
    fn account(&self, cx: &App) -> Option<Account> {
        let accounts = &self.state.read(cx).accounts;
        self.picked
            .as_ref()
            .and_then(|(endpoint, login)| {
                accounts
                    .iter()
                    .find(|a| a.endpoint == *endpoint && a.login == *login)
            })
            .or_else(|| crate::cloneable_repositories::default_account(accounts, cx))
            .cloned()
    }

    /// `ensureRepositoriesForAccount` (on mount and when the account
    /// changes): load once, when nothing is known yet. Only while the list
    /// shows, so a signed-in user with repositories never fetches it.
    fn ensure_loaded(&mut self, account: &Account, cx: &mut Context<Self>) {
        let known = {
            let s = self.state.read(cx);
            s.api_repositories.contains_key(&account.endpoint)
                || s.api_repositories_loading.contains(&account.endpoint)
        };
        if !known && self.requested.insert(account.endpoint.clone()) {
            let account = account.clone();
            cx.defer(move |cx| Dispatcher::load_api_repositories(account, cx));
        }
    }

    /// `onSelectionChanged { kind: 'filter' }`: a filter that hides the
    /// selection selects the first match instead (or nothing).
    fn filter_changed(&mut self, cx: &mut Context<Self>) {
        let Some(account) = self.account(cx) else {
            return;
        };
        let query = self.filter.read(cx).value().to_string();
        let rows = match self.state.read(cx).api_repositories.get(&account.endpoint) {
            Some(repos) => group_rows(
                repos,
                &account.login,
                &crate::cloneable_repositories::filter_query(&query, cx),
            ),
            None => return,
        };
        let selected = self.selected.as_ref().map(|r| r.clone_url.clone());
        if let Some(next) =
            crate::cloneable_repositories::filtered_selection(&rows, &query, selected.as_deref())
        {
            self.selected = next;
        }
    }

    /// `onCloneSelectedRepository`: Clone a Repository on the selected URL.
    fn clone_selected(&self, cx: &mut App) {
        if let Some(repo) = &self.selected {
            Dispatcher::show_popup(
                Popup::CloneRepository {
                    url: Some(repo.clone_url.clone()),
                },
                cx,
            );
        }
    }

    /// `.content-pane.repository-list`, built inside `with_zoom(z, …)`:
    /// the picker, the filter row and the list in `list_h`, then the Clone
    /// button below.
    fn repository_pane(
        &self,
        account: &Account,
        list_h: Pixels,
        z: f32,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let t = cx.ghd();
        let accounts = self.state.read(cx).accounts.clone();
        let (loading, loaded, rows) = {
            let s = self.state.read(cx);
            let repos = s.api_repositories.get(&account.endpoint);
            (
                s.api_repositories_loading.contains(&account.endpoint),
                repos.is_some(),
                repos
                    .map(|r| {
                        group_rows(
                            r,
                            &account.login,
                            &crate::cloneable_repositories::filter_query(
                                &self.filter.read(cx).value(),
                                cx,
                            ),
                        )
                    })
                    .unwrap_or_default(),
            )
        };
        let query = self.filter.read(cx).value().trim().to_string();
        let list: AnyElement = if rows.is_empty() {
            no_items(
                "nr-refresh-link",
                account,
                loading,
                loaded,
                &query,
                false,
                cx,
            )
        } else {
            let weak = cx.weak_entity();
            repository_list(
                "nr-repository-list",
                Rc::new(rows),
                self.selected.as_ref().map(|r| r.clone_url.clone()),
                ListStyle {
                    inset: 10.,
                    small_headers: false,
                    zoom: z,
                    focused: self.list_focus.is_focused(window),
                },
                Rc::new(move |repo, window, cx| {
                    let repo = repo.clone();
                    weak.update(cx, |this, cx| {
                        window.focus(&this.list_focus, cx);
                        this.selected = Some(repo);
                        cx.notify();
                    })
                    .ok();
                }),
            )
        };
        let picker = (accounts.len() > 1).then(|| {
            let weak = cx.weak_entity();
            account_picker(
                "nr-account-picker",
                account,
                &self.picker,
                move |window, cx| {
                    weak.update(cx, |this, cx| {
                        this.picker.open = !this.picker.open;
                        // the popover's filter box has `autoFocus`
                        if this.picker.open {
                            let handle = this.picker.filter.read(cx).focus_handle(cx);
                            window.focus(&handle, cx);
                        }
                        cx.notify();
                    })
                    .ok();
                },
                cx,
            )
            .mb(SPACING())
        });
        let popover = (picker.is_some() && self.picker.open).then(|| {
            let pick = cx.weak_entity();
            let close = cx.weak_entity();
            account_popover(
                "nr-account",
                &self.picker,
                crate::cloneable_repositories::PopoverPlacement {
                    scale: z,
                    gap: 8.,
                    fixed_height: true,
                },
                accounts.clone(),
                Some(account),
                Rc::new(move |account, _, cx| {
                    let key = (account.endpoint.clone(), account.login.clone());
                    pick.update(cx, |this, cx| {
                        this.picked = Some(key);
                        this.picker.open = false;
                        cx.notify();
                    })
                    .ok();
                }),
                Rc::new(move |_, cx| {
                    close
                        .update(cx, |this, cx| {
                            this.picker.open = false;
                            cx.notify();
                        })
                        .ok();
                }),
                window,
                cx,
            )
        });
        let clone_button = self.selected.as_ref().map(|repo| {
            let weak = cx.weak_entity();
            crate::widgets::primary_button(
                "nr-clone-selected",
                div()
                    .flex()
                    .flex_row()
                    .child("Clone\u{a0}")
                    .child(div().font_weight(FontWeight::BOLD).child(repo.full_name())),
                false,
                cx,
            )
            .mt(SPACING())
            .w_full()
            .on_click(move |_, _, cx| {
                weak.update(cx, |this, cx| this.clone_selected(cx)).ok();
            })
        });
        div()
            .w_1_2()
            .flex_none()
            .pr(SPACING())
            .flex()
            .flex_col()
            .child(
                div()
                    .h(list_h)
                    .flex()
                    .flex_col()
                    .children(picker)
                    .children(popover)
                    .child(
                        // `.filter-field-row`: the filter box and refresh
                        div()
                            .flex_none()
                            .mb(SPACING())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING())
                            .child(crate::widgets::filter_text_box(
                                "nr-filter",
                                &self.filter,
                                Some(octicon(Octicon::Search, t.text_secondary)),
                                window,
                                cx,
                            ))
                            .child(
                                refresh_button("nr-refresh", account, loading, cx).px(SPACING()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .track_focus(&self.list_focus)
                            .child(list),
                    ),
            )
            .children(clone_button)
    }
}

impl Render for NoRepositoriesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let account = self.account(cx);
        if let Some(account) = &account {
            self.ensure_loaded(account, cx);
        }
        // the filter's `autoFocus`, each time the list pane mounts
        if account.is_none() {
            self.autofocus = true;
        } else if self.autofocus {
            self.autofocus = false;
            let handle = self.filter.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
        no_repositories(self, account, window, cx)
    }
}

fn no_repositories(
    view: &NoRepositoriesView,
    account: Option<Account>,
    window: &Window,
    cx: &Context<NoRepositoriesView>,
) -> impl IntoElement + use<> {
    let t = cx.ghd();
    let viewport = window.viewport_size();
    let z = zoom(viewport.width);
    let s = |v: f32| zpx(v * z);
    let signed_in =
        corvane_core::AppState::try_global(cx).is_some_and(|s| !s.read(cx).accounts.is_empty());
    let ring = !signed_in && CLONE_FOCUS_VISIBLE.load(Ordering::Relaxed);
    // `103-product-name`; GHD's ProTip says "add it to Desktop" (the last word)
    let name = corvane_core::AppState::try_global(cx)
        .map(|s| s.read(cx).product_name().to_string())
        .unwrap_or_else(|| "Corvane".into());
    let short_name = name
        .split_whitespace()
        .last()
        .unwrap_or("Corvane")
        .to_string();
    // `height: 20%` / `40%` of the view, width from the SVGs' aspect,
    // capped at 20 % / 40 % of its width
    let top_h = (viewport.height * 0.2).min(viewport.width * 0.2 * (43.835956 / 42.2971));
    let bottom_h = (viewport.height * 0.4).min(viewport.width * 0.4 * (172.30263 / 113.99702));
    // what is left under the header: the view less its padding, the header
    // (63 + 18 px lines) and its 40 px margin
    let list_h = viewport.height - s(60.) * 2. - s(63. + 18.) - s(40.);
    let repository_pane = account
        .as_ref()
        .map(|account| with_zoom(z, || view.repository_pane(account, list_h, z, window, cx)));
    let with_list = repository_pane.is_some();
    // Corvane extra (flag 206): GitHub Desktop's data is on this machine
    let show_import = view.ghd_installed
        && view
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::IMPORT_FROM_GITHUB_DESKTOP);
    div()
        .id("no-repositories")
        .relative()
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_col()
        .items_center()
        .p(s(60.))
        .bg(t.background)
        .child(
            img("illustrations/welcome-illustration-left-top.svg")
                .absolute()
                .right(s(80.))
                .top(s(40.))
                .h(top_h)
                .w(top_h * (42.2971 / 43.835956)),
        )
        .child(
            img("illustrations/welcome-illustration-left-bottom.svg")
                .absolute()
                .right(s(10.))
                .bottom(s(10.))
                .h(bottom_h)
                .w(bottom_h * (113.99702 / 172.30263)),
        )
        .child(
            // `section`: as wide as the action pane's content (536 px for
            // these titles), the full height; with the repository list, as
            // wide as the view
            div()
                .flex_1()
                .map(|d| if with_list { d.w_full() } else { d.w(s(536.1)) })
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    // header
                    div()
                        .flex()
                        .flex_col()
                        .mb(s(40.))
                        .child(
                            div()
                                .text_size(s(42.))
                                .line_height(s(63.))
                                .font_weight(FontWeight::LIGHT)
                                .child("Let's get started!"),
                        )
                        .child(
                            div().text_size(s(12.)).line_height(s(18.)).child(format!(
                                "Add a repository to {name} to start collaborating"
                            )),
                        ),
                )
                .child(
                    // `.content`: at least the height left, taller when the
                    // Clone button hangs under the list
                    div()
                        .flex()
                        .flex_row()
                        .map(|d| {
                            if with_list {
                                d.flex_none().min_h(list_h)
                            } else {
                                d.flex_1()
                            }
                        })
                        .children(repository_pane)
                        .child(
                    // `.content > .content-pane`: 50 %, the button group
                    // growing above the ProTip
                    div()
                        .w_1_2()
                        .flex_none()
                        .when(with_list, |d| d.pl(s(10.)))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .children(tutorial_button(z, cx).map(|b| slot(b, false, z, cx)))
                                .child(slot(
                                    big_button(
                                        "nr-clone",
                                        Octicon::RepoClone,
                                        "Clone a Repository from the Internet…",
                                        |_, cx| {
                                            Dispatcher::show_popup(
                                                Popup::CloneRepository { url: None },
                                                cx,
                                            )
                                        },
                                        z,
                                        cx,
                                    ),
                                    ring,
                                    z,
                                    cx,
                                ))
                                .child(slot(
                                    big_button(
                                        "nr-create",
                                        Octicon::Plus,
                                        "Create a New Repository on your Local Drive…",
                                        |_, cx| {
                                            Dispatcher::show_popup(
                                                Popup::CreateRepository { path: None },
                                                cx,
                                            )
                                        },
                                        z,
                                        cx,
                                    ),
                                    false,
                                    z,
                                    cx,
                                ))
                                .child(slot(
                                    big_button(
                                        "nr-add",
                                        Octicon::FileDirectory,
                                        "Add an Existing Repository from your Local Drive…",
                                        |_, cx| {
                                            Dispatcher::show_popup(
                                                Popup::AddExistingRepository { path: None },
                                                cx,
                                            )
                                        },
                                        z,
                                        cx,
                                    ),
                                    false,
                                    z,
                                    cx,
                                ))
                                // Corvane extra (flag 206)
                                .when(show_import, |d| {
                                    d.child(slot(
                                        big_button(
                                            "nr-import-ghd",
                                            Octicon::DesktopDownload,
                                            "Import Repositories from GitHub Desktop…",
                                            |_, cx| {
                                                Dispatcher::show_popup(
                                                    Popup::ImportFromGitHubDesktop,
                                                    cx,
                                                )
                                            },
                                            z,
                                            cx,
                                        ),
                                        false,
                                        z,
                                        cx,
                                    ))
                                }),
                        )
                        .child(
                            // `.drag-drop-info`
                            div()
                                .p(s(20.))
                                .flex()
                                .flex_row()
                                .rounded(s(6.))
                                .border_2()
                                .border_dashed()
                                .border_color(t.tip_box_border)
                                .bg(t.tip_box_background)
                                .text_size(s(12.))
                                .line_height(s(18.))
                                .child(
                                    octicon(Octicon::LightBulb, t.text)
                                        .flex_none()
                                        .size(s(16.))
                                        .mr(s(10.)),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(
                                        // `<strong>ProTip!</strong>` inline in the sentence
                                        StyledText::new(format!(
                                            "ProTip! You can drag & drop an existing repository \
                                             folder here to add it to {short_name}"
                                        ))
                                        .with_highlights(
                                            [(
                                                0..7,
                                                HighlightStyle {
                                                    font_weight: Some(FontWeight::SEMIBOLD),
                                                    ..Default::default()
                                                },
                                            )],
                                        ),
                                    ),
                                ),
                        ),
                ),
                ),
        )
}
