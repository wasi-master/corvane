//! Chromium views menus as Electron shows them on Linux: the menu bar's
//! dropdowns (`shell/browser/ui/views/menu_bar.cc`) and context menus
//! (`Menu.popup`, GHD `showContextualMenu`). Each level is its own popup
//! window (`WindowKind::AnchoredPopup`: an override-redirect window on X11,
//! an `xdg_popup` on Wayland), so a menu can extend past the main window as
//! Chromium's does.
//!
//! Geometry and colours are Chromium's `MenuConfig` (`menu_config.cc`,
//! `menu_item_view.cc`, `submenu_view.cc`) measured from GitHub Desktop
//! 3.6.6 on Linux (Electron 42, GTK Adwaita): 32 px items (6 px margins
//! around a 20 px line), 17 px separators with the rule at +8, 4 px above
//! and below the items, labels at x 20, accelerators right-aligned 21 px
//! from the edge, Noto Sans 10 pt. Without a compositing window manager
//! Chromium draws the menu square and without a shadow (Xvfb, openbox);
//! Corvane does the same everywhere.
//!
//! Keyboard: the main window keeps the keyboard focus (Chromium grabs it;
//! GPUI's popups on X11 do not), and [`install`]'s keystroke interceptor
//! routes keys to the open menu: ↑ / ↓ / Home / End move, → opens a
//! submenu (or the next menu bar menu), ← closes one (or goes to the
//! previous menu), Enter / Space activate, Escape closes a level, a
//! mnemonic letter activates its item. A click outside the menus, or the
//! main window losing focus, closes them.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::popup::{PopupAnchor, PopupConstraintAdjustment, PopupGravity, PopupOptions};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{MenuItem, MenuItemKind};

/// Chromium's `MenuConfig` and the measured GitHub Desktop menus.
pub mod metrics {
    /// Menu text (`style::CONTEXT_MENU`, `STYLE_BODY_3`): the 10 pt UI font
    /// one pixel larger (measured: "Select all" is 59 px of ink in GHD).
    pub const FONT_SIZE: f32 = 13.333 + 1.;
    /// The menu bar's buttons use the UI font as is (10 pt at 96 dpi,
    /// rounded to whole pixels).
    pub const BAR_FONT_SIZE: f32 = 13.;
    pub const ITEM_HEIGHT: f32 = 32.;
    pub const SEPARATOR_HEIGHT: f32 = 17.;
    /// Where the separator's 1 px rule sits inside its 17 px.
    pub const SEPARATOR_RULE: f32 = 8.;
    /// Space above the first and below the last item.
    pub const VERTICAL_INSET: f32 = 4.;
    pub const LABEL_START: f32 = 20.;
    /// Right of the widest label (fitted with the `&` quirk in `measure`).
    pub const TRAILING: f32 = 20.;
    /// Accelerator column: the widest accelerator plus this (fitted to the
    /// GHD menus: Edit 197 px, Branch 474 px).
    pub const ACCELERATOR_PADDING: f32 = 10.;
    /// Text sits a pixel above the row's centre.
    pub const TEXT_RAISE: f32 = 1.;
    /// From the accelerators' right edge to the menu's.
    pub const ACCELERATOR_RIGHT: f32 = 21.;
    /// A submenu arrow: `arrow_size` + `arrow_to_edge_padding`.
    pub const ARROW_COLUMN: f32 = 24.;
    /// A check mark column in front of the labels of a menu with checkboxes.
    pub const CHECK_COLUMN: f32 = 24.;
    /// `MenuConfig::show_delay`: hovering a submenu item opens it after this.
    pub const SUBMENU_DELAY_MS: u64 = 400;
}

/// The GTK theme's menu colours Chromium draws with (Adwaita).
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub background: Hsla,
    pub text: Hsla,
    pub accelerator: Hsla,
    pub disabled: Hsla,
    pub separator: Hsla,
    pub highlight: Hsla,
    pub highlight_text: Hsla,
    /// The menu bar (`MenuBar`, `SubmenuButton`).
    pub bar_background: Hsla,
    pub bar_text: Hsla,
    /// A bar button whose menu is open.
    pub bar_open: Hsla,
    /// The keyboard-focused bar button.
    pub bar_hot: Hsla,
    pub focus_ring: Hsla,
}

impl Palette {
    /// Adwaita (light), measured from GitHub Desktop 3.6.6 under Xvfb.
    pub fn light() -> Self {
        Self {
            background: rgb(0xffffff).into(),
            text: rgb(0x2e3436).into(),
            accelerator: rgb(0x8c9091).into(),
            disabled: rgb(0x929595).into(),
            separator: rgb(0xe6e6e6).into(),
            highlight: rgb(0x3584e4).into(),
            highlight_text: rgb(0xffffff).into(),
            bar_background: rgb(0xf6f5f4).into(),
            bar_text: rgb(0x2e3436).into(),
            bar_open: rgb(0xd7d7d6).into(),
            bar_hot: rgb(0xe8e7e6).into(),
            focus_ring: rgb(0x0b57d0).into(),
        }
    }

