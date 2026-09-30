//! Modal dialog chrome (`styles/ui/_dialog.scss`): overlay, 400–600 px box,
//! 50 px header with close button, 20 px padded content, footer buttons.
//!
//! Accessibility: the box is a `Dialog` node labelled with its title, and the
//! open dialog's title becomes the window title (VoiceOver reads it as the
//! `AXWindow` title; the title bar itself is hidden) until the dialog closes
//! (`DialogHost` restores "Corvane").

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::IconButtonA11y;

use crate::icons::{Octicon, loading as loading_icon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};

pub type ClickHandler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// The window title when no dialog is open.
pub const APP_WINDOW_TITLE: &str = "Corvane";

thread_local! {
    /// The title last handed to `set_window_title`, so it is set only on change.
    static WINDOW_TITLE: std::cell::RefCell<SharedString> =
        const { std::cell::RefCell::new(SharedString::new_static(APP_WINDOW_TITLE)) };
}

/// Set the window title unless it already is `title`.
pub fn sync_window_title(title: &SharedString, window: &mut Window) {
    let changed = WINDOW_TITLE.with(|current| {
        let mut current = current.borrow_mut();
        if *current == *title {
            false
        } else {
            *current = title.clone();
            true
        }
    });
    if changed {
        window.set_window_title(title);
    }
}

/// A zero-size element that makes `title` the window title while it renders
/// (dialog frames drawn outside `dialog()` add it themselves).
pub fn window_title(title: impl Into<SharedString>) -> impl IntoElement {
    let title: SharedString = title.into();
    canvas(
        move |_, window, _| sync_window_title(&title, window),
        |_, _, _, _| {},
    )
    .absolute()
    .size_0()
}

/// GHD `Dialog type`: warning/error dialogs show a 24 px icon left of the content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Normal,
    Warning,
    Error,
}

/// Per-dialog chrome variations.
pub struct DialogFrame {
    /// `.dialog-header`'s bottom border (`#choose-branch` has none).
    pub header_border: bool,
    /// `.dialog-content`'s 20 px padding (off for list dialogs whose content
    /// runs edge to edge).
    pub content_padding: bool,
    /// A footer drawn by the dialog itself instead of the button row.
    pub footer: Option<AnyElement>,
    /// GHD renders no `.dialog-header` for an untitled `Dialog` (About).
    pub show_header: bool,
    /// The primary button holds focus as the dialog opens (GHD focuses the
    /// submit button when nothing else asks for focus): its focus ring and
    /// `:focus` background show.
    pub focus_primary: bool,
    /// `focusCloseButtonOnOpen`: the header's close button holds focus.
    pub focus_close: bool,
}

impl Default for DialogFrame {
    fn default() -> Self {
        Self {
            header_border: true,
            content_padding: true,
            footer: None,
            show_header: true,
            focus_primary: false,
            focus_close: false,
        }
    }
}

pub struct DialogButton {
    pub id: &'static str,
    pub label: SharedString,
    pub primary: bool,
    /// GHD `okButtonDisabled`: 60 % opacity, clicks ignored.
    pub disabled: bool,
    pub on_click: ClickHandler,
}

/// GHD's per-dialog `width` rules (`dialog#<id> { width }` in the
/// stylesheets), by Corvane dialog id; other dialogs size to their content
/// between 400 and 600 px.
fn ghd_dialog_width(id: &str) -> Option<f32> {
    Some(match id {
        "dialog-conflicts" | "create-fork" | "clone-repository" => 500.,
        "dialog-confirm-abort"
        | "dialog-push-needs-pull"
        | "dialog-stash-and-switch"
        | "create-tutorial-repository-dialog"
        | "dialog-merge-branch"
        | "dialog-rebase-branch"
        | "dialog-cherry-pick"
        | "dialog-about"
        | "push-branch-commits"
        | "dialog-generic-git-auth"
        | "dialog-change-repository-alias"
        | "dialog-confirm-remove-repository" => 450.,
        "create-repository"
        | "dialog-create-branch"
        | "dialog-rename-branch"
        | "dialog-create-tag"
        | "sign-in"
        | "add-existing-repository"
        | "dialog-initialize-lfs" => 400.,
        "dialog-preferences" => 600.,
        _ => return None,
    })
}

