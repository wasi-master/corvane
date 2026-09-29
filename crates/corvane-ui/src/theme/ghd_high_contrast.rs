//! High contrast theme (Corvane addition; GitHub Desktop 3.6.6 has no such
//! theme): GHD's dark tokens (`ghd_dark`) recoloured with Primer's
//! `dark_high_contrast` palette (`@primer/primitives` 7.x, see
//! `.docs/ghd-theme-tokens.md` › High contrast). Surfaces go to
//! near-black, borders and secondary text to Primer's high-contrast grays,
//! and emphasis colours to the brighter high-contrast scale, with dark text
//! on the bright selection and primary-button fills.

use super::{Appearance, GhdTheme, c, ca};

// Primer `dark_high_contrast` (functional tokens and scale steps).
const CANVAS_DEFAULT: u32 = 0x0a0c10;
const CANVAS_INSET: u32 = 0x010409;
const CANVAS_SUBTLE: u32 = 0x272b33;
const FG_DEFAULT: u32 = 0xf0f3f6;
const FG_SUBTLE: u32 = 0x9ea7b3;
const FG_ON_EMPHASIS: u32 = 0x0a0c10;
const GRAY_2: u32 = 0xd9dee3;
const GRAY_3: u32 = 0xbdc4cc;
const GRAY_6: u32 = 0x525964;
const BORDER_DEFAULT: u32 = 0x7a828e;
const ACCENT_FG: u32 = 0x71b7ff;
const ACCENT_EMPHASIS: u32 = 0x409eff;
const BLUE_2: u32 = 0x91cbff;
const BLUE_1: u32 = 0xaddcff;
const SUCCESS_FG: u32 = 0x26cd4d;
const SUCCESS_EMPHASIS: u32 = 0x09b43a;
const GREEN_1: u32 = 0x72f088;
const ATTENTION_FG: u32 = 0xf0b72f;
const SEVERE_FG: u32 = 0xe7811d;
const DANGER_FG: u32 = 0xff6a69;
const RED_3: u32 = 0xff9492;
const PURPLE_2: u32 = 0xdbb7ff;
const ORANGE_2: u32 = 0xffb757;