    /// Adwaita dark, for a desktop that prefers dark (Chromium follows the
    /// GTK theme, not GitHub Desktop's own theme).
    pub fn dark() -> Self {
        Self {
            background: rgb(0x2b2b2b).into(),
            text: rgb(0xeeeeec).into(),
            accelerator: rgb(0x9c9c9a).into(),
            disabled: rgb(0x919190).into(),
            separator: rgb(0x3e3e3e).into(),
            highlight: rgb(0x3584e4).into(),
            highlight_text: rgb(0xffffff).into(),
            bar_background: rgb(0x303030).into(),
            bar_text: rgb(0xeeeeec).into(),
            bar_open: rgb(0x4a4a4a).into(),
            bar_hot: rgb(0x3d3d3d).into(),
            focus_ring: rgb(0x7ab4ff).into(),
        }
    }

    pub fn for_window(window: &Window) -> Self {
        match window.appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
            _ => Self::light(),
        }
    }
}

/// The font Chromium's menus use: the desktop UI font (fontconfig
/// `sans-serif`, Noto Sans on Ubuntu).
pub fn font_family() -> SharedString {
    crate::theme::ui_font()
}

/// `&File` → ("File", Some(0)); `&&` is a literal ampersand.
pub fn parse_mnemonic(label: &str) -> (String, Option<usize>) {
    let mut text = String::with_capacity(label.len());
    let mut mnemonic = None;
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.peek() {
                Some('&') => {
                    chars.next();
                    text.push('&');
                }
                Some(_) if mnemonic.is_none() => mnemonic = Some(text.len()),
                _ => {}
            }
        } else {
            text.push(c);
        }
    }
    (text, mnemonic)
}

/// The mnemonic's character, lowercased.
pub fn mnemonic_char(text: &str, mnemonic: Option<usize>) -> Option<char> {
    text.get(mnemonic?..)?
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
}

pub type EntryAction = Rc<dyn Fn(&mut Window, &mut App)>;
/// Runs when a menu session ends.
pub type OnClose = Rc<dyn Fn(&mut App)>;

#[derive(Clone)]
pub enum EntryKind {
    Action(EntryAction),
    Submenu(Vec<Entry>),
    Separator,
}

/// One row of a views menu.
#[derive(Clone)]
pub struct Entry {
    pub text: SharedString,
    pub mnemonic: Option<usize>,
    pub accelerator: Option<SharedString>,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub kind: EntryKind,
}

impl Entry {
    pub fn separator() -> Self {
        Self {
            text: SharedString::default(),
            mnemonic: None,
            accelerator: None,
            enabled: false,
            checked: None,
            kind: EntryKind::Separator,
        }
    }

    fn is_separator(&self) -> bool {
        matches!(self.kind, EntryKind::Separator)
    }

    fn selectable(&self) -> bool {
        self.enabled && !self.is_separator()
    }

    /// A context menu item (`context_menu::MenuItem`); its label may carry
    /// an `&` mnemonic.
    pub fn from_menu_item(item: &MenuItem) -> Self {
        let (text, mnemonic) = parse_mnemonic(&item.label);
        Self {
            text: text.into(),
            mnemonic,
            accelerator: None,
            enabled: item.enabled,
            checked: item.checked,
            kind: match &item.kind {
                MenuItemKind::Separator => EntryKind::Separator,
                MenuItemKind::Action(action) => EntryKind::Action(action.clone()),
                MenuItemKind::Submenu(items) => {
                    EntryKind::Submenu(items.iter().map(Self::from_menu_item).collect())
                }
            },
        }
    }
}

/// Where a session came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Context,
    /// The menu bar's menu at this index.
    MenuBar(usize),
}

struct Level {
    entries: Vec<Entry>,
    highlighted: Option<usize>,
    window: WindowHandle<MenuLevelView>,
    /// Top of each row inside the menu.
    row_tops: Vec<f32>,
}

/// The open menus. One session at a time, like Chromium's `MenuController`.
struct Session {
    owner: AnyWindowHandle,
    source: Source,
    levels: Vec<Level>,
    /// Opened from the keyboard: the first item starts highlighted.
    keyboard: bool,
    submenu_timer: Option<Task<()>>,
    _auto_dismiss: Option<Task<()>>,
}

#[derive(Default)]
struct Menus {
    session: Option<Session>,
    /// Called when the session ends (the menu bar redraws its buttons).
    on_close: Option<OnClose>,
}

