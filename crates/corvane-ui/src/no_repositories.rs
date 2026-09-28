//! `#no-repositories` - first-launch blankslate ("Let's get started!").
//! `styles/ui/_no-repositories.scss`, `ui/no-repositories/no-repositories-view.tsx`.

use corvane_core::{Dispatcher, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

fn big_button(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let hover_bg = t.secondary_button_hover_background;
    let hover_border = t.secondary_button_hover_border;
    div()
        .id(id)
        .w_full()
        .mb(SPACING)
        .p(SPACING)
        .flex()
        .flex_row()
        .items_center()
        .border_1()
        .rounded(BORDER_RADIUS)
        .bg(t.secondary_button_background)
        .border_color(t.secondary_button_border)
        .text_color(t.secondary_button_text)
        .text_size(FONT_SIZE)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(
            octicon(icon, t.secondary_button_text)
                .size(px(24.))
                .my(SPACING_HALF)
                .ml(SPACING_HALF)
                .mr(SPACING),
        )
        .child(div().flex_1().min_w_0().child(label))
}

pub fn no_repositories(cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("no-repositories")
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .p(px(60.))
        .bg(t.background)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    // header
                    div()
                        .flex()
                        .flex_col()
                        .mb(px(40.))
                        .child(
                            div()
                                .text_size(px(42.))
                                .line_height(px(50.))
                                .font_weight(FontWeight::LIGHT)
                                .child("Let's get started!"),
                        )
                        .child(
                            div()
                                .text_size(FONT_SIZE)
                                .child("Add a repository to Corvane to start collaborating"),
                        ),
                )
                .child(
                    // `.content`: two 50 % panes
                    div()
                        .flex()
                        .flex_row()
                        .w_full()
                        .child(
                            div()
                                .w_1_2()
                                .pr(SPACING)
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .child(
                                    img("illustrations/empty-no-repo.svg")
                                        .w_full()
                                        .max_w(px(400.))
                                        .h(px(240.))
                                        .object_fit(ObjectFit::Contain),
                                ),
                        )
                        .child(
                            div()
                                .w_1_2()
                                .pl(SPACING)
                                .flex()
                                .flex_col()
                                .child(big_button(
                                    "nr-clone",
                                    Octicon::RepoClone,
                                    "Clone a Repository from the Internet…",
                                    |_, cx| {
                                        Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx)
                                    },
                                    cx,
                                ))
                                .child(big_button(
                                    "nr-create",
                                    Octicon::Plus,
                                    "Create a New Repository on your Local Drive…",
                                    |_, cx| {
                                        Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx)
                                    },
                                    cx,
                                ))
                                .child(big_button(
                                    "nr-add",
                                    Octicon::FileDirectory,
                                    "Add an Existing Repository from your Local Drive…",
                                    |_, cx| Dispatcher::show_popup(Popup::AddExistingRepository { path: None }, cx),
                                    cx,
                                ))
                                .child(
                                    // `.drag-drop-info`
                                    div()
                                        .mt(SPACING)
                                        .p(SPACING_DOUBLE)
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .rounded(BORDER_RADIUS)
                                        .border_2()
                                        .border_dashed()
                                        .border_color(t.box_border_contrast)
                                        .bg(t.box_alt_background)
                                        .child(octicon(Octicon::FileDirectory, t.text_secondary).mr(SPACING))
                                        .child(
                                            div()
                                                .text_size(FONT_SIZE)
                                                .child("ProTip! You can drag & drop an existing repository folder here to add it to Corvane"),
                                        ),
                                ),
                        ),
                ),
        )
}
