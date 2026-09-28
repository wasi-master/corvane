//! First-launch Welcome flow (`ui/welcome/*`): Start → Configure Git.
//! `styles/ui/_welcome.scss`: left pane 60 % with content ≤ 500 px × scale,
//! right pane 40 % illustration; scale 1.2 at ≥ 1366×700.

use corvane_core::{AppState, Dispatcher, Popup};
use corvane_github::Endpoint;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{labeled, text_box};

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
}

impl WelcomeView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Your Name"));
        let email = cx.new(|cx| InputState::new(window, cx).placeholder("your-email@example.com"));
        let had_account = !state.read(cx).accounts.is_empty();
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
        }
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

    fn start(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let this = cx.entity();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(42. * SCALE))
                    .line_height(px(42. * SCALE * 1.25))
                    .font_weight(FontWeight::LIGHT)
                    .mb(SPACING)
                    .child("Welcome to Corvane"),
            )
            .child(
                div()
                    .my(SPACING)
                    .text_size(px(14. * SCALE))
                    .line_height(px(14. * SCALE * 1.5))
                    .child(
                        "Corvane is a seamless way to contribute to projects on GitHub and \
                         GitHub Enterprise. Sign in below to get started with your existing projects.",
                    ),
            )
            .child(
                // `.welcome-main-buttons`
                div()
                    .mt(px(40.))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .child(
                        welcome_button("welcome-sign-in", true, cx)
                            .child("Sign in to GitHub.com")
                            .child(octicon(Octicon::LinkExternal, t.button_text).ml(SPACING_HALF))
                            .on_click(|_, _, cx| {
                                Dispatcher::show_popup(Popup::SignIn { enterprise: false }, cx);
                                Dispatcher::sign_in_device_flow(Endpoint::github_com(), cx);
                            }),
                    )
                    .child(
                        welcome_button("welcome-sign-in-enterprise", false, cx)
                            .child("Sign in to GitHub Enterprise")
                            .on_click(|_, _, cx| {
                                Dispatcher::show_popup(Popup::SignIn { enterprise: true }, cx)
                            }),
                    ),
            )
            .child(
                // `.skip-action-container`
                div()
                    .mt(SPACING_DOUBLE)
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .text_size(px(14. * SCALE))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(4.))
                            .child("New to GitHub?")
                            .child(
                                div()
                                    .id("welcome-create-account")
                                    .text_color(t.link)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .cursor_pointer()
                                    .child("Create your free account.")
                                    .on_click(|_, _, cx| cx.open_url("https://github.com/join?source=corvane")),
                            ),
                    )
                    .child(
                        div()
                            .id("welcome-skip")
                            .text_color(t.link)
                            .cursor_pointer()
                            .child("Skip this step")
                            .on_click(move |_, window, cx| {
                                this.update(cx, |w, cx| w.advance(window, cx))
                            }),
                    ),
            )
    }

    fn configure_git(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let this = cx.entity();
        let name = self.name.read(cx).value().to_string();
        let email = self.email.read(cx).value().to_string();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(42. * SCALE))
                    .line_height(px(42. * SCALE * 1.25))
                    .font_weight(FontWeight::LIGHT)
                    .mb(SPACING)
                    .child("Configure Git"),
            )
            .child(
                div()
                    .my(SPACING)
                    .text_size(px(14. * SCALE))
                    .line_height(px(14. * SCALE * 1.5))
                    .child(
                        "This is used to identify the commits you create. Anyone will be able to \
                         see this information if you publish commits.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .my(SPACING)
                    .text_size(px(14. * SCALE))
                    .child(labeled(
                        "Name",
                        text_box("welcome-name", &self.name, None, window, cx),
                        cx,
                    ))
                    .child(labeled(
                        "Email",
                        text_box("welcome-email", &self.email, None, window, cx),
                        cx,
                    )),
            )
            .child(
                // "Example commit" preview (`#commit-list.commit-list-example`)
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF)
                    .my(SPACING)
                    .child(
                        div()
                            .text_size(px(11. * SCALE))
                            .text_color(t.text_secondary)
                            .child("Example commit"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING)
                            .p(SPACING)
                            .border_1()
                            .border_color(t.box_border)
                            .rounded(BORDER_RADIUS)
                            .bg(t.box_background)
                            .child(crate::widgets::avatar_placeholder(px(16. * SCALE), cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(12. * SCALE))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Fix all the things"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11. * SCALE))
                                            .text_color(t.text_secondary)
                                            .child(format!(
                                                "{} <{}> · just now",
                                                if name.is_empty() { "Your Name" } else { &name },
                                                if email.is_empty() {
                                                    "your-email@example.com"
                                                } else {
                                                    &email
                                                }
                                            )),
                                    ),
                            ),
                    ),
            )
            .child(
                div().mt(SPACING).flex().flex_row().gap(SPACING).child(
                    welcome_button("welcome-finish", true, cx)
                        .child("Finish")
                        .on_click(move |_, _, cx| this.update(cx, |w, cx| w.finish(cx))),
                ),
            )
    }
}

fn welcome_button(id: &'static str, primary: bool, cx: &App) -> Stateful<Div> {
    let t = cx.ghd();
    let (bg, border, text, hover) = if primary {
        (
            t.button_background,
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
        .h(px(30. * SCALE))
        .px(SPACING_DOUBLE)
        .mr(SPACING_DOUBLE)
        .mb(SPACING)
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .border_1()
        .rounded(BORDER_RADIUS)
        .bg(bg)
        .border_color(border)
        .text_color(text)
        .text_size(px(14. * SCALE))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
}

impl Render for WelcomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let content: AnyElement = match self.step {
            Step::Start => self.start(cx).into_any_element(),
            Step::ConfigureGit => self.configure_git(window, cx).into_any_element(),
        };
        div()
            .id("welcome")
            .size_full()
            .flex()
            .flex_row()
            .bg(t.background)
            .text_color(t.text)
            .child(
                // `.welcome-left`: 60 %, padding 40, illustrations pinned top-right / bottom-right
                div()
                    .relative()
                    .w_3_5()
                    .h_full()
                    .flex()
                    .items_center()
                    .p(px(40.))
                    .overflow_hidden()
                    .child(
                        img("illustrations/welcome-illustration-left-top.svg")
                            .absolute()
                            .right(px(80.))
                            .top(px(40.))
                            .w(px(120.))
                            .h(px(90.))
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(
                        img("illustrations/welcome-illustration-left-bottom.svg")
                            .absolute()
                            .right(SPACING)
                            .bottom(SPACING)
                            .w_2_5()
                            .h(px(180.))
                            .object_fit(ObjectFit::Contain),
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
                // `.welcome-right`: 40 %, right illustration
                div()
                    .w_2_5()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .child(
                        img("illustrations/welcome-illustration-right.svg")
                            .w_full()
                            .h_full()
                            .object_fit(ObjectFit::Contain),
                    ),
            )
    }
}