impl Global for Menus {}

/// Register the keystroke interceptor that drives open menus from the
/// keyboard. Once, at startup.
pub fn install(cx: &mut App) {
    cx.set_global(Menus::default());
    cx.intercept_keystrokes(|event, _window, cx| {
        LAST_KEY.with(|k| *k.borrow_mut() = LastKey::Passed);
        let consumed = is_open(cx) && handle_key(&event.keystroke, cx);
        LAST_KEY.with(|k| {
            let mut k = k.borrow_mut();
            // `activate` sets `Activated` while the key is handled
            if consumed && *k == LastKey::Passed {
                *k = LastKey::Consumed;
            }
        });
        if consumed {
            cx.stop_propagation();
        }
    })
    .detach();
}

/// What the open menu did with the latest keystroke; the menu bar's own
/// interceptor runs after this one and must not act on it again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LastKey {
    Passed,
    Consumed,
    /// The key ran an item (the menus are closed).
    Activated,
}

thread_local! {
    static LAST_KEY: std::cell::RefCell<LastKey> = const { std::cell::RefCell::new(LastKey::Passed) };
}

/// What the menus did with the keystroke being dispatched.
pub fn last_key() -> LastKey {
    LAST_KEY.with(|k| *k.borrow())
}

/// Whether any menu is open.
pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Menus>()
        .is_some_and(|m| m.session.is_some())
}

/// The menu bar menu that is open, if any.
pub fn open_menu_bar_index(cx: &App) -> Option<usize> {
    let menus = cx.try_global::<Menus>()?;
    match menus.session.as_ref()?.source {
        Source::MenuBar(index) => Some(index),
        Source::Context => None,
    }
}

/// Whether the menus were opened (or are driven) from the keyboard.
pub fn opened_from_keyboard(cx: &App) -> bool {
    cx.try_global::<Menus>()
        .and_then(|m| m.session.as_ref())
        .is_some_and(|s| s.keyboard)
}

/// Where a menu opens, in the owner window's coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub rect: Bounds<Pixels>,
    pub anchor: PopupAnchor,
    pub gravity: PopupGravity,
}

impl Anchor {
    /// A context menu: its top-left corner at the pointer.
    pub fn point(position: Point<Pixels>) -> Self {
        Self {
            rect: Bounds::new(position, size(px(0.), px(0.))),
            anchor: PopupAnchor::TopLeft,
            gravity: PopupGravity::BottomRight,
        }
    }

    /// A dropdown under a menu bar button.
    pub fn below(rect: Bounds<Pixels>) -> Self {
        Self {
            rect,
            anchor: PopupAnchor::BottomLeft,
            gravity: PopupGravity::BottomRight,
        }
    }
}

/// Open `entries` as a new session (closing any other), owned by `window`.
/// `on_close` runs when the session ends.
pub fn open(
    entries: Vec<Entry>,
    anchor: Anchor,
    source: Source,
    keyboard: bool,
    on_close: Option<OnClose>,
    window: &mut Window,
    cx: &mut App,
) {
    close_all(cx);
    let owner = window.window_handle();
    let Some(level) = open_level(entries, anchor, owner, keyboard, window, cx) else {
        return;
    };
    let auto_dismiss = crate::native_menu_common::auto_dismiss().map(|hold| {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(hold).await;
            cx.update(close_all);
        })
    });
    let menus = cx.global_mut::<Menus>();
    menus.on_close = on_close;
    menus.session = Some(Session {
        owner,
        source,
        levels: vec![level],
        keyboard,
        submenu_timer: None,
        _auto_dismiss: auto_dismiss,
    });
    // the level's first frame ran before its entries were stored
    refresh_levels(cx);
}

/// Row tops and the menu's size for `entries`, measured in `window`.
fn measure(entries: &[Entry], window: &Window) -> (Vec<f32>, Size<Pixels>) {
    use metrics::*;
    let font = Font {
        family: font_family(),
        ..Font::default()
    };
    let width_of = |text: &str| -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let run = TextRun {
            len: text.len(),
            font: font.clone(),
            color: black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        f32::from(
            window
                .text_system()
                .shape_line(
                    SharedString::from(text.to_string()),
                    px(FONT_SIZE),
                    &[run],
                    None,
                )
                .width,
        )
    };
    let has_check = entries.iter().any(|e| e.checked.is_some());
    let mut label_max = 0f32;
    let mut minor_max = 0f32;
    let mut row_tops = Vec::with_capacity(entries.len());
    let mut y = VERTICAL_INSET;
    for entry in entries {
        row_tops.push(y);
        if entry.is_separator() {
            y += SEPARATOR_HEIGHT;
            continue;
        }
        y += ITEM_HEIGHT;
        // Chromium measures `title_` with its `&` prefix still in it
        // (`MenuItemView::CalculateDimensions`; it is only stripped when
        // drawn), so a label with a mnemonic reserves an ampersand more
        let ampersand = if entry.mnemonic.is_some() {
            width_of("&")
        } else {
            0.
        };
        label_max = label_max.max(width_of(&entry.text) + ampersand);
        let accelerator = entry
            .accelerator
            .as_ref()
            .map(|a| width_of(a))
            .unwrap_or(0.);
        let arrow = if matches!(entry.kind, EntryKind::Submenu(_)) {
            ARROW_COLUMN
        } else {
            0.
        };
        minor_max = minor_max.max(accelerator + arrow);
    }
    let label_start = LABEL_START + if has_check { CHECK_COLUMN } else { 0. };
    let minor = if minor_max > 0. {
        minor_max + ACCELERATOR_PADDING
    } else {
        0.
    };
    let width = (label_start + label_max.ceil() + TRAILING + minor.ceil()).ceil();
    (row_tops, size(px(width), px(y + VERTICAL_INSET)))
}

