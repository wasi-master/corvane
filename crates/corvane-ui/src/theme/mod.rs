//! GitHub Desktop's theme tokens as a GPUI global, plus sizes.
//!
//! Token names follow `app/styles/_variables.scss` / `themes/_dark.scss` with
//! the `--` prefix and `-color` suffix dropped. See
//! `.docs/ghd-theme-tokens.md`.

mod ghd_dark;
mod ghd_light;
pub mod primer;

use gpui_kit::component::theme::{Theme as KitTheme, ThemeMode};
use gpui_kit::{App, Global, Hsla, Pixels, Rgba, px, rgb};

/// Which built-in theme is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// Sizes that are the same in every theme (`_variables.scss`).
pub mod sizes {
    use gpui_kit::{Pixels, px};

    pub const TITLE_BAR_HEIGHT: Pixels = px(32.);
    pub const TOOLBAR_HEIGHT: Pixels = px(50.);
    pub const TOOLBAR_BUTTON_HEIGHT: Pixels = px(49.);
    pub const TOOLBAR_BUTTON_WIDTH: Pixels = px(230.);
    pub const TOOLBAR_ARROW_WIDTH: Pixels = px(39.);
    pub const TAB_BAR_HEIGHT: Pixels = px(29.);
    pub const ROW_HEIGHT: Pixels = px(29.);
    pub const SIDEBAR_DEFAULT_WIDTH: Pixels = px(250.);
    pub const SIDEBAR_MIN_WIDTH: Pixels = px(220.);
    pub const RESIZE_HANDLE_WIDTH: Pixels = px(6.);
    pub const BUTTON_HEIGHT: Pixels = px(25.);
    pub const TEXT_FIELD_HEIGHT: Pixels = px(25.);
    pub const BORDER_RADIUS: Pixels = px(6.);
    pub const OUTLINED_BORDER_RADIUS: Pixels = px(3.);
    pub const SPACING: Pixels = px(10.);
    pub const SPACING_HALF: Pixels = px(5.);
    pub const SPACING_THIRD: Pixels = px(3.33);
    pub const SPACING_DOUBLE: Pixels = px(20.);
    pub const FONT_SIZE: Pixels = px(12.);
    pub const FONT_SIZE_XS: Pixels = px(9.);
    pub const FONT_SIZE_SM: Pixels = px(11.);
    pub const FONT_SIZE_MD: Pixels = px(14.);
    pub const FONT_SIZE_LG: Pixels = px(28.);
    pub const AVATAR_SIZE: Pixels = px(25.);
    pub const ICON_SIZE: Pixels = px(16.);
    pub const CHECKBOX_SIZE: Pixels = px(13.);
    pub const DIFF_LINE_NUMBER_WIDTH: Pixels = px(50.);
}

pub const UI_FONT: &str = ".SystemUIFont";
/// The macOS system font by its real family name. gpui-kit only enumerates
/// every installed font (≈1.5 s) when its theme font is left at the
/// `.SystemUIFont` sentinel, so the kit theme is seeded with this instead.
pub const KIT_UI_FONT: &str = ".AppleSystemUIFont";
/// GHD asks for SF Mono first, then Menlo. The system monospace family name
/// also differs from gpui-base's default ("Menlo"), which keeps the kit from
/// enumerating installed fonts at startup.
pub const MONO_FONT: &str = ".AppleSystemUIFontMonospaced";

/// GitHub Desktop colour tokens.
#[derive(Clone, Debug)]
pub struct GhdTheme {
    pub name: &'static str,
    pub appearance: Appearance,

    // Text + surfaces
    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_secondary_muted: Hsla,
    pub background: Hsla,
    pub box_background: Hsla,
    pub box_alt_background: Hsla,
    pub box_border: Hsla,
    pub box_border_contrast: Hsla,
    pub box_border_accent: Hsla,
    pub box_selected_background: Hsla,
    pub box_selected_text: Hsla,
    pub box_selected_active_background: Hsla,
    pub box_selected_active_text: Hsla,
    pub box_hover_background: Hsla,
    pub box_hover_text: Hsla,
    pub box_placeholder: Hsla,
    pub shadow: Hsla,
    pub overlay: Hsla,

    // Buttons + links
    pub button_background: Hsla,
    pub button_hover_background: Hsla,
    pub button_text: Hsla,
    pub button_focus_border: Hsla,
    pub secondary_button_background: Hsla,
    pub secondary_button_border: Hsla,
    pub secondary_button_hover_background: Hsla,
    pub secondary_button_hover_border: Hsla,
    pub secondary_button_text: Hsla,
    pub link: Hsla,
    pub link_hover: Hsla,

