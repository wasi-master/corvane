//! Electron's classic Linux menu bar (`shell/browser/ui/views/menu_bar.cc`,
//! `submenu_button.cc`), which GitHub Desktop shows on Linux instead of its
//! macOS title bar or Windows app menu (`app.tsx`: `renderTitlebar` returns
//! null, "Linux still uses the classic Electron menu").
//!
//! The items are the menu model `crates/corvane/src/menus.rs` installs with
//! `cx.set_menus` (GHD's non-darwin `build-default-menu.ts`), with `&`
//! mnemonics; the enabled state is GPUI's (an action without a live handler
//! is disabled, as in the macOS menu bar) combined with GHD's
//! `getMenuState` (`corvane_core::menu_state`); accelerators come from the
//! keymap. Dropdowns are `views_menu` popups.
//!
//! Measured from GitHub Desktop 3.6.6 on Linux (Adwaita): the bar is 28 px,
//! `#f6f5f4`, its buttons 6 px either side of the label, the open button
//! `#d7d7d6` over the top 27 px, the keyboard-focused one `#e8e7e6` with
//! the edge of Chromium's focus ring past its right side. Alt shows the
//! mnemonic underlines; Alt alone focuses the bar (← / → move, ↓ / ↑ /
//! Enter open, Escape leaves); Alt+letter opens that menu. Electron's zoom
//! applies to the page only, so the bar ignores the window zoom factor.

use std::rc::Rc;

use corvane_core::menu_state::MenuId;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::views_menu::{self, Anchor, Entry, EntryKind, Palette, Source};

pub const HEIGHT: f32 = 28.;
/// The open button's background covers the bar minus its last row.
const OPEN_HEIGHT: f32 = 27.;
const BUTTON_PADDING: f32 = 6.;

/// GHD's menu id for one of Corvane's menu actions (`menu-update.ts` works
/// on ids).
fn menu_id(action: &dyn Action) -> Option<MenuId> {
    let name = action.name();
    let short = name.rsplit("::").next().unwrap_or(name);
    Some(match short {
        "RenameBranch" => MenuId::RenameBranch,
        "DeleteBranch" => MenuId::DeleteBranch,
        "DiscardAllChanges" => MenuId::DiscardAllChanges,
        "StashAllChanges" => MenuId::StashAllChanges,
        "OpenSettings" => MenuId::Preferences,
        "UpdateFromDefaultBranch" => MenuId::UpdateBranchWithContributionTargetBranch,
        "CompareToBranch" => MenuId::CompareToBranch,
        "MergeIntoCurrentBranch" => MenuId::MergeBranch,
        "RebaseCurrentBranch" => MenuId::RebaseBranch,
        "ViewOnGitHub" => MenuId::ViewRepositoryOnGithub,
        "CompareOnGitHub" => MenuId::CompareOnGithub,
        "ViewBranchOnGitHub" => MenuId::BranchOnGithub,
        "OpenInShell" => MenuId::OpenInShell,
        "Push" => MenuId::Push,
        "Pull" => MenuId::Pull,
        "Fetch" => MenuId::Fetch,
        "GoToSummary" => MenuId::GoToCommitMessage,
        "NewBranch" => MenuId::CreateBranch,
        "ShowChanges" => MenuId::ShowChanges,
        "ShowHistory" => MenuId::ShowHistory,
        "ShowRepositoryList" => MenuId::ShowRepositoryList,
        "ShowBranchesList" => MenuId::ShowBranchesList,
        "ShowInFinder" => MenuId::OpenWorkingDirectory,
        "RepositorySettings" => MenuId::ShowRepositorySettings,
        "OpenInEditor" => MenuId::OpenExternalEditor,
        "OpenWith" => MenuId::OpenWithExternalEditor,
        "RemoveRepository" => MenuId::RemoveRepository,
        "NewRepository" => MenuId::NewRepository,
        "AddLocalRepository" => MenuId::AddLocalRepository,
        "CloneRepository" => MenuId::CloneRepository,
        "About" => MenuId::About,
        "CreatePullRequest" => MenuId::CreatePullRequest,
        "PreviewPullRequest" => MenuId::PreviewPullRequest,
        "SquashAndMergeIntoCurrentBranch" => MenuId::SquashAndMergeBranch,
        "ToggleStashedChanges" => MenuId::ToggleStashedChanges,
        "NewWorktree" => MenuId::CreateWorktree,
        "ShowWorktreesList" => MenuId::ShowWorktreesList,
        "CreateIssue" => MenuId::CreateIssueInRepositoryOnGithub,
        "ToggleChangesFilter" => MenuId::ToggleChangesFilter,
        "ExpandActiveResizable" => MenuId::IncreaseActiveResizableWidth,
        "ContractActiveResizable" => MenuId::DecreaseActiveResizableWidth,
        _ => return None,
    })
}