fn open_level(
    entries: Vec<Entry>,
    anchor: Anchor,
    owner: AnyWindowHandle,
    keyboard: bool,
    window: &mut Window,
    cx: &mut App,
) -> Option<Level> {
    let (row_tops, menu_size) = measure(&entries, window);
    let level_index = cx
        .try_global::<Menus>()
        .and_then(|m| m.session.as_ref())
        .map_or(0, |s| s.levels.len());
    let highlighted = keyboard
        .then(|| entries.iter().position(Entry::selectable))
        .flatten();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            menu_size,
        ))),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::AnchoredPopup(PopupOptions {
            parent: owner,
            anchor_rect: anchor.rect,
            anchor: anchor.anchor,
            gravity: anchor.gravity,
            constraint_adjustment: PopupConstraintAdjustment::FLIP_X
                | PopupConstraintAdjustment::FLIP_Y
                | PopupConstraintAdjustment::SLIDE_X
                | PopupConstraintAdjustment::SLIDE_Y,
            offset: point(px(0.), px(0.)),
            grab: false,
        }),
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Opaque,
        app_id: Some(corvane_platform::BUNDLE_ID.into()),
        ..Default::default()
    };
    match cx.open_window(options, |_, cx| {
        cx.new(|_| MenuLevelView { level: level_index })
    }) {
        Ok(handle) => Some(Level {
            entries,
            highlighted,
            window: handle,
            row_tops,
        }),
        Err(err) => {
            tracing::warn!(%err, "could not open a menu window");
            None
        }
    }
}

fn remove_windows(levels: Vec<Level>, cx: &mut App) {
    for level in levels {
        level
            .window
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    }
}

/// Close every open menu.
pub fn close_all(cx: &mut App) {
    let Some(menus) = cx.try_global::<Menus>() else {
        return;
    };
    if menus.session.is_none() {
        return;
    }
    let menus = cx.global_mut::<Menus>();
    let session = menus.session.take();
    let on_close = menus.on_close.take();
    if let Some(session) = session {
        let owner = session.owner;
        remove_windows(session.levels, cx);
        owner.update(cx, |_, window, _| window.refresh()).ok();
    }
    if let Some(on_close) = on_close {
        // the owner (the menu bar) may be mid-update: it opens the next
        // menu, which closes this one
        cx.defer(move |cx| on_close(cx));
    }
}

/// Close the levels past `keep` (a submenu's parent stays).
fn close_after(keep: usize, cx: &mut App) {
    let removed = {
        let Some(session) = cx.global_mut::<Menus>().session.as_mut() else {
            return;
        };
        session.submenu_timer = None;
        if session.levels.len() <= keep + 1 {
            return;
        }
        session.levels.split_off(keep + 1)
    };
    remove_windows(removed, cx);
    refresh_levels(cx);
}

fn refresh_levels(cx: &mut App) {
    let windows: Vec<_> = cx
        .try_global::<Menus>()
        .and_then(|m| m.session.as_ref())
        .map(|s| s.levels.iter().map(|l| l.window).collect())
        .unwrap_or_default();
    for handle in windows {
        handle.update(cx, |_, window, _| window.refresh()).ok();
    }
}

fn set_highlight(level: usize, row: Option<usize>, cx: &mut App) {
    if let Some(l) = cx
        .global_mut::<Menus>()
        .session
        .as_mut()
        .and_then(|s| s.levels.get_mut(level))
    {
        if l.highlighted == row {
            return;
        }
        l.highlighted = row;
    }
    refresh_levels(cx);
}