    // Toolbar
    pub toolbar_background: Hsla,
    pub toolbar_border: Hsla,
    pub toolbar_text: Hsla,
    pub toolbar_text_secondary: Hsla,
    pub toolbar_button_border: Hsla,
    pub toolbar_button_hover_background: Hsla,
    pub toolbar_button_hover_text: Hsla,
    pub toolbar_button_active_background: Hsla,
    pub toolbar_button_active_text: Hsla,
    pub toolbar_button_progress: Hsla,
    pub toolbar_badge_background: Hsla,
    pub toolbar_badge_active_background: Hsla,
    pub toolbar_dropdown_text_warning: Hsla,

    // Tab bar + lists
    pub tab_bar_active: Hsla,
    pub tab_bar_background: Hsla,
    pub tab_bar_hover_background: Hsla,
    pub tab_bar_count_text: Hsla,
    pub tab_bar_count_background: Hsla,
    pub list_item_hover_background: Hsla,
    pub list_item_badge_text: Hsla,
    pub list_item_badge_background: Hsla,
    pub scroll_bar_thumb: Hsla,
    pub scroll_bar_thumb_active: Hsla,

    // Focus + accents
    pub focus: Hsla,
    pub accent: Hsla,
    pub text_field_focus_shadow: Hsla,
    pub primary_suggested_action_background: Hsla,
    pub primary_suggested_action_border: Hsla,
    pub suggested_action_icon: Hsla,

    // File status colours
    pub color_new: Hsla,
    pub color_deleted: Hsla,
    pub color_modified: Hsla,
    pub color_renamed: Hsla,
    pub color_conflicted: Hsla,

    // Commit form
    pub co_author_tag_background: Hsla,
    pub co_author_tag_border: Hsla,
    pub input_icon_warning: Hsla,
    pub input_icon_error: Hsla,
    pub input_icon_hover_background: Hsla,

    // Dialogs + status
    pub error: Hsla,
    pub form_error_background: Hsla,
    pub form_error_border: Hsla,
    pub form_error_text: Hsla,
    pub dialog_warning: Hsla,
    pub dialog_information: Hsla,
    pub dialog_error: Hsla,
    pub dialog_banner_success_background: Hsla,
    pub dialog_banner_success_border: Hsla,
    pub dialog_banner_success_text: Hsla,
    pub status_pending: Hsla,
    pub status_error: Hsla,
    pub status_success: Hsla,
    pub tooltip_background: Hsla,
    pub tooltip_text: Hsla,

    // Diff
    pub diff_text: Hsla,
    pub diff_alt_text: Hsla,
    pub diff_border: Hsla,
    pub diff_gutter: Hsla,
    pub diff_gutter_background: Hsla,
    pub diff_line_number: Hsla,
    pub diff_selected_background: Hsla,
    pub diff_selected_border: Hsla,
    pub diff_selected_gutter: Hsla,
    pub diff_selected_text: Hsla,
    pub diff_add_background: Hsla,
    pub diff_add_border: Hsla,
    pub diff_add_gutter: Hsla,
    pub diff_add_gutter_background: Hsla,
    pub diff_add_inner_background: Hsla,
    pub diff_add_text: Hsla,
    pub diff_delete_background: Hsla,
    pub diff_delete_border: Hsla,
    pub diff_delete_gutter: Hsla,
    pub diff_delete_gutter_background: Hsla,
    pub diff_delete_inner_background: Hsla,
    pub diff_delete_text: Hsla,
    pub diff_hunk_background: Hsla,
    pub diff_hunk_border: Hsla,
    pub diff_hunk_gutter: Hsla,
    pub diff_hunk_gutter_background: Hsla,
    pub diff_hunk_text: Hsla,
    pub diff_hover_background: Hsla,
    pub diff_hover_border: Hsla,
    pub diff_hover_gutter: Hsla,
    pub diff_hover_text: Hsla,
    pub diff_add_hover_background: Hsla,
    pub diff_add_hover_border: Hsla,
    pub diff_add_hover_gutter: Hsla,
    pub diff_add_hover_text: Hsla,
    pub diff_delete_hover_background: Hsla,
    pub diff_delete_hover_border: Hsla,
    pub diff_delete_hover_gutter: Hsla,
    pub diff_delete_hover_text: Hsla,
    pub diff_empty_row_background: Hsla,
    pub diff_empty_hunk_handle: Hsla,
}

impl Global for GhdTheme {}

impl GhdTheme {
    pub fn light() -> Self {
        ghd_light::theme()
    }

    pub fn dark() -> Self {
        ghd_dark::theme()
    }

    pub fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self::light(),
            Appearance::Dark => Self::dark(),
        }
    }

    pub fn is_dark(&self) -> bool {
        self.appearance == Appearance::Dark
    }
}