pub fn dialog(
    id: &'static str,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    dialog_with_kind(
        id,
        DialogKind::Normal,
        title,
        content,
        buttons,
        on_close,
        window,
        cx,
    )
}

/// `.dialog.warning` / `.dialog.error`: content gets `margin-left: 20px` and
/// `padding-left: 20px + 24px icon`, so the text starts 64 px from the edge.
fn dialog_content(
    kind: DialogKind,
    content: impl IntoElement,
    padding: bool,
    t: &GhdTheme,
) -> Stateful<Div> {
    let base = div()
        .id("dialog-content")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .when(padding, |d| d.p(SPACING_DOUBLE()))
        .text_size(FONT_SIZE())
        .line_height(zpx(18.));
    match kind {
        DialogKind::Normal => base.child(content),
        DialogKind::Warning | DialogKind::Error => {
            let color = if kind == DialogKind::Warning {
                t.dialog_warning
            } else {
                t.dialog_error
            };
            base.flex()
                .flex_row()
                .items_start()
                .gap(SPACING_DOUBLE())
                .child(
                    octicon(Octicon::Alert, color)
                        .size(zpx(24.))
                        .flex_none()
                        .mt(zpx(5.)),
                )
                .child(div().flex_1().min_w_0().child(content))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn dialog_with_kind(
    id: &'static str,
    kind: DialogKind,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    dialog_with_kind_opts(
        id, kind, true, title, content, buttons, on_close, window, cx,
    )
}

/// `dialog_with_kind` with GHD's `backdropDismissable` prop: with it off a
/// click outside the dialog does nothing.
#[allow(clippy::too_many_arguments)]
pub fn dialog_with_kind_opts(
    id: &'static str,
    kind: DialogKind,
    backdrop_dismissable: bool,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let title: SharedString = title.into();
    dialog_impl(
        id,
        kind,
        false,
        backdrop_dismissable,
        div().child(title.clone()).into_any_element(),
        Some(title),
        content,
        None,
        buttons,
        DialogFrame::default(),
        on_close,
        window,
        cx,
    )
}

/// A dialog whose footer shows `footer_message` above the buttons (GHD
/// dialogs that put a `<div>` in `DialogFooter`, e.g. `#create-repo-path-msg`).
#[allow(clippy::too_many_arguments)]
pub fn dialog_with_footer_message(
    id: &'static str,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    footer_message: Option<AnyElement>,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let title: SharedString = title.into();
    dialog_impl(
        id,
        DialogKind::Normal,
        false,
        true,
        div().child(title.clone()).into_any_element(),
        Some(title),
        content,
        footer_message,
        buttons,
        DialogFrame::default(),
        on_close,
        window,
        cx,
    )
}

/// [`dialog`] with chrome variations (`DialogFrame`).
#[allow(clippy::too_many_arguments)]
pub fn dialog_with_frame(
    id: &'static str,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    frame: DialogFrame,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    dialog_loading_framed(
        id, title, false, content, buttons, frame, on_close, window, cx,
    )
}

/// [`dialog_loading`] with chrome variations (`DialogFrame`).
#[allow(clippy::too_many_arguments)]
pub fn dialog_loading_framed(
    id: &'static str,
    title: impl Into<SharedString>,
    loading: bool,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    frame: DialogFrame,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let title: SharedString = title.into();
    dialog_impl(
        id,
        DialogKind::Normal,
        loading,
        true,
        div().child(title.clone()).into_any_element(),
        Some(title),
        content,
        None,
        buttons,
        frame,
        on_close,
        window,
        cx,
    )
}

/// A dialog with an element title and its own footer (`DialogFrame`), e.g.
/// `#choose-branch`.
#[allow(clippy::too_many_arguments)]
pub fn dialog_framed(
    id: &'static str,
    title: impl IntoElement,
    plain_title: impl Into<SharedString>,
    content: impl IntoElement,
    frame: DialogFrame,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    dialog_impl(
        id,
        DialogKind::Normal,
        false,
        true,
        title.into_any_element(),
        Some(plain_title.into()),
        content,
        None,
        Vec::new(),
        frame,
        on_close,
        window,
        cx,
    )
}

/// [`dialog_with_kind`] with chrome variations (`DialogFrame`).
#[allow(clippy::too_many_arguments)]
pub fn dialog_with_kind_framed(
    id: &'static str,
    kind: DialogKind,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    frame: DialogFrame,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let title: SharedString = title.into();
    dialog_impl(
        id,
        kind,
        false,
        true,
        div().child(title.clone()).into_any_element(),
        Some(title),
        content,
        None,
        buttons,
        frame,
        on_close,
        window,
        cx,
    )
}

/// `Dialog loading={…}`: a spinner in the header while the dialog works.
#[allow(clippy::too_many_arguments)]
pub fn dialog_loading(
    id: &'static str,
    title: impl Into<SharedString>,
    loading: bool,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let title: SharedString = title.into();
    dialog_impl(
        id,
        DialogKind::Normal,
        loading,
        true,
        div().child(title.clone()).into_any_element(),
        Some(title),
        content,
        None,
        buttons,
        DialogFrame::default(),
        on_close,
        window,
        cx,
    )
}

/// A dialog whose title is an element (bold branch names inside the title);
/// `plain_title` is the same text for the window title and VoiceOver.
#[allow(clippy::too_many_arguments)]
pub fn dialog_with_title_element(
    id: &'static str,
    title: impl IntoElement,
    plain_title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    dialog_impl(
        id,
        DialogKind::Normal,
        false,
        true,
        title.into_any_element(),
        Some(plain_title.into()),
        content,
        None,
        buttons,
        DialogFrame::default(),
        on_close,
        window,
        cx,
    )
}

#[allow(clippy::too_many_arguments)]
fn dialog_impl(
    id: &'static str,
    kind: DialogKind,
    loading: bool,
    backdrop_dismissable: bool,
    title: AnyElement,
    plain_title: Option<SharedString>,
    content: impl IntoElement,
    footer_message: Option<AnyElement>,
    buttons: Vec<DialogButton>,
    frame: DialogFrame,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let close_for_overlay = on_close.clone();
    let DialogFrame {
        header_border,
        content_padding,
        footer,
        show_header,
        focus_primary,
        focus_close,
    } = frame;
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(zpx(0.), zpx(0.))).child(
            div()
                .id(id)
                // modal: the views underneath get no hover, clicks or wheel
                // (GHD's `<dialog>` makes the rest of the page inert)
                .occlude()
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .items_center()
                .justify_center()
                .bg(t.dialog_backdrop)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    if backdrop_dismissable {
                        close_for_overlay(window, cx)
                    }
                })
                .child(
                    div()
                        .id("dialog-box")
                        .role(Role::Dialog)
                        .when_some(plain_title.clone(), |d, title| d.aria_label(title))
                        .children(plain_title.map(window_title))
                        .min_w(zpx(400.))
                        .max_w(zpx(600.))
                        .when_some(ghd_dialog_width(id), |d, w| d.w(zpx(w)))
                        // a `<dialog>` never outgrows the viewport; the content scrolls
                        .max_h(viewport.height)
                        .flex()
                        .flex_col()
                        .rounded(BORDER_RADIUS())
                        .bg(t.background)
                        .text_color(t.text)
                        .border_1()
                        .border_color(t.box_border)
                        .shadow(vec![BoxShadow {
                            color: t.shadow,
                            offset: point(zpx(0.), zpx(2.)),
                            blur_radius: css_blur(7.),
                            spread_radius: zpx(0.),
                            inset: false,
                        }])
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .when(show_header, |d| {
                            d.child(
                                // header
                                div()
                                    .h(zpx(50.))
                                    .flex_none()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .px(SPACING_DOUBLE())
                                    .when(header_border, |d| {
                                        d.border_b_1().border_color(t.box_border)
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(FONT_SIZE_MD())
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    // `loading`: a spinning `syncClockwise` before the close button
                                    .when(loading, |d| {
                                        d.child(div().flex_none().mr(SPACING()).child(
                                            loading_icon("dialog-loading", t.text_secondary),
                                        ))
                                    })
                                    .child({
                                        let on_close = on_close.clone();
                                        div()
                                            .id("dialog-close")
                                            .relative()
                                            .icon_button_label("Close")
                                            .size(zpx(16.))
                                            .cursor_pointer()
                                            .on_click(move |_, window, cx| on_close(window, cx))
                                            // Chromium's focus ring: 2 px out,
                                            // 2 px wide, a 1 px dark halo
                                            .when(focus_close, |d| {
                                                d.child(
                                                    div()
                                                        .absolute()
                                                        .top(zpx(-5.))
                                                        .left(zpx(-5.))
                                                        .right(zpx(-5.))
                                                        .bottom(zpx(-5.))
                                                        .border_1()
                                                        .border_color(rgb(0x101010))
                                                        .rounded(zpx(6.))
                                                        .child(
                                                            div()
                                                                .size_full()
                                                                .border_2()
                                                                .border_color(t.focus)
                                                                .rounded(zpx(5.)),
                                                        ),
                                                )
                                            })
                                            .child(octicon(Octicon::X, t.text_secondary))
                                    }),
                            )
                        })
                        .child(dialog_content(kind, content, content_padding, t))
                        .children(footer)
                        .when(!buttons.is_empty(), |d| {
                            d.child(
                                // `.dialog-footer`: a top border, 20 px padding,
                                // an optional message 10 px above the buttons
                                // (`margin: -10px 0 10px`), buttons 5 px apart
                                div()
                                    .flex_none()
                                    .flex()
                                    .flex_col()
                                    .p(SPACING_DOUBLE())
                                    .border_t_1()
                                    .border_color(t.box_border)
                                    .text_size(FONT_SIZE())
                                    .line_height(zpx(18.))
                                    .children(footer_message.map(|message| {
                                        div()
                                            // a block takes the dialog's width
                                            // without widening it
                                            .w(zpx(0.))
                                            .min_w_full()
                                            .mt(-SPACING())
                                            .mb(SPACING())
                                            .child(message)
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .justify_end()
                                            .gap(SPACING_HALF())
                                            .children(buttons.into_iter().map(|b| {
                                                let on_click = b.on_click;
                                                let disabled = b.disabled;
                                                if b.primary {
                                                    let button = crate::widgets::primary_button(
                                                        b.id, b.label, disabled, cx,
                                                    )
                                                    .min_w(zpx(120.))
                                                    .when(focus_primary, |d| {
                                                        d.bg(t.button_hover_background)
                                                    })
                                                    .when(!disabled, |d| {
                                                        d.on_click(move |_, window, cx| {
                                                            on_click(window, cx)
                                                        })
                                                    });
                                                    div()
                                                        .relative()
                                                        .when(focus_primary, |d| {
                                                            d.child(crate::widgets::focus_ring(cx))
                                                        })
                                                        .child(button)
                                                        .into_any_element()
                                                } else {
                                                    if disabled {
                                                        crate::widgets::button_disabled(
                                                            b.id, b.label, cx,
                                                        )
                                                    } else {
                                                        crate::widgets::button(b.id, b.label, cx)
                                                    }
                                                    .min_w(zpx(120.))
                                                    .when(!disabled, |d| {
                                                        d.on_click(move |_, window, cx| {
                                                            on_click(window, cx)
                                                        })
                                                    })
                                                    .into_any_element()
                                                }
                                            })),
                                    ),
                            )
                        }),
                ),
        ),
    )
    .with_priority(20)
}