pub fn theme() -> GhdTheme {
    let mut t = super::ghd_dark::theme();
    t.name = "High Contrast";
    t.appearance = Appearance::Dark;

    // text + surfaces
    t.text = c(FG_DEFAULT);
    t.text_secondary = c(GRAY_2);
    t.text_secondary_muted = c(FG_SUBTLE);
    t.background = c(CANVAS_DEFAULT);
    t.box_background = c(CANVAS_DEFAULT);
    t.box_alt_background = c(CANVAS_SUBTLE);
    t.box_border = c(BORDER_DEFAULT);
    t.box_border_contrast = c(BORDER_DEFAULT);
    t.box_border_accent = c(ACCENT_EMPHASIS);
    t.box_selected_background = c(GRAY_6);
    t.box_selected_text = c(FG_DEFAULT);
    t.box_selected_active_background = c(ACCENT_EMPHASIS);
    t.box_selected_active_text = c(FG_ON_EMPHASIS);
    t.box_hover_background = c(CANVAS_SUBTLE);
    t.box_hover_text = c(FG_DEFAULT);
    t.box_placeholder = c(FG_SUBTLE);
    t.shadow = ca(CANVAS_INSET, 0.8);

    // buttons + links
    t.button_background = c(ACCENT_EMPHASIS);
    t.button_hover_background = c(ACCENT_FG);
    t.button_text = c(FG_ON_EMPHASIS);
    t.button_focus_border = c(FG_DEFAULT);
    t.secondary_button_background = c(CANVAS_SUBTLE);
    t.secondary_button_border = c(BORDER_DEFAULT);
    t.secondary_button_hover_background = c(GRAY_6);
    t.secondary_button_hover_border = c(GRAY_3);
    t.secondary_button_text = c(FG_DEFAULT);
    t.link = c(ACCENT_FG);
    t.link_hover = c(BLUE_2);

    // toolbar
    t.toolbar_background = c(CANVAS_INSET);
    t.toolbar_border = c(BORDER_DEFAULT);
    t.toolbar_text = c(FG_DEFAULT);
    t.toolbar_text_secondary = c(GRAY_2);
    t.toolbar_button_border = c(BORDER_DEFAULT);
    t.toolbar_button_hover_background = c(CANVAS_SUBTLE);
    t.toolbar_button_hover_text = c(FG_DEFAULT);
    t.toolbar_button_active_background = c(CANVAS_DEFAULT);
    t.toolbar_button_active_text = c(FG_DEFAULT);
    t.toolbar_button_progress = ca(ACCENT_EMPHASIS, 0.4);
    t.toolbar_badge_background = c(GRAY_6);
    t.toolbar_badge_active_background = c(GRAY_6);
    t.toolbar_dropdown_text_warning = c(ATTENTION_FG);

    // tab bar + lists
    t.tab_bar_active = c(ACCENT_EMPHASIS);
    t.tab_bar_background = c(CANVAS_DEFAULT);
    t.tab_bar_hover_background = c(CANVAS_SUBTLE);
    t.tab_bar_count_text = c(FG_DEFAULT);
    t.tab_bar_count_background = c(GRAY_6);
    t.pr_open_icon = c(SUCCESS_FG);
    t.pr_draft_icon = c(FG_SUBTLE);
    t.list_item_hover_background = c(CANVAS_SUBTLE);
    t.list_item_badge_text = c(FG_DEFAULT);
    t.list_item_badge_background = c(GRAY_6);
    t.list_item_selected_badge_text = c(FG_DEFAULT);
    t.list_item_selected_badge_background = c(CANVAS_SUBTLE);
    t.list_item_selected_active_badge_text = c(FG_ON_EMPHASIS);
    t.list_item_selected_active_badge_background = c(FG_DEFAULT);
    t.scroll_bar_thumb = ca(FG_SUBTLE, 0.7);
    t.scroll_bar_thumb_active = c(GRAY_3);

    // focus + accents
    t.focus = c(ACCENT_EMPHASIS);
    t.accent = c(ACCENT_EMPHASIS);
    t.text_field_focus_shadow = ca(ACCENT_EMPHASIS, 0.5);
    t.primary_suggested_action_background = ca(ACCENT_EMPHASIS, 0.15);
    t.primary_suggested_action_border = c(ACCENT_EMPHASIS);
    t.suggested_action_icon = c(FG_SUBTLE);
    t.control_background = c(CANVAS_DEFAULT);
    t.control_border = c(GRAY_3);

    // file status
    t.color_new = c(SUCCESS_FG);
    t.color_deleted = c(DANGER_FG);
    t.color_modified = c(ATTENTION_FG);
    t.color_renamed = c(ACCENT_FG);
    t.color_conflicted = c(SEVERE_FG);

    // commit form
    t.co_author_tag_background = c(CANVAS_SUBTLE);
    t.co_author_tag_border = c(BORDER_DEFAULT);
    t.input_icon_warning = c(ATTENTION_FG);
    t.input_icon_error = c(DANGER_FG);

    // dialogs + status
    t.error = c(DANGER_FG);
    t.form_error_background = ca(DANGER_FG, 0.1);
    t.form_error_border = c(DANGER_FG);
    t.form_error_text = c(FG_DEFAULT);
    t.dialog_warning = c(ATTENTION_FG);
    t.dialog_information = c(ACCENT_FG);
    t.dialog_error = c(DANGER_FG);
    t.dialog_banner_success_background = ca(SUCCESS_EMPHASIS, 0.15);
    t.dialog_banner_success_border = c(SUCCESS_EMPHASIS);
    t.dialog_banner_success_text = c(FG_DEFAULT);
    t.status_pending = c(ATTENTION_FG);
    t.status_error = c(DANGER_FG);
    t.status_success = c(SUCCESS_FG);
    t.tooltip_background = c(FG_DEFAULT);
    t.tooltip_text = c(CANVAS_DEFAULT);

    // context menus
    t.menu_background = c(CANVAS_SUBTLE);
    t.menu_border = c(BORDER_DEFAULT);
    t.menu_text = c(FG_DEFAULT);
    t.menu_text_disabled = c(FG_SUBTLE);
    t.menu_highlight = c(ACCENT_EMPHASIS);
    t.menu_highlight_text = c(FG_ON_EMPHASIS);

    // syntax (`prettylights`)
    t.syntax_variable = c(ORANGE_2);
    t.syntax_alt_variable = c(ORANGE_2);
    t.syntax_keyword = c(RED_3);
    t.syntax_atom = c(BLUE_2);
    t.syntax_string = c(BLUE_1);
    t.syntax_qualifier = c(PURPLE_2);
    t.syntax_type = c(PURPLE_2);
    t.syntax_comment = c(GRAY_3);
    t.syntax_tag = c(GREEN_1);
    t.syntax_attribute = c(BLUE_2);
    t.syntax_link = c(BLUE_1);
    t.syntax_header = c(ACCENT_EMPHASIS);
    t.syntax_quote = c(GREEN_1);

    // diff (`diffBlob`)
    t.diff_text = c(FG_DEFAULT);
    t.diff_alt_text = c(GRAY_2);
    t.diff_border = c(BORDER_DEFAULT);
    t.diff_gutter = c(BORDER_DEFAULT);
    t.diff_gutter_background = c(CANVAS_DEFAULT);
    t.diff_line_number = c(FG_DEFAULT);
    t.diff_selected_background = c(ACCENT_EMPHASIS);
    t.diff_selected_border = c(ACCENT_EMPHASIS);
    t.diff_selected_gutter = c(ACCENT_EMPHASIS);
    t.diff_selected_text = c(FG_ON_EMPHASIS);
    t.diff_add_background = ca(SUCCESS_EMPHASIS, 0.15);
    t.diff_add_border = ca(SUCCESS_FG, 0.3);
    t.diff_add_gutter = ca(SUCCESS_FG, 0.3);
    t.diff_add_gutter_background = ca(SUCCESS_FG, 0.3);
    t.diff_add_inner_background = ca(SUCCESS_EMPHASIS, 0.5);
    t.diff_add_text = c(FG_DEFAULT);
    t.diff_delete_background = ca(DANGER_FG, 0.1);
    t.diff_delete_border = ca(DANGER_FG, 0.3);
    t.diff_delete_gutter = ca(DANGER_FG, 0.3);
    t.diff_delete_gutter_background = ca(DANGER_FG, 0.3);
    t.diff_delete_inner_background = ca(DANGER_FG, 0.45);
    t.diff_delete_text = c(FG_DEFAULT);
    t.diff_hunk_background = ca(ACCENT_EMPHASIS, 0.1);
    t.diff_hunk_border = ca(ACCENT_EMPHASIS, 0.4);
    t.diff_hunk_gutter = ca(ACCENT_EMPHASIS, 0.4);
    t.diff_hunk_gutter_background = ca(ACCENT_EMPHASIS, 0.4);
    t.diff_hunk_text = c(FG_DEFAULT);
    t.diff_hover_background = ca(ACCENT_EMPHASIS, 0.4);
    t.diff_hover_border = c(ACCENT_EMPHASIS);
    t.diff_hover_gutter = c(ACCENT_EMPHASIS);
    t.diff_hover_text = c(FG_DEFAULT);
    t.diff_add_hover_background = ca(SUCCESS_EMPHASIS, 0.5);
    t.diff_add_hover_border = c(SUCCESS_EMPHASIS);
    t.diff_add_hover_gutter = c(SUCCESS_EMPHASIS);
    t.diff_add_hover_text = c(FG_DEFAULT);
    t.diff_delete_hover_background = ca(DANGER_FG, 0.45);
    t.diff_delete_hover_border = c(DANGER_FG);
    t.diff_delete_hover_gutter = c(DANGER_FG);
    t.diff_delete_hover_text = c(FG_DEFAULT);
    t.diff_empty_row_background = c(CANVAS_SUBTLE);
    t.diff_empty_row_gutter_background = c(CANVAS_SUBTLE);
    t.file_warning = c(ATTENTION_FG);
    t.file_warning_border = c(ATTENTION_FG);
    t.file_warning_background = ca(ATTENTION_FG, 0.15);

    // markdown
    t.md_border_default = c(BORDER_DEFAULT);
    t.md_border_muted = c(BORDER_DEFAULT);
    t.md_canvas_subtle = c(CANVAS_SUBTLE);
    t.md_fg_muted = c(GRAY_2);
    t.md_neutral_muted = ca(FG_SUBTLE, 0.4);
    t.md_accent_fg = c(ACCENT_FG);

    // pull request reviews
    t.pr_timeline_line = c(BORDER_DEFAULT);
    t.pr_changes_requested_icon = c(FG_ON_EMPHASIS);
    t.pr_changes_requested_icon_background = c(DANGER_FG);
    t.pr_approved_icon = c(FG_ON_EMPHASIS);
    t.pr_approved_icon_background = c(SUCCESS_FG);
    t.pr_commented_icon = c(FG_ON_EMPHASIS);
    t.pr_commented_icon_background = c(FG_SUBTLE);
    t
}
