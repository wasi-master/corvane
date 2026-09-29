//! GHD `ui/release-notes/release-notes-dialog.tsx`
//! (`styles/ui/dialogs/_release-notes.scss`): a 100 px header with the two
//! release-note illustrations and "Version X / date", the entries in one or
//! two columns, "View all release notes" on the left of the footer.
//!
//! Deviations: entries are plain text (GHD's `RichText` renders emoji, links
//! and `#123` references) and the pretext is a plain paragraph (GHD renders it
//! as sandboxed Markdown). There is no "Install and Restart" button: the notes
//! are always the running version's (the self-updater is not built yet).

use corvane_core::Dispatcher;
use corvane_core::release_notes::{RELEASE_NOTES_URL, ReleaseNote, ReleaseSummary};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{link_button, primary_button};

/// `--spacing-triple`
const SPACING_TRIPLE: Pixels = px(30.);
/// `.dialog-header` height and the illustrations' height in it.
const HEADER_HEIGHT: Pixels = px(100.);
const ART_HEIGHT: Pixels = px(90.);

pub struct ReleaseNotesDialog {
    summary: ReleaseSummary,
}

impl ReleaseNotesDialog {
    pub fn new(summary: ReleaseSummary) -> Self {
        Self { summary }
    }
}

/// `renderList`: a bold header and one line per entry.
fn section(header: &'static str, entries: &[ReleaseNote], cx: &App) -> Option<AnyElement> {
    let t = cx.ghd();
    if entries.is_empty() {
        return None;
    }
    Some(
        div()
            .my(SPACING_DOUBLE)
            .flex()
            .flex_col()
            .gap(SPACING_HALF)
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(header),
            )
            .children(
                entries
                    .iter()
                    .map(|e| div().text_color(t.text).child(e.message.clone())),
            )
            .into_any_element(),
    )
}

/// `.column`
fn column(children: Vec<Option<AnyElement>>) -> Div {
    div()
        .flex_1()
        .min_w(px(275.))
        .mr(px(15.))
        .flex()
        .flex_col()
        .children(children.into_iter().flatten())
}

impl Render for ReleaseNotesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let r = &self.summary;
        let viewport = window.viewport_size();
        let date = r
            .date_published
            .map(|d| crate::format::format_pattern("MMMM d, yyyy", &crate::format::local_time(d)));
        // `drawTwoColumnLayout` when there are both enhancements and bugfixes
        let columns: Vec<Div> = if !r.enhancements.is_empty() && !r.bugfixes.is_empty() {
            vec![
                column(vec![
                    section("Enhancements", &r.enhancements, cx),
                    section("Other", &r.other, cx),
                ]),
                column(vec![section("Bugfixes", &r.bugfixes, cx)]),
            ]
        } else {
            vec![column(vec![
                section("Bugfixes", &r.bugfixes, cx),
                section("Enhancements", &r.enhancements, cx),
                section("Other", &r.other, cx),
            ])]
        };
        let pretext = r.pretext.first().map(|p| p.message.clone());
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("release-notes-overlay")
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(t.overlay)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| close(window, cx))
                    .child(
                        div()
                            .id("release-notes")
                            .min_w(px(550.))
                            .max_w(px(800.))
                            .max_h(px(500.))
                            .flex()
                            .flex_col()
                            .rounded(BORDER_RADIUS)
                            .bg(t.background)
                            .text_color(t.text)
                            .text_size(FONT_SIZE)
                            .border_1()
                            .border_color(t.box_border)
                            .shadow(vec![BoxShadow {
                                color: t.shadow,
                                offset: point(px(0.), px(2.)),
                                blur_radius: px(7.),
                                spread_radius: px(0.),
                                inset: false,
                            }])
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                // `.dialog-header`
                                div()
                                    .relative()
                                    .h(HEADER_HEIGHT)
                                    .flex_none()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .border_b_1()
                                    .border_color(t.box_border)
                                    .child(
                                        img("illustrations/release-note-header-left.svg")
                                            .absolute()
                                            .left(SPACING_DOUBLE)
                                            .top(SPACING_HALF)
                                            .h(ART_HEIGHT)
                                            .w(ART_HEIGHT * (147. / 71.)),
                                    )
                                    .child(
                                        img("illustrations/release-note-header-right.svg")
                                            .absolute()
                                            .right(SPACING_DOUBLE)
                                            .top(SPACING_HALF)
                                            .h(ART_HEIGHT)
                                            .w(ART_HEIGHT * (144. / 76.)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(format!("Version {}", r.latest_version)),
                                            )
                                            .children(date.map(|d| {
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(t.text_secondary)
                                                    .child(d)
                                            })),
                                    )
                                    .child(
                                        // `a.close`, top-aligned
                                        div()
                                            .id("release-notes-close")
                                            .absolute()
                                            .top(SPACING)
                                            .right(SPACING)
                                            .size(px(16.))
                                            .cursor_pointer()
                                            .tooltip(crate::widgets::tooltip("Close"))
                                            .on_click(move |_, window, cx| close(window, cx))
                                            .child(octicon(Octicon::X, t.text_secondary)),
                                    ),
                            )
                            .child(
                                // `.dialog-content`
                                div()
                                    .id("release-notes-content")
                                    .max_h(px(335.))
                                    .overflow_y_scroll()
                                    .py(SPACING)
                                    .px(SPACING_TRIPLE)
                                    .line_height(px(18.))
                                    .children(pretext.map(|p| div().my(SPACING).child(p)))
                                    .child(
                                        div().flex().flex_row().justify_around().children(columns),
                                    )
                                    .with_scrollbar(),
                            )
                            .child(
                                // `.dialog-footer`: space-between
                                div()
                                    .flex_none()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .gap(SPACING)
                                    .p(SPACING_DOUBLE)
                                    .border_t_1()
                                    .border_color(t.box_border)
                                    .child(
                                        link_button(
                                            "release-notes-all",
                                            "View all release notes",
                                            cx,
                                        )
                                        .on_click(|_, _, cx| cx.open_url(RELEASE_NOTES_URL)),
                                    )
                                    .child(
                                        primary_button("release-notes-ok", "Close", false, cx)
                                            .min_w(px(120.))
                                            .on_click(move |_, window, cx| close(window, cx)),
                                    ),
                            ),
                    ),
            ),
        )
        .with_priority(20)
    }
}
