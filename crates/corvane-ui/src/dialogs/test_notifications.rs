//! Test Notifications - GHD `ui/test-notifications/test-notifications.tsx`
//! (`styles/ui/dialogs/_test-notifications.scss`), opened from Help › Show
//! Test Notifications in debug builds (GHD's test menu "Show notification").
//!
//! The permission hint (`renderNotificationHint`), "Select the type of
//! notification to display:" and one button per `TestNotificationType`, a
//! lone "Close" button.
//!
//! Deviations: a button posts its notification at once with sample data
//! (`corvane_core::samples::notification`); GHD walks through picking one of
//! the repository's pull requests and one of its reviews / comments from the
//! API, which needs a signed-in account (so there is no "Back" button).

use corvane_core::Dispatcher;
use corvane_core::notifications::TestNotificationType;
use corvane_platform::notifications::NotificationPermission;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{Inline, button, link_button, paragraph};

pub struct TestNotificationsDialog {
    repo: u64,
    /// `getNotificationsPermission()`; `None` until the centre answered.
    permission: Option<NotificationPermission>,
}

impl TestNotificationsDialog {
    pub fn new(repo: u64, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            repo,
            permission: None,
        };
        this.update_notifications_state(cx);
        this
    }

    /// `updateNotificationsState` (asked off the main thread: the centre
    /// answers on its own queue).
    fn update_notifications_state(&mut self, cx: &mut Context<Self>) {
        let task = cx
            .background_executor()
            .spawn(async move { corvane_platform::notifications::permission() });
        cx.spawn(async move |this, cx| {
            let permission = task.await;
            this.update(cx, |this, cx| {
                this.permission = Some(permission);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `onGrantNotificationPermission`: the prompt is asynchronous, so poll
    /// until the status leaves `Default`.
    fn grant_permission(&mut self, cx: &mut Context<Self>) {
        corvane_platform::notifications::request_permission();
        cx.spawn(async move |this, cx| {
            for _ in 0..60 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let permission = cx
                    .background_executor()
                    .spawn(async move { corvane_platform::notifications::permission() })
                    .await;
                let done = this
                    .update(cx, |this, cx| {
                        this.permission = Some(permission);
                        cx.notify();
                        permission != NotificationPermission::Default
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    /// `renderNotificationHint`
    fn hint(&self, cx: &Context<Self>) -> Option<Div> {
        let t = cx.ghd();
        let settings_url =
            corvane_platform::notifications::settings_url(corvane_platform::BUNDLE_ID);
        let settings_link = move |cx: &Context<Self>| {
            let url = settings_url.clone();
            Inline::Element(
                link_button("test-notifications-settings", "Notifications Settings", cx)
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .into_any_element(),
            )
        };
        let parts: Vec<Inline> = match self.permission? {
            NotificationPermission::Unsupported => return None,
            NotificationPermission::Default => vec![
                "You need to ".into(),
                Inline::Element(
                    link_button("test-notifications-grant", "grant permission", cx)
                        .on_click(cx.listener(|this, _, _, cx| this.grant_permission(cx)))
                        .into_any_element(),
                ),
                " to display these notifications from Corvane.".into(),
            ],
            NotificationPermission::Denied => vec![
                Inline::Element(
                    div()
                        .text_color(t.dialog_warning)
                        .child("⚠️")
                        .into_any_element(),
                ),
                " Corvane has no permission to display notifications. Please, enable them in the "
                    .into(),
                settings_link(cx),
                ".".into(),
            ],
            NotificationPermission::Granted => vec![
                "Make sure notifications are properly configured for Corvane in the ".into(),
                settings_link(cx),
                ".".into(),
            ],
        };
        Some(paragraph(parts))
    }
}

impl Render for TestNotificationsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let repo = self.repo;
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        // `.notification-type-list`: a column with half-spacing gaps
        let buttons: Vec<_> = TestNotificationType::ALL
            .into_iter()
            .enumerate()
            .map(|(ix, kind)| {
                button(
                    ElementId::NamedInteger("test-notification-type".into(), ix as u64),
                    kind.friendly_name(),
                    cx,
                )
                .role(Role::Button)
                .aria_label(kind.friendly_name())
                .on_click(move |_, _, cx| {
                    let notification = corvane_core::samples::notification(kind, repo, cx);
                    Dispatcher::simulate_pull_request_event(notification, cx);
                })
            })
            .collect();
        let types = div().flex().flex_col().gap(SPACING_HALF).children(buttons);
        let content = div()
            .w(px(460.))
            .flex()
            .flex_col()
            .gap(SPACING)
            .children(self.hint(cx))
            .child("Select the type of notification to display:")
            .child(types);
        dialog(
            "test-notifications",
            "Test Notifications",
            content,
            vec![DialogButton {
                id: "test-notifications-close",
                label: "Close".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(|_, cx| Dispatcher::close_popup(cx)),
            }],
            close,
            window,
            cx,
        )
    }
}
