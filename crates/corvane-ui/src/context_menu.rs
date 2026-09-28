//! Right-click menus. GHD shows native NSMenus on macOS (`showContextualMenu`);
//! GPUI has no native context-menu API, so this reproduces the system look:
//! 5 px inset, 22 px items, 13 px text, accent highlight, 8 px radius.

use std::rc::Rc;

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::CloseFoldout;
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;

pub type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

pub enum MenuItemKind {
    Action(MenuAction),
    Submenu(Vec<MenuItem>),
    Separator,
}

pub struct MenuItem {
    pub label: SharedString,
    pub enabled: bool,
    pub kind: MenuItemKind,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            kind: MenuItemKind::Action(Rc::new(action)),
        }
    }

    pub fn submenu(label: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            kind: MenuItemKind::Submenu(items),
        }
    }

    pub fn separator() -> Self {
        Self {
            label: SharedString::default(),
            enabled: false,
            kind: MenuItemKind::Separator,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

const ITEM_HEIGHT: f32 = 22.;
const SEPARATOR_HEIGHT: f32 = 11.;
const INSET: f32 = 5.;
const TEXT_SIZE: f32 = 13.;

pub struct ContextMenu {
    position: Point<Pixels>,
    items: Vec<MenuItem>,
    open_submenu: Option<usize>,
    focus_handle: FocusHandle,
    previous_focus: Option<FocusHandle>,
}

impl EventEmitter<DismissEvent> for ContextMenu {}

impl ContextMenu {
    /// Takes focus so Escape closes the menu; focus goes back where it was on dismiss.
    pub fn new(
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let previous_focus = window.focused(cx);
        window.focus(&focus_handle, cx);
        Self {
            position,
            items,
            open_submenu: None,
            focus_handle,
            previous_focus,
        }
    }

    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prev) = self.previous_focus.take() {
            window.focus(&prev, cx);
        }
        cx.emit(DismissEvent);
    }

    fn width(items: &[MenuItem]) -> Pixels {
        let longest = items
            .iter()
            .map(|i| i.label.chars().count())
            .max()
            .unwrap_or(0) as f32;
        px((longest * 6.8 + 48.).clamp(160., 440.))
    }

    fn height(items: &[MenuItem]) -> Pixels {
        let inner: f32 = items
            .iter()
            .map(|i| {
                if matches!(i.kind, MenuItemKind::Separator) {
                    SEPARATOR_HEIGHT
                } else {
                    ITEM_HEIGHT
                }
            })
            .sum();
        px(inner + INSET * 2. + 2.)
    }

    fn panel(&self, items: &[MenuItem], root: bool, cx: &Context<Self>) -> Div {
        let t = cx.ghd();
        let width = Self::width(items);
        div()
            .w(width)
            .p(px(INSET))
            .flex()
            .flex_col()
            .rounded(px(8.))
            .bg(t.background)
            .border_1()
            .border_color(t.box_border)
            .shadow(vec![BoxShadow {
                color: t.shadow,
                offset: point(px(0.), px(4.)),
                blur_radius: px(16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .children(
                items
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| self.item(idx, item, root, width, cx)),
            )
    }

    fn item(
        &self,
        idx: usize,
        item: &MenuItem,
        root: bool,
        width: Pixels,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = cx.ghd();
        if matches!(item.kind, MenuItemKind::Separator) {
            return div()
                .h(px(1.))
                .my(px(5.))
                .mx(px(10.))
                .flex_none()
                .bg(t.box_border)
                .into_any_element();
        }
        let enabled = item.enabled;
        let is_submenu = matches!(item.kind, MenuItemKind::Submenu(_));
        let open = root && self.open_submenu == Some(idx);
        let accent_bg = t.box_selected_active_background;
        let accent_text = t.box_selected_active_text;
        let id = if root {
            ("ctx-item", idx)
        } else {
            ("ctx-subitem", idx)
        };
        let mut el = div()
            .id(id)
            .relative()
            .h(px(ITEM_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .px(px(10.))
            .rounded(px(4.))
            .text_size(px(TEXT_SIZE))
            .text_color(if enabled { t.text } else { t.text_secondary })
            .when(open, |d| d.bg(accent_bg).text_color(accent_text))
            .when(enabled, |d| {
                d.hover(move |s| s.bg(accent_bg).text_color(accent_text))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(item.label.clone()),
            );
        if is_submenu {
            el = el.child(octicon(Octicon::ChevronRight, t.text_secondary).size(px(12.)));
        }
        if root && enabled {
            el = el.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    let next = if is_submenu { Some(idx) } else { None };
                    if this.open_submenu != next {
                        this.open_submenu = next;
                        cx.notify();
                    }
                }
            }));
        }
        match &item.kind {
            MenuItemKind::Action(action) if enabled => {
                let action = action.clone();
                el = el.on_click(cx.listener(move |this, _, window, cx| {
                    this.dismiss(window, cx);
                    action(window, cx);
                }));
            }
            MenuItemKind::Submenu(children) if open => {
                el = el.child(
                    div()
                        .absolute()
                        .left(width - px(16.))
                        .top(px(-INSET - 1.))
                        .child(self.panel(children, false, cx)),
                );
            }
            _ => {}
        }
        el.into_any_element()
    }
}

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let width = Self::width(&self.items);
        let height = Self::height(&self.items);
        let mut x = self.position.x;
        let mut y = self.position.y;
        if x + width > viewport.width {
            x = viewport.width - width - px(4.);
        }
        if y + height > viewport.height {
            y = viewport.height - height - px(4.);
        }
        if x < px(0.) {
            x = px(0.);
        }
        if y < px(0.) {
            y = px(0.);
        }
        let panel = self.panel(&self.items, true, cx);
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("context-menu-overlay")
                    .track_focus(&self.focus_handle)
                    .relative()
                    .w(viewport.width)
                    .h(viewport.height)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.dismiss(window, cx)),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, window, cx| this.dismiss(window, cx)),
                    )
                    // Escape is bound to CloseFoldout; bindings win over key
                    // listeners, so handle the action (and raw key as fallback).
                    .on_action(cx.listener(|this, _: &CloseFoldout, window, cx| {
                        this.dismiss(window, cx)
                    }))
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        if ev.keystroke.key == "escape" {
                            this.dismiss(window, cx);
                        }
                    }))
                    .child(div().absolute().left(x).top(y).child(panel)),
            ),
        )
        .with_priority(30)
    }
}