/// Open the submenu of `row` in `level` (closing deeper ones).
fn open_submenu(level: usize, row: usize, keyboard: bool, cx: &mut App) {
    close_after(level, cx);
    let Some((entries, rect, parent_window, owner)) = (|| {
        let session = cx.try_global::<Menus>()?.session.as_ref()?;
        let l = session.levels.get(level)?;
        let EntryKind::Submenu(children) = &l.entries.get(row)?.kind else {
            return None;
        };
        let top = *l.row_tops.get(row)?;
        Some((children.clone(), top, l.window, session.owner))
    })() else {
        return;
    };
    let _ = owner;
    let parent: AnyWindowHandle = parent_window.into();
    let opened = parent_window.update(cx, |_, window, cx| {
        let width = window.bounds().size.width;
        // Chromium lines the submenu's first item up with its parent item
        let anchor = Anchor {
            rect: Bounds::new(
                point(px(0.), px(rect - metrics::VERTICAL_INSET)),
                size(width, px(metrics::ITEM_HEIGHT)),
            ),
            anchor: PopupAnchor::TopRight,
            gravity: PopupGravity::BottomRight,
        };
        open_level(entries, anchor, parent, keyboard, window, cx)
    });
    if let Ok(Some(level)) = opened
        && let Some(session) = cx.global_mut::<Menus>().session.as_mut()
    {
        session.levels.push(level);
    }
    refresh_levels(cx);
}

/// Run the entry at `row` of `level`: open its submenu or close every menu
/// and run its action in the owner window.
fn activate(level: usize, row: usize, keyboard: bool, cx: &mut App) {
    let Some((entry, owner)) = cx
        .try_global::<Menus>()
        .and_then(|m| m.session.as_ref())
        .and_then(|s| Some((s.levels.get(level)?.entries.get(row)?.clone(), s.owner)))
    else {
        return;
    };
    if !entry.selectable() {
        return;
    }
    match entry.kind {
        EntryKind::Submenu(_) => open_submenu(level, row, keyboard, cx),
        EntryKind::Action(action) => {
            if keyboard {
                LAST_KEY.with(|k| *k.borrow_mut() = LastKey::Activated);
            }
            close_all(cx);
            // deferred: a key reaches here from inside the owner window's
            // update (the keystroke interceptor), which cannot nest
            cx.defer(move |cx| {
                owner.update(cx, |_, window, cx| action(window, cx)).ok();
            });
        }
        EntryKind::Separator => {}
    }
}

/// The next selectable row after (or before) `from`, wrapping around
/// (`arrow_key_selection_wraps`).
fn step(entries: &[Entry], from: Option<usize>, forward: bool) -> Option<usize> {
    let n = entries.len();
    if n == 0 {
        return None;
    }
    let start = match (from, forward) {
        (Some(i), _) => i,
        (None, true) => n - 1,
        (None, false) => 0,
    };
    (1..=n)
        .map(|k| {
            if forward {
                (start + k) % n
            } else {
                (start + n - k % n) % n
            }
        })
        .find(|&i| entries[i].selectable())
}

/// Requests from the keyboard that need the menu bar (switching menus).
pub type SwitchMenu = Rc<dyn Fn(isize, &mut App)>;

thread_local! {
    static SWITCH_MENU: std::cell::RefCell<Option<SwitchMenu>> = const { std::cell::RefCell::new(None) };
}

/// The menu bar's handler for ← / → at the top level.
pub fn set_switch_menu(handler: SwitchMenu) {
    SWITCH_MENU.with(|s| *s.borrow_mut() = Some(handler));
}

