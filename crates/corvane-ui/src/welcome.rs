//! First-launch Welcome flow (`ui/welcome/*`): Start → Configure Git.
//! `styles/ui/_welcome.scss`: left pane 60 % (40 px padding, content ≤ 500 px
//! × scale, the two graphics pinned behind it), right pane 40 % on `#28373b`
//! with the illustration cropped from the left; scale 1.2 at ≥ 1366×700.
//!
//! Configure Git (`ui/welcome/configure-git.tsx`, `lib/configure-git-user.tsx`):
//! signed in, "Use my GitHub account name and email address" (the first
//! account's name read-only, its emails in a `Select`) or "Configure
//! manually" (name + email text boxes and `GitEmailNotFoundWarning`).
//!
//! Deviations: the product name reads Corvane, and the start footer's
//! second paragraph says Corvane sends no usage metrics where GHD links to
//! its metrics page (see `.docs/deviations.md`).

use corvane_core::{AppState, Dispatcher, Popup};
use corvane_github::Endpoint;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    Inline, SelectHandler, avatar_image, avatar_lookup, link_button, paragraph, select_button,
};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::Input;

const SCALE: f32 = 1.2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Start,
    ConfigureGit,
}

pub struct WelcomeView {
    state: Entity<AppState>,
    step: Step,
    /// "Configure manually" name and email.
    name: Entity<InputState>,
    email: Entity<InputState>,
    /// `useGitHubAuthorInfo`: the account's name and email are used.
    use_github: bool,
    /// `gitHubName` (shown read-only) and `gitHubEmail`.
    github_name: Entity<InputState>,
    github_email: String,
    /// `accounts[0]` as `(endpoint, login)`, to notice a new first account.
    account_key: Option<(String, String)>,
    prefilled: bool,
    had_account: bool,
    /// "Sign in to GitHub.com" (`autoFocus`); its focus ring shows until a
    /// mouse press moves focus there without `:focus-visible`.
    sign_in_focus: FocusHandle,
    sign_in_focus_visible: bool,
    /// `autoFocus`: focus the sign-in button on the next render.
    autofocus: bool,
}