/// Electron's role accelerators, for the Edit items whose actions the
/// text fields bind themselves.
fn role_accelerator(os_action: OsAction) -> &'static str {
    match os_action {
        OsAction::Undo => "Ctrl+Z",
        OsAction::Redo => "Ctrl+Shift+Z",
        OsAction::Cut => "Ctrl+X",
        OsAction::Copy => "Ctrl+C",
        OsAction::Paste => "Ctrl+V",
        OsAction::SelectAll => "Ctrl+A",
    }
}

struct Button {
    text: SharedString,
    mnemonic: Option<usize>,
    x: f32,
    width: f32,
}

pub struct MenuBar {
    /// The keyboard-focused button (Alt released alone, or ← / →).
    focused: Option<usize>,
    /// Alt is down with no other key yet: underlines show, and releasing
    /// it focuses the bar.
    alt_alone: bool,
    buttons: Vec<Button>,
}

impl MenuBar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let this = cx.entity().downgrade();
        views_menu::set_switch_menu(Rc::new(move |delta, cx| {
            let Some(bar) = this.upgrade() else {
                return;
            };
            let Some(next) = bar.read(cx).neighbour(delta, cx) else {
                return;
            };
            let Some(window) = cx.active_window() else {
                return;
            };
            window
                .update(cx, |_, window, cx| {
                    bar.update(cx, |bar, cx| bar.open(next, true, window, cx))
                })
                .ok();
        }));
        let bar = cx.entity().downgrade();
        cx.intercept_keystrokes(move |event, window, cx| {
            bar.update(cx, |bar, cx| bar.intercept(&event.keystroke, window, cx))
                .ok();
        })
        .detach();
        Self {
            focused: None,
            alt_alone: false,
            buttons: Vec::new(),
        }
    }

    fn menus(cx: &App) -> Vec<OwnedMenu> {
        cx.get_menus().unwrap_or_default()
    }

    /// Alt+letter, and the keys of a focused bar.
    fn intercept(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        self.alt_alone = false;
        match views_menu::last_key() {
            views_menu::LastKey::Activated => {
                // Chromium leaves the menu bar once an item runs
                self.unfocus(cx);
                return;
            }
            views_menu::LastKey::Consumed => return,
            views_menu::LastKey::Passed => {}
        }
        if views_menu::is_open(cx) {
            return;
        }
        let m = &keystroke.modifiers;
        let letter = keystroke
            .key
            .chars()
            .next()
            .filter(|c| keystroke.key.chars().count() == 1 && c.is_alphanumeric())
            .map(|c| c.to_ascii_lowercase());
        let by_mnemonic = |bar: &Self| {
            letter.and_then(|l| {
                bar.buttons
                    .iter()
                    .position(|b| views_menu::mnemonic_char(&b.text, b.mnemonic) == Some(l))
            })
        };
        if m.alt && !m.control && !m.platform {
            if let Some(index) = by_mnemonic(self) {
                self.open(index, true, window, cx);
                cx.stop_propagation();
            }
            return;
        }
        let Some(focused) = self.focused else {
            return;
        };
        if m.control || m.platform {
            self.unfocus(cx);
            return;
        }
        let count = self.buttons.len();
        match keystroke.key.as_str() {
            "left" if count > 0 => self.focused = Some((focused + count - 1) % count),
            "right" if count > 0 => self.focused = Some((focused + 1) % count),
            "down" | "up" | "enter" | "space" => self.open(focused, true, window, cx),
            "escape" => self.focused = None,
            _ => match by_mnemonic(self) {
                Some(index) => self.open(index, true, window, cx),
                None => self.focused = None,
            },
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn unfocus(&mut self, cx: &mut Context<Self>) {
        if self.focused.take().is_some() {
            cx.notify();
        }
    }

    /// ← / → with a menu open: the neighbouring menu's index.
    fn neighbour(&self, delta: isize, cx: &App) -> Option<usize> {
        let open = views_menu::open_menu_bar_index(cx)?;
        let count = self.buttons.len() as isize;
        (count > 0).then(|| (open as isize + delta).rem_euclid(count) as usize)
    }

    /// Open menu `index` under its button.
    fn open(&mut self, index: usize, keyboard: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = Self::menus(cx).into_iter().nth(index) else {
            return;
        };
        let Some(button) = self.buttons.get(index) else {
            return;
        };
        let entries = entries(&menu.items, window, cx);
        let rect = Bounds::new(
            point(px(button.x), px(0.)),
            size(px(button.width), px(OPEN_HEIGHT)),
        );
        // the bar keeps the keyboard position while its menu is open
        self.focused = keyboard.then_some(index);
        let bar = cx.entity().downgrade();
        let on_close: views_menu::OnClose = Rc::new(move |cx| {
            bar.update(cx, |_, cx| cx.notify()).ok();
        });
        views_menu::open(
            entries,
            Anchor::below(rect),
            Source::MenuBar(index),
            keyboard,
            Some(on_close),
            window,
            cx,
        );
        cx.notify();
    }

    fn button_at(&self, x: f32) -> Option<usize> {
        self.buttons
            .iter()
            .position(|b| x >= b.x && x < b.x + b.width)
    }
}

/// The views menu rows for a GPUI menu's items.
fn entries(items: &[OwnedMenuItem], window: &mut Window, cx: &mut App) -> Vec<Entry> {
    let state = corvane_core::AppState::try_global(cx).map(|s| {
        let state = s.read(cx);
        corvane_core::menu_state::menu_state(state)
    });
    let mut out: Vec<Entry> = Vec::new();
    for item in items {
        match item {
            OwnedMenuItem::Separator => {
                // no leading, trailing or doubled separators (hidden items)
                if out
                    .last()
                    .is_some_and(|e| !matches!(e.kind, EntryKind::Separator))
                {
                    out.push(Entry::separator());
                }
            }
            OwnedMenuItem::Submenu(menu) => {
                let (text, mnemonic) = views_menu::parse_mnemonic(&menu.name);
                out.push(Entry {
                    text: text.into(),
                    mnemonic,
                    accelerator: None,
                    enabled: !menu.disabled,
                    checked: None,
                    kind: EntryKind::Submenu(entries(&menu.items, window, cx)),
                });
            }
            OwnedMenuItem::SystemMenu(_) => {}
            OwnedMenuItem::Action {
                name,
                action,
                os_action,
                checked,
                disabled,
            } => {
                let (text, mnemonic) = views_menu::parse_mnemonic(name);
                let available =
                    window.is_action_available(&**action, cx) || cx.is_action_available(&**action);
                let ghd_enabled = match (menu_id(&**action), state.as_ref()) {
                    (Some(id), Some(state)) => state.get(&id).copied().unwrap_or(true),
                    _ => true,
                };
                let accelerator = window
                    .bindings_for_action(&**action)
                    .last()
                    .and_then(|binding| binding.keystrokes().first().cloned())
                    .map(|k| views_menu::accelerator_text(k.inner()))
                    .or_else(|| os_action.map(|a| role_accelerator(a).to_string()));
                let action = action.boxed_clone();
                out.push(Entry {
                    text: text.into(),
                    mnemonic,
                    accelerator: accelerator.map(Into::into),
                    enabled: !disabled && available && ghd_enabled,
                    checked: checked.then_some(true),
                    kind: EntryKind::Action(Rc::new(move |window, cx| {
                        window.dispatch_action(action.boxed_clone(), cx);
                    })),
                });
            }
        }
    }
    while out
        .last()
        .is_some_and(|e| matches!(e.kind, EntryKind::Separator))
    {
        out.pop();
    }
    out
}

impl Render for MenuBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_window(window);
        let font = Font {
            family: views_menu::font_family(),
            ..Font::default()
        };
        let font_size = px(views_menu::metrics::BAR_FONT_SIZE);
        let mut x = 0.;
        self.buttons = Self::menus(cx)
            .iter()
            .map(|menu| {
                let (text, mnemonic) = views_menu::parse_mnemonic(&menu.name);
                let run = TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: palette.bar_text,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let text_width = f32::from(
                    window
                        .text_system()
                        .shape_line(SharedString::from(text.clone()), font_size, &[run], None)
                        .width,
                );
                let width = (text_width + 2. * BUTTON_PADDING).floor();
                let button = Button {
                    text: text.into(),
                    mnemonic,
                    x,
                    width,
                };
                x += width;
                button
            })
            .collect();
        let open = views_menu::open_menu_bar_index(cx);
        let underline =
            self.alt_alone || self.focused.is_some() || views_menu::opened_from_keyboard(cx);
        let buttons = self.buttons.iter().enumerate().map(|(ix, b)| {
            let is_open = open == Some(ix);
            let hot = !is_open && self.focused == Some(ix);
            let label = match b.mnemonic.filter(|_| underline) {
                Some(at) => {
                    views_menu::mnemonic_text(&b.text, at, palette.bar_text).into_any_element()
                }
                None => div().child(b.text.clone()).into_any_element(),
            };
            div()
                .absolute()
                .left(px(b.x))
                .top_0()
                .w(px(b.width))
                .h(px(HEIGHT))
                .when(is_open, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(px(OPEN_HEIGHT))
                            .rounded(px(1.))
                            .bg(palette.bar_open),
                    )
                })
                .when(hot, |d| {
                    d.bg(palette.bar_hot).child(
                        // Chromium's focus ring: only its right edge shows
                        // past the button
                        div()
                            .absolute()
                            .top_0()
                            .left(px(b.width + 2.))
                            .w(px(2.))
                            .h(px(HEIGHT))
                            .bg(palette.focus_ring),
                    )
                })
                .child(
                    div()
                        .absolute()
                        .left(px(BUTTON_PADDING))
                        .top_0()
                        .h(px(HEIGHT))
                        .flex()
                        .items_center()
                        .child(label),
                )
        });
        div()
            .id("menu-bar")
            .relative()
            .w_full()
            .h(px(HEIGHT))
            .flex_none()
            .bg(palette.bar_background)
            .text_color(palette.bar_text)
            .font_family(views_menu::font_family())
            .text_size(font_size)
            .line_height(px(20.))
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                let alt_only = event.modifiers.alt
                    && !event.modifiers.control
                    && !event.modifiers.shift
                    && !event.modifiers.platform;
                if alt_only {
                    if !this.alt_alone {
                        this.alt_alone = true;
                        cx.notify();
                    }
                } else if this.alt_alone && !event.modifiers.alt {
                    // Alt released on its own: focus the bar, or leave it
                    this.alt_alone = false;
                    if views_menu::is_open(cx) {
                        views_menu::close_all(cx);
                        this.focused = None;
                    } else {
                        this.focused = if this.focused.is_some() {
                            None
                        } else {
                            Some(0)
                        };
                    }
                    cx.notify();
                } else if this.alt_alone {
                    this.alt_alone = false;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let Some(index) = this.button_at(f32::from(event.position.x)) else {
                        views_menu::close_all(cx);
                        this.unfocus(cx);
                        return;
                    };
                    if views_menu::open_menu_bar_index(cx) == Some(index) {
                        views_menu::close_all(cx);
                    } else {
                        this.open(index, false, window, cx);
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                // with a menu open, hovering another button opens its menu
                let Some(open) = views_menu::open_menu_bar_index(cx) else {
                    return;
                };
                if let Some(index) = this.button_at(f32::from(event.position.x))
                    && index != open
                {
                    this.open(index, false, window, cx);
                }
            }))
            .children(buttons)
    }
}

/// The main window's root on Linux: the menu bar over the app, as Electron
/// draws its menu bar over the page. Presses outside an open menu close it
/// and go no further (Chromium's menus take the press that dismisses them).
pub struct MenuBarShell {
    menu_bar: Entity<MenuBar>,
    content: AnyView,
}

impl MenuBarShell {
    pub fn new(content: AnyView, cx: &mut Context<Self>) -> Self {
        let menu_bar = cx.new(MenuBar::new);
        Self { menu_bar, content }
    }
}

impl Render for MenuBarShell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let menu_bar = self.menu_bar.clone();
        div()
            .id("menu-bar-shell")
            .size_full()
            .flex()
            .flex_col()
            .capture_any_mouse_down(move |event, _, cx| {
                // the bar handles presses on itself
                if f32::from(event.position.y) >= HEIGHT && views_menu::dismiss_on_outside_click(cx)
                {
                    cx.stop_propagation();
                }
                if f32::from(event.position.y) >= HEIGHT {
                    menu_bar.update(cx, |bar, cx| bar.unfocus(cx));
                }
            })
            .child(self.menu_bar.clone())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.content.clone()),
            )
    }
}
