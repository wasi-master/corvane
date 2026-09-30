//! First-launch Welcome flow (`ui/welcome/*`): Start → Configure Git.
//! `styles/ui/_welcome.scss`: left pane 60 % (40 px padding, content ≤ 500 px
//! × scale, the two graphics pinned behind it), right pane 40 % on `#28373b`
//! with the illustration cropped from the left; scale 1.2 at ≥ 1366×700.
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
use crate::widgets::{Inline, avatar_image, avatar_lookup, link_button, paragraph};
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
    name: Entity<InputState>,
    email: Entity<InputState>,
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
        let had_account = !state.read(cx).accounts.is_empty();
        let sign_in_focus = cx.focus_handle();
        // A successful sign-in advances to Configure Git (GHD `Welcome.componentWillReceiveProps`).
        cx.observe_in(&state, window, |this, state, window, cx| {
            let has_account = !state.read(cx).accounts.is_empty();
            if has_account && !this.had_account && this.step == Step::Start {
                this.advance(window, cx);
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
                (s.git.clone(), s.dotcom_account().cloned())
            };
            let identity = git.map(corvane_git::global_identity).unwrap_or_default();
            let name = identity
                .name
                .clone()
                .or_else(|| account.as_ref().and_then(|a| a.name.clone()))
                .or_else(|| account.as_ref().map(|a| a.login.clone()))
                .unwrap_or_default();
            let email = identity
                .email
                .clone()
                .or_else(|| account.as_ref().and_then(|a| a.emails.first().cloned()))
                .unwrap_or_default();
            self.name.update(cx, |s, cx| s.set_value(name, window, cx));
            self.email
                .update(cx, |s, cx| s.set_value(email, window, cx));
        }
        let handle = self.name.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().to_string();
        let email = self.email.read(cx).value().to_string();
        Dispatcher::set_global_identity(name, email, cx);
        Dispatcher::complete_welcome(cx);
    }

    fn start(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
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
                    .child(welcome_title("Welcome to Corvane"))
                    .child(
                        welcome_text(
                            "Corvane is a seamless way to contribute to projects on GitHub and \
                         GitHub Enterprise. Sign in below to get started with your existing \
                         projects.",
                        )
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
                                                Dispatcher::sign_in_device_flow(
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
                    .child(div().child(
                        "Corvane does not send usage metrics. Nothing about how you use the \
                             app is collected or shared with anyone.",
                    )),
            )
    }

    fn configure_git(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let finish = cx.entity();
        let cancel = cx.entity();
        let name = self.name.read(cx).value().to_string();
        let email = self.email.read(cx).value().to_string();
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
                // `#configure-git-user`: the form (20 px margins), then the example commit
                div()
                    .pb(px(10.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .mt(px(10.))
                            .mb(px(20.))
                            .flex()
                            .flex_col()
                            .child(welcome_field(
                                "Name",
                                welcome_text_box("welcome-name", &self.name, window, cx),
                                cx,
                            ))
                            .child(
                                welcome_field(
                                    "Email",
                                    welcome_text_box("welcome-email", &self.email, window, cx),
                                    cx,
                                )
                                .mt(px(10.)),
                            )
                            .child(
                                // `Row`: Finish (submit) and Cancel, 33.36 px under
                                // the email field as GHD lays it out
                                div()
                                    .mt(px(33.36))
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
fn welcome_title(text: &'static str) -> Div {
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
fn welcome_text(text: &'static str) -> Div {
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