/// A key while a menu is open; true when the menu used it.
fn handle_key(keystroke: &Keystroke, cx: &mut App) -> bool {
    let Some((depth, entries, highlighted, source)) = cx
        .try_global::<Menus>()
        .and_then(|m| m.session.as_ref())
        .and_then(|s| {
            let last = s.levels.last()?;
            Some((
                s.levels.len() - 1,
                last.entries.clone(),
                last.highlighted,
                s.source,
            ))
        })
    else {
        return false;
    };
    let m = &keystroke.modifiers;
    if m.control || m.platform || m.function {
        // shortcuts close the menu and go on to the window
        close_all(cx);
        return false;
    }
    if let Some(session) = cx.global_mut::<Menus>().session.as_mut() {
        session.keyboard = true;
    }
    match keystroke.key.as_str() {
        "down" => set_highlight(depth, step(&entries, highlighted, true), cx),
        "up" => set_highlight(depth, step(&entries, highlighted, false), cx),
        "home" => set_highlight(depth, step(&entries, None, true), cx),
        "end" => set_highlight(depth, step(&entries, None, false), cx),
        "right" => match highlighted.map(|i| &entries[i].kind) {
            Some(EntryKind::Submenu(_)) if highlighted.is_some_and(|i| entries[i].enabled) => {
                if let Some(row) = highlighted {
                    open_submenu(depth, row, true, cx);
                }
            }
            _ => {
                if let Source::MenuBar(_) = source {
                    switch_menu(1, cx);
                }
            }
        },
        "left" => {
            if depth > 0 {
                close_after(depth - 1, cx);
            } else if let Source::MenuBar(_) = source {
                switch_menu(-1, cx);
            }
        }
        "enter" | "space" => {
            if let Some(row) = highlighted {
                activate(depth, row, true, cx);
            }
        }
        "escape" => {
            if depth > 0 {
                close_after(depth - 1, cx);
            } else {
                close_all(cx);
            }
        }
        "tab" => {}
        _ => {
            // mnemonics (Chromium: a unique match activates, several cycle)
            let typed = keystroke
                .key_char
                .as_deref()
                .unwrap_or(&keystroke.key)
                .chars()
                .next()
                .map(|c| c.to_ascii_lowercase());
            let Some(typed) = typed.filter(|c| c.is_alphanumeric()) else {
                return true;
            };
            let matches: Vec<usize> = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.selectable() && mnemonic_char(&e.text, e.mnemonic) == Some(typed)
                })
                .map(|(i, _)| i)
                .collect();
            match matches.as_slice() {
                [] => {}
                [only] => activate(depth, *only, true, cx),
                several => {
                    let next = several
                        .iter()
                        .copied()
                        .find(|&i| highlighted.is_none_or(|h| i > h))
                        .unwrap_or(several[0]);
                    set_highlight(depth, Some(next), cx);
                }
            }
        }
    }
    true
}

fn switch_menu(delta: isize, cx: &mut App) {
    let handler = SWITCH_MENU.with(|s| s.borrow().clone());
    if let Some(handler) = handler {
        // the menu bar opens the next menu in its window, which the key's
        // dispatch is still updating
        cx.defer(move |cx| handler(delta, cx));
    }
}

/// The owner window saw a mouse press outside every menu (the menu bar
/// handles its own buttons first).
pub fn dismiss_on_outside_click(cx: &mut App) -> bool {
    if is_open(cx) {
        close_all(cx);
        true
    } else {
        false
    }
}

/// One level's window.
pub struct MenuLevelView {
    level: usize,
}

impl MenuLevelView {
    fn row_at(&self, y: f32, cx: &App) -> Option<usize> {
        let session = cx.try_global::<Menus>()?.session.as_ref()?;
        let level = session.levels.get(self.level)?;
        level
            .row_tops
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, top)| {
                let height = if level.entries[i].is_separator() {
                    metrics::SEPARATOR_HEIGHT
                } else {
                    metrics::ITEM_HEIGHT
                };
                (y >= *top && y < top + height).then_some(i)
            })
    }

    fn hover(&mut self, y: f32, cx: &mut Context<Self>) {
        let level = self.level;
        let row = self.row_at(y, cx);
        let entry = row.and_then(|r| {
            cx.try_global::<Menus>()?
                .session
                .as_ref()?
                .levels
                .get(level)?
                .entries
                .get(r)
                .cloned()
        });
        let selectable = entry.as_ref().is_some_and(Entry::selectable);
        let current = cx
            .try_global::<Menus>()
            .and_then(|m| m.session.as_ref())
            .and_then(|s| s.levels.get(level))
            .and_then(|l| l.highlighted);
        let target = if selectable { row } else { None };
        if target == current {
            return;
        }
        set_highlight(level, target, cx);
        // a submenu opens after `show_delay`; hovering elsewhere closes the
        // open one after the same delay
        let delay = Duration::from_millis(metrics::SUBMENU_DELAY_MS);
        let is_submenu = entry
            .as_ref()
            .is_some_and(|e| selectable && matches!(e.kind, EntryKind::Submenu(_)));
        let task = cx.spawn(async move |_, cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|cx| {
                if is_submenu {
                    if let Some(row) = target {
                        open_submenu(level, row, false, cx);
                    }
                } else {
                    close_after(level, cx);
                }
            });
        });
        if let Some(session) = cx.global_mut::<Menus>().session.as_mut() {
            session.submenu_timer = Some(task);
        }
    }
}

