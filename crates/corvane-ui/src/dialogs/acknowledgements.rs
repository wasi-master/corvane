//! GHD `ui/acknowledgements/acknowledgements.tsx` (`styles/ui/_acknowledgements.scss`):
//! "License and Open Source Notices", Corvane's own license, then every
//! distributed library with a link to its repository and its license text in
//! monospace. The notices come from the embedded `acknowledgements.json`
//! (`corvane_core::acknowledgements`); hundreds of entries render through a
//! virtualized `list`.
//!
//! Deviation: the license text is not selectable (GPUI's static text has no
//! selection; GHD sets `user-select: text`).

use corvane_core::Dispatcher;
use corvane_core::acknowledgements::{Acknowledgements, parse};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::scrollbar::{gutter, scrollbar};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, MONO_FONT};
use crate::widgets::{Inline, link_button, paragraph};

const WEBSITE_URL: &str = "https://github.com/wasi-master/corvane";
const REPOSITORY_URL: &str = "https://github.com/wasi-master/corvane";

/// `#acknowledgements`: 600 px wide, 300 px of scrolling content.
const CONTENT_WIDTH: Pixels = px(560.);
const CONTENT_HEIGHT: Pixels = px(300.);
/// Rows before the libraries: intro, Corvane's license, "also distributes".
const HEAD_ROWS: usize = 3;

pub struct AcknowledgementsDialog {
    notices: Option<Acknowledgements>,
    list: ListState,
}

impl AcknowledgementsDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let notices = cx
            .asset_source()
            .load("acknowledgements.json")
            .ok()
            .flatten()
            .and_then(|bytes| parse(&bytes));
        let rows = HEAD_ROWS + notices.as_ref().map_or(0, |n| n.libraries.len());
        Self {
            notices,
            list: ListState::new(rows, ListAlignment::Top, px(300.)),
        }
    }
}

/// `.license-text`
fn license_text(text: impl Into<SharedString>, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .mb(SPACING_DOUBLE)
        .font_family(MONO_FONT)
        .text_size(FONT_SIZE_SM)
        .text_color(t.text)
        .child(text.into())
}

impl Render for AcknowledgementsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let notices = self.notices.clone();
        let body: AnyElement = match notices {
            None => div()
                .w(CONTENT_WIDTH)
                .child("The license notices could not be loaded.")
                .into_any_element(),
            Some(notices) => {
                let notices = std::rc::Rc::new(notices);
                div()
                    .w(CONTENT_WIDTH)
                    .h(CONTENT_HEIGHT)
                    .relative()
                    .child(
                        list(self.list.clone(), move |ix, _, cx| {
                            let t = cx.ghd();
                            match ix {
                                0 => div()
                                    .pb(SPACING)
                                    .child(paragraph(vec![
                                        Inline::Element(
                                            link_button("ack-website", "Corvane", cx)
                                                .on_click(|_, _, cx| cx.open_url(WEBSITE_URL))
                                                .into_any_element(),
                                        ),
                                        " is an open source project published under the MIT \
                                         License. You can view the source code and contribute \
                                         to this project on "
                                            .into(),
                                        Inline::Element(
                                            link_button("ack-repository", "GitHub", cx)
                                                .on_click(|_, _, cx| cx.open_url(REPOSITORY_URL))
                                                .into_any_element(),
                                        ),
                                        ".".into(),
                                    ]))
                                    .into_any_element(),
                                1 => {
                                    license_text(notices.app_license.clone(), cx).into_any_element()
                                }
                                2 => div()
                                    .pb(SPACING)
                                    .child("Corvane also distributes these libraries:")
                                    .into_any_element(),
                                _ => {
                                    let Some(lib) = notices.libraries.get(ix - HEAD_ROWS) else {
                                        return div().into_any_element();
                                    };
                                    let title = format!("{}@{}", lib.name, lib.version);
                                    // `<h2>`: the name, linked to the repository
                                    let header: AnyElement = match lib.repository.clone() {
                                        Some(url) => link_button(
                                            SharedString::from(format!("ack-{ix}")),
                                            title,
                                            cx,
                                        )
                                        .on_click(move |_, _, cx| cx.open_url(&url))
                                        .into_any_element(),
                                        None => div().child(title).into_any_element(),
                                    };
                                    let text: SharedString = if !lib.texts.is_empty() {
                                        lib.texts
                                            .iter()
                                            .map(|t| t.as_ref())
                                            .collect::<Vec<_>>()
                                            .join("\n\n")
                                            .into()
                                    } else if let Some(license) = &lib.license {
                                        format!("License: {license}").into()
                                    } else {
                                        "Unknown license".into()
                                    };
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .mb(SPACING)
                                                .text_size(FONT_SIZE_MD)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(t.text)
                                                .child(header),
                                        )
                                        .child(license_text(text, cx))
                                        .into_any_element()
                                }
                            }
                        })
                        .size_full()
                        .pr(gutter(&self.list)),
                    )
                    .child(scrollbar("acknowledgements-scrollbar", self.list.clone()))
                    .into_any_element()
            }
        };
        dialog(
            "acknowledgements",
            "License and Open Source Notices",
            body,
            vec![DialogButton {
                id: "acknowledgements-close",
                label: "Close".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            }],
            close,
            window,
            cx,
        )
    }
}