impl WelcomeView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Your Name"));
        let email = cx.new(|cx| InputState::new(window, cx).placeholder("your-email@example.com"));
        let github_name = cx.new(|cx| InputState::new(window, cx).placeholder("Your Name"));
        cx.observe(&email, |_, _, cx| cx.notify()).detach();
        let had_account = !state.read(cx).accounts.is_empty();
        let sign_in_focus = cx.focus_handle();
        // A successful sign-in advances to Configure Git (GHD `Welcome.componentWillReceiveProps`).
        cx.observe_in(&state, window, |this, state, window, cx| {
            let has_account = !state.read(cx).accounts.is_empty();
            if has_account && !this.had_account && this.step == Step::Start {
                this.advance(window, cx);
            } else if this.prefilled {
                this.sync_account(window, cx);
            }
            this.had_account = has_account;
            cx.notify();
        })
        .detach();
        Self {
            state,
            step: Step::Start,
            name,
            email,
            use_github: false,
            github_name,
            github_email: String::new(),
            account_key: None,
            prefilled: false,
            had_account,
            sign_in_focus,
            sign_in_focus_visible: true,
            autofocus: true,
        }
    }

    /// Configure Git › Cancel (`WelcomeStep.Start`).
    fn back_to_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step = Step::Start;
        window.focus(&self.sign_in_focus, cx);
        self.sign_in_focus_visible = true;
        self.autofocus = true;
        cx.notify();
    }

    fn advance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step = Step::ConfigureGit;
        if !self.prefilled {
            self.prefilled = true;
            let (git, account) = {
                let s = self.state.read(cx);
                (s.git.clone(), s.accounts.first().cloned())
            };
            let identity = git.map(corvane_git::global_identity).unwrap_or_default();
            // `ConfigureGitUser` constructor: the global config first, then
            // the account's name / login and preferred email
            let name = identity
                .name
                .clone()
                .filter(|n| !n.is_empty())
                .or_else(|| account.as_ref().and_then(|a| a.name.clone()))
                .filter(|n| !n.is_empty())
                .or_else(|| account.as_ref().map(|a| a.login.clone()))
                .unwrap_or_default();
            let email = identity
                .email
                .clone()
                .filter(|e| !e.is_empty())
                .or_else(|| account.as_ref().map(|a| a.preferred_email()))
                .unwrap_or_default();
            self.name.update(cx, |s, cx| s.set_value(name, window, cx));
            self.email
                .update(cx, |s, cx| s.set_value(email, window, cx));
            self.use_github = account.is_some();
            if let Some(account) = &account {
                self.set_github_info(account, window, cx);
            }
        }
        let handle = self.name.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    /// `gitHubName` / `gitHubEmail` from `account`.
    fn set_github_info(
        &mut self,
        account: &corvane_core::Account,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.account_key = Some((account.endpoint.clone(), account.login.clone()));
        let name = account
            .name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| account.login.clone());
        self.github_name
            .update(cx, |s, cx| s.set_value(name, window, cx));
        self.github_email = account.preferred_email();
    }

    /// `componentDidUpdate`: a new first account selects the account option
    /// and fills the manual fields that are still empty.
    fn sync_account(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(account) = self.state.read(cx).accounts.first().cloned() else {
            return;
        };
        if self.account_key.as_ref() == Some(&(account.endpoint.clone(), account.login.clone())) {
            return;
        }
        self.set_github_info(&account, window, cx);
        self.use_github = true;
        if self.name.read(cx).value().is_empty() {
            let name = self.github_name.read(cx).value().to_string();
            self.name.update(cx, |s, cx| s.set_value(name, window, cx));
        }
        if self.email.read(cx).value().is_empty() {
            let email = self.github_email.clone();
            self.email
                .update(cx, |s, cx| s.set_value(email, window, cx));
        }
        cx.notify();
    }

    fn set_use_github(&mut self, use_github: bool, cx: &mut Context<Self>) {
        self.use_github = use_github;
        cx.notify();
    }

    /// The name and email the example commit shows and Finish saves.
    fn author(&self, cx: &App) -> (String, String) {
        if self.use_github {
            (
                self.github_name.read(cx).value().to_string(),
                self.github_email.clone(),
            )
        } else {
            (
                self.name.read(cx).value().to_string(),
                self.email.read(cx).value().to_string(),
            )
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let (name, email) = self.author(cx);
        Dispatcher::set_global_identity(name, email, cx);
        Dispatcher::complete_welcome(cx);
    }

    /// `renderAuthorOptions`: the two `RadioButton`s, 5 px apart.
    fn author_options(
        &self,
        account: &corvane_core::Account,
        use_github: bool,
        cx: &Context<Self>,
    ) -> Div {
        let suffix = if account.is_dotcom() {
            ""
        } else {
            " Enterprise"
        };
        let github = cx.listener(|this, _, _, cx| this.set_use_github(true, cx));
        let manual = cx.listener(|this, _, _, cx| this.set_use_github(false, cx));
        div()
            .flex()
            .flex_col()
            .child(
                welcome_radio_row("welcome-use-github", use_github, cx)
                    .on_click(github)
                    .child(format!(
                        "Use my GitHub{suffix} account name and email address"
                    )),
            )
            .child(
                welcome_radio_row("welcome-configure-manually", !use_github, cx)
                    .mt(px(5.))
                    .on_click(manual)
                    .child("Configure manually"),
            )
    }

    /// `renderGitHubInfo`: the account's emails in a `Select`.
    fn github_email_select(&self, account: &corvane_core::Account, cx: &Context<Self>) -> Div {
        let t = cx.ghd();
        let mut emails = account.emails.clone();
        if emails.is_empty() {
            emails.push(self.github_email.clone());
        }
        let selected = emails.iter().position(|e| *e == self.github_email);
        let weak = cx.weak_entity();
        let choices = emails.clone();
        let on_select: SelectHandler = std::rc::Rc::new(move |ix, _, cx| {
            if let Some(email) = choices.get(ix).cloned() {
                weak.update(cx, |this, cx| {
                    this.github_email = email;
                    cx.notify();
                })
                .ok();
            }
        });
        welcome_field(
            "Email",
            // `#welcome select`: `--text-field-height`, 16.8 px text
            select_button(
                "welcome-github-email",
                self.github_email.clone(),
                emails.into_iter().map(SharedString::from).collect(),
                selected,
                false,
                on_select,
                cx,
            )
            .h(px(29. * SCALE))
            .rounded(BORDER_RADIUS())
            .bg(t.box_background)
            .text_size(px(WELCOME_FONT_MD)),
            cx,
        )
        .mt(px(10.))
    }

    fn start(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        // `103-product-name`: "GitHub Desktop" under the GitHub Desktop preset
        let name = self.state.read(cx).product_name().to_string();
        let this = cx.entity();
        let press = cx.entity();
        let ring = self.sign_in_focus_visible && self.sign_in_focus.is_focused(window);
        let focused = self.sign_in_focus.is_focused(window);
        let focus = self.sign_in_focus.clone();
        // `#start`: at least the pane's height less its padding, content
        // centred above the footer (`.start-content { margin-top: 100px }`)
        div()
            .min_h(window.viewport_size().height - px(80.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_grow(1.)
                    .mt(px(100.))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(welcome_title(format!("Welcome to {name}")))
                    .child(
                        welcome_text(format!(
                            "{name} is a seamless way to contribute to projects on GitHub and \
                             GitHub Enterprise. Sign in below to get started with your existing \
                             projects."
                        ))
                        .mt(px(10.)),
                    )
                    .child(
                        // `.welcome-main-buttons`
                        div()
                            .mt(px(40.))
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .child(
                                div()
                                    .relative()
                                    .mr(px(20.))
                                    .mb(px(10.))
                                    .when(ring, |d| d.child(crate::widgets::focus_ring(cx)))
                                    .child(
                                        welcome_button("welcome-sign-in", true, focused, cx)
                                            .track_focus(&focus)
                                            .px(px(20.))
                                            .child("Sign in to GitHub.com")
                                            .child(
                                                octicon(Octicon::LinkExternal, t.button_text)
                                                    .ml(px(10.)),
                                            )
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                press.update(cx, |w, cx| {
                                                    w.sign_in_focus_visible = false;
                                                    cx.notify();
                                                })
                                            })
                                            .on_click(|_, _, cx| {
                                                Dispatcher::show_popup(
                                                    Popup::SignIn { enterprise: false },
                                                    cx,
                                                );
                                                Dispatcher::begin_sign_in(
                                                    Endpoint::github_com(),
                                                    cx,
                                                );
                                            }),
                                    ),
                            )
                            .child(
                                welcome_button("welcome-sign-in-enterprise", false, false, cx)
                                    .px(px(20.))
                                    .mr(px(20.))
                                    .mb(px(10.))
                                    .child("Sign in to GitHub Enterprise")
                                    .on_click(|_, _, cx| {
                                        Dispatcher::show_popup(
                                            Popup::SignIn { enterprise: true },
                                            cx,
                                        )
                                    }),
                            ),
                    )
                    .child(
                        // `.skip-action-container`
                        div()
                            .mt(px(40.))
                            .flex()
                            .flex_col()
                            .text_size(px(WELCOME_FONT_MD))
                            .line_height(px(WELCOME_FONT_MD * 1.5))
                            .child(
                                paragraph(vec![
                                    "New to GitHub?".into(),
                                    link_button(
                                        "welcome-create-account",
                                        "Create your free account.",
                                        cx,
                                    )
                                    .text_size(px(WELCOME_FONT_MD))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .on_click(|_, _, cx| {
                                        cx.open_url("https://github.com/join?source=corvane")
                                    })
                                    .into_any_element()
                                    .into(),
                                ])
                                .gap_x(px(4.5))
                                .line_height(px(WELCOME_FONT_MD * 1.5))
                                .my(px(10.)),
                            )
                            .child(
                                div().flex().child(
                                    link_button("welcome-skip", "Skip this step", cx)
                                        .text_size(px(WELCOME_FONT_MD))
                                        .text_color(t.text_secondary)
                                        .on_click(move |_, window, cx| {
                                            this.update(cx, |w, cx| w.advance(window, cx))
                                        }),
                                ),
                            ),
                    ),
            )
            .child(
                // `.start-footer`: 13.2 px secondary text, paragraphs 13.2 px apart
                div()
                    .flex()
                    .flex_col()
                    .py(px(WELCOME_FONT_SM))
                    .gap(px(WELCOME_FONT_SM))
                    .text_size(px(WELCOME_FONT_SM))
                    .line_height(px(WELCOME_FONT_SM * 1.5))
                    .text_color(t.text_secondary)
                    .child(
                        paragraph(vec![
                            "By creating an account, you agree to the".into(),
                            footer_link(
                                "welcome-terms",
                                "Terms of Service",
                                "https://github.com/site/terms",
                                cx,
                            ),
                            ". For more information about GitHub's privacy practices, see the"
                                .into(),
                            footer_link(
                                "welcome-privacy",
                                "GitHub Privacy Statement.",
                                "https://github.com/site/privacy",
                                cx,
                            ),
                        ])
                        .line_height(px(WELCOME_FONT_SM * 1.5)),
                    )
                    .child(div().child(format!(
                        "{name} does not send usage metrics. Nothing about how you use the \
                         app is collected or shared with anyone."
                    ))),
            )
    }

    fn configure_git(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let finish = cx.entity();
        let cancel = cx.entity();
        let account = self.state.read(cx).accounts.first().cloned();
        let use_github = self.use_github && account.is_some();
        let (name, email) = self.author(cx);
        let when = std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 60);
        div()
            .flex()
            .flex_col()
            .child(welcome_title("Configure Git"))
            .child(welcome_text(
                "This is used to identify the commits you create. Anyone will be able to see \
                 this information if you publish commits.",
            ))
            .child(
                // `#configure-git-user`: the author options, the form (20 px
                // margins), then the example commit
                div()
                    .pb(px(10.))
                    .flex()
                    .flex_col()
                    .when_some(account.as_ref(), |d, account| {
                        d.child(self.author_options(account, use_github, cx))
                    })
                    .child(
                        div()
                            // after the options the form's 20 px top margin;
                            // after the text it collapses into its 10 px
                            .mt(px(if account.is_some() { 20. } else { 10. }))
                            .mb(px(20.))
                            .flex()
                            .flex_col()
                            .child(
                                // `.sign-in-form-inputs { min-height: 150px }`
                                div()
                                    .min_h(px(150.))
                                    .flex()
                                    .flex_col()
                                    .child(welcome_field(
                                        "Name",
                                        if use_github {
                                            welcome_read_only_box(
                                                "welcome-github-name",
                                                &self.github_name,
                                                cx,
                                            )
                                        } else {
                                            welcome_text_box("welcome-name", &self.name, window, cx)
                                        },
                                        cx,
                                    ))
                                    .map(|d| match (&account, use_github) {
                                        (Some(account), true) => {
                                            d.child(self.github_email_select(account, cx))
                                        }
                                        _ => d
                                            .child(
                                                welcome_field(
                                                    "Email",
                                                    welcome_text_box(
                                                        "welcome-email",
                                                        &self.email,
                                                        window,
                                                        cx,
                                                    ),
                                                    cx,
                                                )
                                                .mt(px(10.)),
                                            )
                                            .when_some(account.as_ref(), |d, account| {
                                                d.children(email_not_found_warning(
                                                    account, &email, cx,
                                                ))
                                            }),
                                    }),
                            )
                            .child(
                                // `Row`: Finish (submit) and Cancel; the inputs'
                                // 10 px bottom margin + the row's 10 px top one
                                div()
                                    .mt(px(20.))
                                    .flex()
                                    .flex_row()
                                    .child(
                                        welcome_button("welcome-finish", true, false, cx)
                                            .px(px(10.))
                                            .mr(px(10.))
                                            .child("Finish")
                                            .on_click(move |_, _, cx| {
                                                finish.update(cx, |w, cx| w.finish(cx))
                                            }),
                                    )
                                    .child(
                                        welcome_button("welcome-cancel", false, false, cx)
                                            .px(px(10.))
                                            .mr(px(10.))
                                            .child("Cancel")
                                            .on_click(move |_, window, cx| {
                                                cancel
                                                    .update(cx, |w, cx| w.back_to_start(window, cx))
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        // `#commit-list.commit-list-example`
                        div()
                            .flex()
                            .flex_col()
                            .border_1()
                            .border_color(t.box_border)
                            .rounded(BORDER_RADIUS())
                            .bg(rgb(0xffffff))
                            .overflow_hidden()
                            .child(
                                div()
                                    .px(px(10.))
                                    .py(px(5.))
                                    .bg(rgb(0xf2f8fe))
                                    .text_size(px(WELCOME_FONT_SM))
                                    .line_height(px(WELCOME_FONT_SM * 1.5))
                                    .child("Example commit"),
                            )
                            .child(
                                // `.commit`: 12 px × scale, at most 280 px wide
                                div()
                                    .h(px(50.4))
                                    .max_w(px(280.))
                                    .px(px(10.))
                                    .py(px(5.))
                                    .text_size(px(12. * SCALE))
                                    .line_height(px(12. * SCALE * 1.5))
                                    .child(
                                        div()
                                            .mt(px(-4.))
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .truncate()
                                                    .child("Fix all the things"),
                                            )
                                            .child(
                                                div()
                                                    .mt(px(3.))
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .child(
                                                        // `.AvatarStack--small`: avatar + 5 px
                                                        div().w(px(16. * SCALE + 5.)).child(
                                                            avatar_image(
                                                                avatar_lookup(&email, cx),
                                                                px(16. * SCALE),
                                                                cx,
                                                            ),
                                                        ),
                                                    )
                                                    .child(
                                                        div()
                                                            .min_w_0()
                                                            .truncate()
                                                            .text_size(px(WELCOME_FONT_SM))
                                                            .line_height(px(WELCOME_FONT_SM * 1.5))
                                                            .text_color(t.text_secondary)
                                                            .child(format!(
                                                                "{} • {}",
                                                                name,
                                                                crate::relative_time::relative(
                                                                    when
                                                                )
                                                            )),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

const WELCOME_FONT_SM: f32 = 11. * SCALE;
const WELCOME_FONT_MD: f32 = 14. * SCALE;

/// `.welcome-title`: 42 px × scale, light, line-height 1.25, 10 px below.
fn welcome_title(text: impl Into<SharedString>) -> Div {
    let text: SharedString = text.into();
    div()
        .text_size(px(42. * SCALE))
        .line_height(px(42. * SCALE * 1.25))
        .font_weight(FontWeight::LIGHT)
        .mb(px(10.))
        .child(text)
}

/// `.welcome-text`: 10 px vertical margins. In `.start-content` (a flex
/// column) they add to the title's; in `#configure-git` the top one
/// collapses into it, so callers add `mt` where it applies.
fn welcome_text(text: impl Into<SharedString>) -> Div {
    let text: SharedString = text.into();
    div()
        .mb(px(10.))
        .text_size(px(WELCOME_FONT_MD))
        .line_height(px(WELCOME_FONT_MD * 1.5))
        .child(text)
}

fn footer_link(id: &'static str, label: &'static str, url: &'static str, cx: &App) -> Inline {
    link_button(id, label, cx)
        .text_size(px(WELCOME_FONT_SM))
        .on_click(move |_, _, cx| cx.open_url(url))
        .into_any_element()
        .into()
}

/// `.text-box-component` in the welcome: label, 3.33 px gap, a 34.8 px input.
fn welcome_field(label: &'static str, field: impl IntoElement, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .text_color(cx.ghd().text)
        .child(
            div()
                .mb(px(3.33))
                .text_size(px(WELCOME_FONT_MD))
                .line_height(px(WELCOME_FONT_MD * 1.5))
                .child(label),
        )
        .child(field)
}

/// `#welcome input`: `--text-field-height`, 5 px padding, 16.8 px text.
fn welcome_text_box(
    id: &'static str,
    state: &Entity<InputState>,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let focused = state.read(cx).focus_handle(cx).is_focused(window);
    div()
        .id(id)
        .h(px(29. * SCALE))
        .w_full()
        .flex()
        .items_center()
        // the kit's xsmall input pads 4 px itself
        .px(px(1.))
        .border_1()
        .rounded(BORDER_RADIUS())
        .bg(t.box_background)
        .border_color(if focused {
            t.focus
        } else {
            t.box_border_contrast
        })
        .when(focused, |d| {
            d.shadow(vec![BoxShadow {
                color: t.text_field_focus_shadow,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: false,
            }])
        })
        .child(
            div().flex_1().min_w_0().child(
                Input::new(state)
                    .appearance(false)
                    .xsmall()
                    .text_size(px(WELCOME_FONT_MD)),
            ),
        )
}

/// `.text-box-component input[readonly]`: the account's name, not editable.
fn welcome_read_only_box(id: &'static str, state: &Entity<InputState>, cx: &App) -> Stateful<Div> {
    let t = cx.ghd();
    div()
        .id(id)
        .h(px(29. * SCALE))
        .w_full()
        .flex()
        .items_center()
        .px(px(1.))
        .border_1()
        .rounded(BORDER_RADIUS())
        .bg(t.box_alt_background)
        .border_color(t.box_border_contrast)
        .text_color(t.text_secondary)
        .child(
            div().flex_1().min_w_0().child(
                Input::new(state)
                    .appearance(false)
                    .xsmall()
                    .readonly(true)
                    .text_color(t.text_secondary)
                    .text_size(px(WELCOME_FONT_MD)),
            ),
        )
}

/// `RadioButton` in the welcome: Chromium's radio at `--radio-control-radius`
/// (13 px × scale): a 1 px ring, grey when off; on, an accent ring around a
/// 2 px white gap and an accent dot. The label follows 5 px later.
fn welcome_radio_row(id: &'static str, selected: bool, cx: &App) -> Stateful<Div> {
    let t = cx.ghd();
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .h(px(WELCOME_FONT_MD * 1.5))
        .cursor_pointer()
        .child(
            div()
                .size(px(15.))
                .mr(px(5.6))
                .flex_none()
                .rounded_full()
                .border_1()
                .border_color(if selected {
                    t.accent
                } else {
                    rgb(0x767676).into()
                })
                .bg(t.box_background)
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |d| {
                    d.child(div().size(px(9.)).rounded_full().bg(t.accent))
                }),
        )
}

/// `GitEmailNotFoundWarning` in the welcome: 10 px under the email box,
/// ⚠️ and "Learn more." when commits with `email` would not be linked to the
/// account, a green check when they would; nothing for an empty email.
fn email_not_found_warning(account: &corvane_core::Account, email: &str, cx: &App) -> Option<Div> {
    let email = email.trim();
    if email.is_empty() {
        return None;
    }
    let t = cx.ghd();
    let kind = if account.is_dotcom() {
        "GitHub"
    } else {
        "GitHub Enterprise"
    };
    let attributable = account.is_attributable_email(email);
    let mut parts: Vec<Inline> = Vec::new();
    if attributable {
        parts.push(
            div()
                .size(px(12.))
                .mr(px(5.))
                .rounded_full()
                .bg(t.color_new)
                .flex()
                .items_center()
                .justify_center()
                .child(octicon(Octicon::Check, t.background).size(px(10.)))
                .into_any_element()
                .into(),
        );
        parts.push(format!("This email address matches your {kind} account.").into());
    } else {
        parts.push(
            format!(
                "⚠️This email address does not match your {kind} account. Your commits \
                 will be wrongly attributed."
            )
            .into(),
        );
        parts.push(
            link_button("welcome-email-learn-more", "Learn more.", cx)
                .text_size(px(WELCOME_FONT_MD))
                .on_click(|_, _, cx| {
                    cx.open_url(
                        "https://docs.github.com/en/github/committing-changes-to-your-project/\
                         why-are-my-commits-linked-to-the-wrong-user",
                    )
                })
                .into_any_element()
                .into(),
        );
    }
    Some(
        paragraph(parts)
            .mt(px(10.))
            .gap_x(px(4.5))
            .line_height(px(WELCOME_FONT_MD * 1.5)),
    )
}

/// `#welcome button`: `--button-height`, 5 px vertical padding, 16.8 px text;
/// callers add the horizontal padding and margins. `focused` draws the
/// primary button's `:focus` colours.
fn welcome_button(id: &'static str, primary: bool, focused: bool, cx: &App) -> Stateful<Div> {
    let t = cx.ghd();
    let (bg, border, text, hover) = if primary {
        (
            if focused {
                t.button_hover_background
            } else {
                t.button_background
            },
            t.button_background,
            t.button_text,
            t.button_hover_background,
        )
    } else {
        (
            t.secondary_button_background,
            t.secondary_button_border,
            t.secondary_button_text,
            t.secondary_button_hover_background,
        )
    };
    div()
        .id(id)
        .flex_none()
        .h(px(29. * SCALE))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .border_1()
        .rounded(BORDER_RADIUS())
        .bg(bg)
        .border_color(border)
        .text_color(text)
        .text_size(px(WELCOME_FONT_MD))
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
}

impl Render for WelcomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.autofocus && self.step == Step::Start {
            self.autofocus = false;
            window.focus(&self.sign_in_focus, cx);
        }
        let t = cx.ghd();
        let content: AnyElement = match self.step {
            Step::Start => self.start(window, cx).into_any_element(),
            Step::ConfigureGit => self.configure_git(window, cx).into_any_element(),
        };
        let viewport = window.viewport_size();
        let left_w = viewport.width * 0.6;
        let right_w = viewport.width - left_w;
        // `.welcome-right .welcome-graphic { height: 100%; object-fit: cover;
        // object-position: left }` in a flex box centred inside 40 px
        // top / left padding: wider than the box, it overflows both sides
        let graphic_h = viewport.height - px(40.);
        let graphic_w = graphic_h * (296.45 / 378.377);
        let graphic_x = px(40.) + (right_w - px(40.) - graphic_w) / 2.;
        let top_w = left_w * 0.2;
        let bottom_w = left_w * 0.4;
        div()
            .id("welcome")
            .size_full()
            .flex()
            .flex_row()
            .bg(t.background)
            .text_color(t.text)
            .text_size(px(WELCOME_FONT_MD))
            .line_height(px(WELCOME_FONT_MD * 1.5))
            .child(
                // `.welcome-left`: graphics behind the content (`z-index: -1`)
                div()
                    .id("welcome-left")
                    .relative()
                    .w(left_w)
                    .h_full()
                    .flex()
                    .items_center()
                    .p(px(40.))
                    .overflow_y_scroll()
                    .child(
                        img("illustrations/welcome-illustration-left-top.svg")
                            .absolute()
                            .right(px(80.))
                            .top(px(40.))
                            .w(top_w)
                            .h(top_w * (43.835956 / 42.2971)),
                    )
                    .child(
                        img("illustrations/welcome-illustration-left-bottom.svg")
                            .absolute()
                            .right(px(10.))
                            .bottom(px(10.))
                            .w(bottom_w)
                            .h(bottom_w * (172.30263 / 113.99702)),
                    )
                    .child(
                        div()
                            .w_full()
                            .max_w(px(500. * SCALE))
                            .flex()
                            .flex_col()
                            .child(content),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w(right_w)
                    .h_full()
                    .bg(rgb(0x28373b))
                    .overflow_hidden()
                    .child(
                        img("illustrations/welcome-illustration-right.svg")
                            .absolute()
                            .left(graphic_x)
                            .top(px(40.))
                            .w(graphic_w)
                            .h(graphic_h),
                    ),
            )
    }
}