impl Render for MenuLevelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use metrics::*;
        let palette = Palette::for_window(window);
        let Some((entries, highlighted)) = cx
            .try_global::<Menus>()
            .and_then(|m| m.session.as_ref())
            .and_then(|s| {
                let l = s.levels.get(self.level)?;
                Some((l.entries.clone(), l.highlighted))
            })
        else {
            return div().id("views-menu");
        };
        let has_check = entries.iter().any(|e| e.checked.is_some());
        let label_start = LABEL_START + if has_check { CHECK_COLUMN } else { 0. };
        let rows = entries.into_iter().enumerate().map(move |(ix, entry)| {
            if entry.is_separator() {
                return div()
                    .h(px(SEPARATOR_HEIGHT))
                    .w_full()
                    .flex_none()
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .top(px(SEPARATOR_RULE))
                            .left_0()
                            .w_full()
                            .h(px(1.))
                            .bg(palette.separator),
                    )
                    .into_any_element();
            }
            let hot = highlighted == Some(ix);
            let (text, minor) = if !entry.enabled {
                (palette.disabled, palette.disabled)
            } else if hot {
                (palette.highlight_text, palette.highlight_text)
            } else {
                (palette.text, palette.accelerator)
            };
            // Chromium's Linux menus show no mnemonic underlines, even when
            // opened from the keyboard (measured in GitHub Desktop)
            let label_element = div().child(entry.text.clone());
            div()
                .h(px(ITEM_HEIGHT))
                .w_full()
                .flex_none()
                .relative()
                .flex()
                .items_center()
                .when(hot, |d| d.bg(palette.highlight))
                .text_color(text)
                .when_some(entry.checked.filter(|c| *c), |d, _| {
                    d.child(
                        div()
                            .absolute()
                            .left(px(LABEL_START))
                            .top(px(-TEXT_RAISE))
                            .h(px(ITEM_HEIGHT))
                            .flex()
                            .items_center()
                            .child(
                                svg()
                                    .path("octicons/check-16.svg")
                                    .w(px(16.))
                                    .h(px(16.))
                                    .text_color(text),
                            ),
                    )
                })
                .child(
                    div()
                        .absolute()
                        .left(px(label_start))
                        .top(px(-TEXT_RAISE))
                        .h(px(ITEM_HEIGHT))
                        .flex()
                        .items_center()
                        .child(label_element),
                )
                .when_some(entry.accelerator.clone(), |d, accelerator| {
                    let arrow = matches!(entry.kind, EntryKind::Submenu(_));
                    d.child(
                        div()
                            .absolute()
                            .right(px(ACCELERATOR_RIGHT + if arrow { ARROW_COLUMN } else { 0. }))
                            .top(px(-TEXT_RAISE))
                            .h(px(ITEM_HEIGHT))
                            .flex()
                            .items_center()
                            .text_color(minor)
                            .child(accelerator),
                    )
                })
                .when(matches!(entry.kind, EntryKind::Submenu(_)), |d| {
                    d.child(
                        div()
                            .absolute()
                            .right(px(8.))
                            .top(px(-TEXT_RAISE))
                            .h(px(ITEM_HEIGHT))
                            .flex()
                            .items_center()
                            .child(
                                svg()
                                    .path("octicons/chevron-right-16.svg")
                                    .w(px(16.))
                                    .h(px(16.))
                                    .text_color(text),
                            ),
                    )
                })
                .into_any_element()
        });
        div()
            .id("views-menu")
            .size_full()
            .pt(px(VERTICAL_INSET))
            .flex()
            .flex_col()
            .bg(palette.background)
            .font_family(font_family())
            .text_size(px(FONT_SIZE))
            .line_height(px(20.))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.hover(f32::from(event.position.y), cx);
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                // leaving the menu drops the highlight unless a submenu is open
                if !*hovered {
                    let level = this.level;
                    let deeper = cx
                        .try_global::<Menus>()
                        .and_then(|m| m.session.as_ref())
                        .is_some_and(|s| s.levels.len() > level + 1);
                    if !deeper {
                        set_highlight(level, None, cx);
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _, cx| {
                    if let Some(row) = this.row_at(f32::from(event.position.y), cx) {
                        activate(this.level, row, false, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, event: &MouseUpEvent, _, cx| {
                    if let Some(row) = this.row_at(f32::from(event.position.y), cx) {
                        activate(this.level, row, false, cx);
                    }
                }),
            )
            .children(rows)
    }
}

/// `text` with the character at byte `at` underlined (Chromium's
/// mnemonic underline: the text colour, 1 px, two pixels below the
/// baseline).
pub fn mnemonic_text(text: &str, at: usize, color: Hsla) -> impl IntoElement {
    let end = text[at..]
        .char_indices()
        .nth(1)
        .map_or(text.len(), |(i, _)| at + i);
    StyledText::new(text.to_string()).with_highlights([(
        at..end,
        HighlightStyle {
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(color),
                wavy: false,
            }),
            ..Default::default()
        },
    )])
}