/// `cx.ghd()` - read the active GitHub Desktop theme.
pub trait ActiveGhdTheme {
    fn ghd(&self) -> &GhdTheme;
}

impl ActiveGhdTheme for App {
    fn ghd(&self) -> &GhdTheme {
        self.global::<GhdTheme>()
    }
}

/// Call before `gpui_kit::init`: pre-create the kit theme with explicit font
/// families so `Theme::change` skips its installed-font enumeration.
#[allow(clippy::field_reassign_with_default)] // kit Theme has private fields
pub fn preseed_kit_theme(cx: &mut App) {
    if cx.has_global::<KitTheme>() {
        return;
    }
    let mut theme = KitTheme::default();
    theme.font_family = KIT_UI_FONT.into();
    theme.mono_font_family = MONO_FONT.into();
    cx.set_global(theme);
}

/// Install the theme global and align gpui-kit's theme (used by Input,
/// Textarea, Scrollbar, Popover…) with GHD's palette, fonts and radius.
pub fn init(cx: &mut App, appearance: Appearance) {
    let theme = GhdTheme::for_appearance(appearance);
    apply(theme, cx);
}

pub fn apply(theme: GhdTheme, cx: &mut App) {
    let mode = if theme.is_dark() {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    KitTheme::change(mode, None, cx);
    {
        let kit = KitTheme::global_mut(cx);
        kit.font_family = KIT_UI_FONT.into();
        kit.font_size = sizes::FONT_SIZE;
        kit.mono_font_family = MONO_FONT.into();
        kit.mono_font_size = sizes::FONT_SIZE;
        kit.radius = sizes::BORDER_RADIUS;
        kit.radius_lg = sizes::BORDER_RADIUS;
        kit.shadow = false;
        kit.focus_ring = false;

        let c = &mut kit.colors;
        c.background = theme.background;
        c.foreground = theme.text;
        c.border = theme.box_border_contrast;
        // gpui-kit draws the Input border with `input`; GHD's textboxish uses the contrast border.
        c.input = theme.box_border_contrast;
        c.muted = theme.box_alt_background;
        c.muted_foreground = theme.text_secondary;
        c.primary = theme.button_background;
        c.primary_hover = theme.button_hover_background;
        c.primary_active = theme.button_hover_background;
        c.primary_foreground = theme.button_text;
        c.secondary = theme.secondary_button_background;
        c.secondary_hover = theme.secondary_button_hover_background;
        c.secondary_active = theme.secondary_button_hover_background;
        c.secondary_foreground = theme.secondary_button_text;
        c.accent = theme.box_hover_background;
        c.accent_foreground = theme.box_hover_text;
        c.ring = theme.focus;
        c.caret = theme.text;
        c.selection = theme.text_field_focus_shadow;
        c.link = theme.link;
        c.link_hover = theme.link_hover;
        c.link_active = theme.link_hover;
        c.popover = theme.box_background;
        c.popover_foreground = theme.text;
        c.list = theme.background;
        c.list_hover = theme.list_item_hover_background;
        c.list_active = theme.box_selected_background;
        c.list_active_border = theme.box_selected_background;
        c.list_even = theme.background;
        c.list_head = theme.box_alt_background;
        c.scrollbar = theme.background;
        c.scrollbar_thumb = theme.scroll_bar_thumb;
        c.scrollbar_thumb_hover = theme.scroll_bar_thumb_active;
        c.sidebar = theme.background;
        c.sidebar_foreground = theme.text;
        c.sidebar_border = theme.box_border;
        c.title_bar = theme.toolbar_background;
        c.title_bar_border = theme.toolbar_border;
        c.tab_bar = theme.tab_bar_background;
        c.tab = theme.tab_bar_background;
        c.tab_active = theme.tab_bar_background;
        c.tab_foreground = theme.text;
        c.tab_active_foreground = theme.text;
        c.danger = theme.error;
        c.danger_foreground = theme.button_text;
        c.success = theme.status_success;
        c.warning = theme.dialog_warning;
        c.info = theme.dialog_information;
        c.overlay = theme.overlay;
        c.skeleton = theme.box_alt_background;
        c.drag_border = theme.focus;
        c.drop_target = theme.text_field_focus_shadow;
    }
    cx.set_global(theme);
}

/// Hex → Hsla.
pub(crate) fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Hex + alpha → Hsla.
pub(crate) fn ca(hex: u32, alpha: f32) -> Hsla {
    let rgba: Rgba = rgb(hex);
    let mut hsla: Hsla = rgba.into();
    hsla.a = alpha;
    hsla
}

/// Convenience for `px` in theme code.
#[allow(dead_code)]
pub(crate) const fn p(v: f32) -> Pixels {
    px(v)
}