/// Format a GPUI keystroke the way Chromium labels accelerators on Linux
/// (`Accelerator::GetShortcutText`): `Alt+Ctrl+Shift+Key`, keys by their
/// Chromium names (`Comma`, `Backspace`, `F11`).
pub fn accelerator_text(keystroke: &Keystroke) -> String {
    let m = &keystroke.modifiers;
    let mut out = String::new();
    if m.alt {
        out.push_str("Alt+");
    }
    if m.control {
        out.push_str("Ctrl+");
    }
    if m.platform {
        out.push_str("Super+");
    }
    if m.shift {
        out.push_str("Shift+");
    }
    // GPUI names a shifted punctuation key by the character it types;
    // Chromium names the key
    let (shifted, key) = match keystroke.key.as_str() {
        "<" => (true, ","),
        ">" => (true, "."),
        "{" => (true, "["),
        "}" => (true, "]"),
        "?" => (true, "/"),
        "+" => (true, "="),
        "_" => (true, "-"),
        other => (false, other),
    };
    if shifted && !m.shift {
        out.push_str("Shift+");
    }
    let name = match key {
        "," => "Comma".to_string(),
        "." => "Period".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "enter" => "Enter".to_string(),
        "escape" => "Esc".to_string(),
        "space" => "Space".to_string(),
        "tab" => "Tab".to_string(),
        "up" => "Up".to_string(),
        "down" => "Down".to_string(),
        "left" => "Left".to_string(),
        "right" => "Right".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "Page Up".to_string(),
        "pagedown" => "Page Down".to_string(),
        k if k.len() > 1 && k.starts_with('f') && k[1..].chars().all(|c| c.is_ascii_digit()) => {
            k.to_uppercase()
        }
        k => k.to_uppercase(),
    };
    out.push_str(&name);
    out
}

/// A context menu as Electron pops it on Linux: at `position` in `window`.
pub fn show_context_menu(
    items: &[MenuItem],
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let entries = items.iter().map(Entry::from_menu_item).collect();
    open(
        entries,
        Anchor::point(position),
        Source::Context,
        false,
        None,
        window,
        cx,
    );
}

/// The open context or menu bar menu's rows, for the parity harness.
pub fn pick(label: &str, cx: &mut App) -> bool {
    let found = cx
        .try_global::<Menus>()
        .and_then(|m| m.session.as_ref())
        .and_then(|s| {
            s.levels.iter().enumerate().find_map(|(li, l)| {
                l.entries
                    .iter()
                    .position(|e| e.text.as_ref() == label && e.selectable())
                    .map(|row| (li, row))
            })
        });
    match found {
        Some((level, row)) => {
            activate(level, row, false, cx);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    // not `super::*`: gpui's `test` attribute would shadow the standard one
    use super::{Entry, EntryKind, accelerator_text, mnemonic_char, parse_mnemonic, step};
    use gpui_kit::Keystroke;
    use std::rc::Rc;

    #[test]
    fn parses_mnemonics() {
        assert_eq!(parse_mnemonic("&File"), ("File".into(), Some(0)));
        assert_eq!(
            parse_mnemonic("Clo&ne repository…"),
            ("Clone repository…".into(), Some(3))
        );
        assert_eq!(
            parse_mnemonic("Fish && chips"),
            ("Fish & chips".into(), None)
        );
        assert_eq!(parse_mnemonic("Plain"), ("Plain".into(), None));
        assert_eq!(mnemonic_char("Clone", Some(3)), Some('n'));
    }

    #[test]
    fn formats_accelerators_like_chromium() {
        let k = |s: &str| accelerator_text(&Keystroke::parse(s).unwrap());
        assert_eq!(k("ctrl-,"), "Ctrl+Comma");
        assert_eq!(k("ctrl-shift-backspace"), "Ctrl+Shift+Backspace");
        assert_eq!(k("ctrl-alt-shift-a"), "Alt+Ctrl+Shift+A");
        assert_eq!(k("ctrl-alt-w"), "Alt+Ctrl+W");
        assert_eq!(k("f11"), "F11");
        assert_eq!(k("ctrl-`"), "Ctrl+`");
        assert_eq!(k("ctrl-="), "Ctrl+=");
        assert_eq!(k("ctrl--"), "Ctrl+-");
        assert_eq!(k("ctrl-<"), "Ctrl+Shift+Comma");
    }

    fn entry(enabled: bool) -> Entry {
        Entry {
            text: "x".into(),
            mnemonic: None,
            accelerator: None,
            enabled,
            checked: None,
            kind: EntryKind::Action(Rc::new(|_, _| {})),
        }
    }

    #[test]
    fn keyboard_steps_skip_disabled_rows_and_wrap() {
        let entries = vec![entry(false), entry(true), Entry::separator(), entry(true)];
        assert_eq!(step(&entries, None, true), Some(1));
        assert_eq!(step(&entries, Some(1), true), Some(3));
        assert_eq!(step(&entries, Some(3), true), Some(1));
        assert_eq!(step(&entries, Some(1), false), Some(3));
        assert_eq!(step(&entries, None, false), Some(3));
    }
}
