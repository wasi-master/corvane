//! Flags - a Corvane addition with no GitHub Desktop counterpart: a
//! chrome://flags-style dialog over `corvane_core::flags`. Every switchable
//! deviation from GHD is a row with a toggle, select, number or text
//! control; values apply at once (no Save), restart-required flags show a
//! Relaunch bar, presets are the base layer and overrides sit on top.
//! Opened from Corvane › Flags… (⌘⇧,), `CORVANE_POPUP=flags[:query]` and
//! `x-corvane://flags?q=`. Snapshot-verified only: the parity harness has
//! nothing to compare it with.
//!
//! Bug-fix flags (`Nature::BugFix`) are hidden from the list, the search and
//! every count unless "Show bug fixes" is ticked; the checkbox lasts for the
//! dialog session. Hiding is display-only: presets, Reset all and
//! `CORVANE_FLAGS` still cover them.

use std::collections::HashMap;
use std::rc::Rc;

use corvane_core::flags::{self, Category, FlagDef, FlagId, Kind, Nature, Preset, REGISTRY, Value};
use corvane_core::{AppState, Dispatcher};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{CloseFoldout, Find};
use crate::autocompletion::highlighted;
use crate::context_menu::MenuItem;
use crate::dialog::window_title;
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, mono_font};
use crate::widgets::{
    IconButtonA11y, ListRowA11y, SelectHandler, button, checkbox_row, counter, dialog_error_banner,
    filter_text_box, link_button, pill, primary_button, select_button, switch, text_box_opts,
};

/// The left navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Nav {
    All,
    Modified,
    Category(Category),
    Unavailable,
}

/// Where the search matched, as char positions for `highlighted`.
#[derive(Clone, Debug, Default)]
struct Hit {
    title: Vec<usize>,
    ident: Vec<usize>,
    summary: Vec<usize>,
    /// The query is an upstream issue number of this flag.
    issue: bool,
}

/// Char positions of `needle` (already lowercased) inside `hay`.
fn substring_positions(hay: &str, needle: &str) -> Option<Vec<usize>> {
    let lower = hay.to_lowercase();
    let start = lower.find(needle)?;
    let first = lower[..start].chars().count();
    Some((first..first + needle.chars().count()).collect())
}

/// Search over the title, `id-slug`, summary and upstream numbers.
fn matches(def: &FlagDef, query: &str) -> Option<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(Hit::default());
    }
    let mut hit = Hit::default();
    let mut any = false;
    if let Some(p) = substring_positions(def.title, &q) {
        hit.title = p;
        any = true;
    }
    if let Some(p) = substring_positions(&def.ident(), &q) {
        hit.ident = p;
        any = true;
    }
    if let Some(p) = substring_positions(def.summary, &q) {
        hit.summary = p;
        any = true;
    }
    let digits = q.trim_start_matches('#');
    if !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && def
            .upstream
            .iter()
            .any(|u| u.number.to_string().contains(digits))
    {
        hit.issue = true;
        any = true;
    }
    any.then_some(hit)
}

/// What a Number / Text input shows for a value.
fn display(value: &Value) -> String {
    match value {
        Value::Bool(_) | Value::Number(_) => value.render(),
        Value::Text(s) => s.to_string(),
    }
}

fn category_icon(category: Category) -> Octicon {
    match category {
        Category::Appearance => Octicon::Paintbrush,
        Category::Repository => Octicon::Repo,
        Category::GitHub => Octicon::GitPullRequest,
        Category::WindowAndMenus => Octicon::DeviceDesktop,
        Category::SettingsAndUpdates => Octicon::Gear,
        Category::Accessibility => Octicon::Accessibility,
        Category::ChangesAndDiffs => Octicon::FileDiff,
        Category::HistoryAndBranches => Octicon::History,
        Category::Experimental => Octicon::Telescope,
    }
}

pub struct FlagsDialog {
    state: Entity<AppState>,
    /// Routes ⌘F / Esc through the dialog while nothing inside has focus.
    focus_handle: FocusHandle,
    search: Entity<InputState>,
    /// One input per Number / Text flag, created up front (never in render).
    inputs: HashMap<u16, Entity<InputState>>,
    /// The value each input was last written from; a core value that differs
    /// (preset, reset, import, env) rewrites the input on the next observe.
    synced: HashMap<u16, Value>,
    /// Inputs whose text is not committable, with the message under them.
    errors: HashMap<u16, String>,
    nav: Nav,
    /// "Show bug fixes": list and count `Nature::BugFix` flags.
    show_bug_fixes: bool,
    /// "Paste JSON" outcome, shown under the toolbar.
    import_error: Option<String>,
    import_note: Option<String>,
}

impl FlagsDialog {
    pub fn new(
        state: Entity<AppState>,
        query: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search flags")
                .default_value(query.unwrap_or_default())
        });
        cx.observe(&search, |_, _, cx| cx.notify()).detach();

        let editable: Vec<(&'static FlagDef, Value)> = {
            let flags = &state.read(cx).flags;
            REGISTRY
                .iter()
                .filter(|def| matches!(def.kind, Kind::Number { .. } | Kind::Text { .. }))
                .map(|def| (def, flags.value(def.id).clone()))
                .collect()
        };
        let mut inputs = HashMap::new();
        let mut synced = HashMap::new();
        let errors = HashMap::new();
        for (def, value) in editable {
            let placeholder = match def.kind {
                Kind::Text { placeholder, .. } => placeholder,
                _ => "",
            };
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(display(&value))
            });
            let id = def.id.0;
            cx.subscribe(&input, move |this, _, ev: &InputEvent, cx| {
                this.on_input(id, ev, cx)
            })
            .detach();
            inputs.insert(id, input);
            synced.insert(id, value);
        }
        cx.observe_in(&state, window, |this, _, window, cx| {
            this.resync(window, cx);
            cx.notify();
        })
        .detach();

        let handle = search.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            state,
            focus_handle,
            search,
            inputs,
            synced,
            errors,
            nav: Nav::All,
            show_bug_fixes: false,
            import_error: None,
            import_note: None,
        }
    }

    /// A Number / Text input changed: validate, and commit when it fits.
    fn on_input(&mut self, id: u16, ev: &InputEvent, cx: &mut Context<Self>) {
        if !matches!(ev, InputEvent::Change) {
            return;
        }
        let Some(input) = self.inputs.get(&id) else {
            return;
        };
        let text = input.read(cx).value().to_string();
        let def = flags::def(FlagId(id));
        match def.kind.parse(&text) {
            Ok(value) => {
                self.errors.remove(&id);
                if self.synced.get(&id) == Some(&value) {
                    return;
                }
                self.synced.insert(id, value.clone());
                if let Err(err) = Dispatcher::set_flag(FlagId(id), value, cx) {
                    self.errors.insert(id, err);
                }
            }
            Err(err) => {
                self.errors.insert(id, err);
            }
        }
        cx.notify();
    }

    /// The app state changed: inputs whose flag changed elsewhere (preset,
    /// reset, import) show the new value.
    fn resync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current: Vec<(u16, Value)> = {
            let flags = &self.state.read(cx).flags;
            self.inputs
                .keys()
                .map(|id| (*id, flags.value(FlagId(*id)).clone()))
                .collect()
        };
        for (id, value) in current {
            if self.synced.get(&id) == Some(&value) {
                continue;
            }
            if let Some(input) = self.inputs.get(&id) {
                let text = display(&value);
                input.update(cx, |s, cx| s.set_value(text, window, cx));
            }
            self.synced.insert(id, value);
            self.errors.remove(&id);
        }
    }

    fn in_nav(def: &FlagDef, nav: Nav, flags: &flags::Flags) -> bool {
        match nav {
            Nav::All => true,
            Nav::Modified => flags.is_overridden(def.id),
            Nav::Category(category) => def.category() == category,
            Nav::Unavailable => !flags.is_available(def.id),
        }
    }

    /// The rows to list, and how many bug-fix flags would match but are
    /// hidden by "Show bug fixes".
    fn visible_rows(&self, cx: &App) -> (Vec<(&'static FlagDef, Hit)>, usize) {
        let query = self.search.read(cx).value().to_string();
        let flags = &self.state.read(cx).flags;
        let (rows, hidden): (Vec<_>, Vec<_>) = REGISTRY
            .iter()
            .filter(|def| Self::in_nav(def, self.nav, flags))
            .filter_map(|def| matches(def, &query).map(|hit| (def, hit)))
            .partition(|(def, _)| def.is_shown(self.show_bug_fixes));
        (rows, hidden.len())
    }

    fn paste_json(&mut self, cx: &mut Context<Self>) {
        self.import_error = None;
        self.import_note = None;
        match cx.read_from_clipboard().and_then(|item| item.text()) {
            None => self.import_error = Some("The clipboard has no text to import.".into()),
            Some(text) => match Dispatcher::import_flags_json(&text, cx) {
                Ok(report) if report.unknown.is_empty() => {}
                Ok(report) => {
                    self.import_note = Some(format!(
                        "Imported. Unknown flags were skipped: {}.",
                        report.unknown.join(", ")
                    ))
                }
                Err(err) => self.import_error = Some(format!("Could not import flags: {err}")),
            },
        }
        cx.notify();
    }

    fn close(cx: &mut App) {
        Dispatcher::close_popup(cx);
    }

    // ---- rendering ----

    fn nav_row(
        &self,
        nav: Nav,
        label: &'static str,
        icon: Octicon,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.ghd();
        let selected = self.nav == nav;
        let (text, icon_color, badge_bg, badge_fg) = if selected {
            (
                t.box_selected_active_text,
                t.box_selected_active_text,
                t.list_item_selected_active_badge_background,
                t.list_item_selected_active_badge_text,
            )
        } else {
            (
                t.text,
                t.text_secondary,
                t.list_item_badge_background,
                t.list_item_badge_text,
            )
        };
        div()
            .id(SharedString::from(format!("flags-nav-{label}")))
            .a11y_row(format!("{label}, {count}"), selected)
            .h(ROW_HEIGHT())
            .mx(SPACING())
            .my(zpx(1.))
            .px(SPACING())
            .rounded(BORDER_RADIUS())
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING())
            .text_size(FONT_SIZE())
            .text_color(text)
            .cursor_pointer()
            .when(selected, |d| d.bg(t.box_selected_active_background))
            .when(!selected, |d| d.hover(|d| d.bg(t.tab_bar_hover_background)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.nav = nav;
                cx.notify();
            }))
            .child(octicon(icon, icon_color).size(ICON_SIZE()))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(counter(count, cx).bg(badge_bg).text_color(badge_fg))
            .into_any_element()
    }

    fn nav_separator(&self, cx: &App) -> AnyElement {
        let t = cx.ghd();
        div()
            .h(zpx(1.))
            .mx(SPACING_DOUBLE())
            .my(SPACING_HALF())
            .bg(t.box_border)
            .into_any_element()
    }

    fn group_header(&self, category: Category, cx: &App) -> AnyElement {
        let t = cx.ghd();
        div()
            .h(ROW_HEIGHT())
            .px(SPACING_DOUBLE())
            .pt(SPACING())
            .flex()
            .items_center()
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(FONT_SIZE())
            .text_color(t.text_secondary)
            .child(format!("{} · {}", category.block(), category.title()))
            .into_any_element()
    }

    fn control(
        &self,
        def: &'static FlagDef,
        value: &Value,
        disabled: bool,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let t = cx.ghd();
        let id = def.id;
        match def.kind {
            Kind::Bool => switch(
                ("flag-switch", id.0 as usize),
                value.as_bool().unwrap_or(false),
                disabled,
                move |on, _, cx| {
                    let _ = Dispatcher::set_flag(id, Value::Bool(on), cx);
                },
                cx,
            )
            .aria_label(def.title)
            .into_any_element(),
            Kind::Select { options } => {
                let current = value.as_text().unwrap_or("");
                let selected = options.iter().position(|o| o.value == current);
                let labels: Vec<SharedString> = options
                    .iter()
                    .map(|o| {
                        if Value::text(o.value) == def.corvane {
                            format!("{} (default)", o.label).into()
                        } else {
                            o.label.into()
                        }
                    })
                    .collect();
                let shown = selected
                    .and_then(|ix| labels.get(ix).cloned())
                    .unwrap_or_else(|| current.to_string().into());
                let on_select: SelectHandler = Rc::new(move |ix, _, cx| {
                    if let Some(option) = options.get(ix) {
                        let _ = Dispatcher::set_flag(id, Value::text(option.value), cx);
                    }
                });
                div()
                    .w_full()
                    .child(select_button(
                        ("flag-select", id.0 as usize),
                        shown,
                        labels,
                        selected,
                        disabled,
                        on_select,
                        cx,
                    ))
                    .into_any_element()
            }
            Kind::Number { unit, .. } => {
                let Some(input) = self.inputs.get(&id.0) else {
                    return div().into_any_element();
                };
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(SPACING_HALF())
                    .child(div().w(zpx(120.)).child(text_box_opts(
                        ("flag-input", id.0 as usize),
                        input,
                        None,
                        disabled,
                        window,
                        cx,
                    )))
                    .when_some(unit, |d, unit| {
                        d.child(
                            div()
                                .text_size(FONT_SIZE_SM())
                                .text_color(t.text_secondary)
                                .child(unit),
                        )
                    })
                    .into_any_element()
            }
            Kind::Text { .. } => {
                let Some(input) = self.inputs.get(&id.0) else {
                    return div().into_any_element();
                };
                div()
                    .w_full()
                    .child(text_box_opts(
                        ("flag-input", id.0 as usize),
                        input,
                        None,
                        disabled,
                        window,
                        cx,
                    ))
                    .into_any_element()
            }
        }
    }

    fn flag_row(&self, def: &'static FlagDef, hit: &Hit, window: &Window, cx: &App) -> AnyElement {
        let t = cx.ghd();
        let id = def.id;
        let (value, overridden, env_locked, available) = {
            let flags = &self.state.read(cx).flags;
            (
                flags.value(id).clone(),
                flags.is_overridden(id),
                flags.is_env_locked(id),
                flags.is_available(id),
            )
        };
        let disabled = env_locked || !available;
        let unavailable_reason = match def.availability() {
            flags::Availability::Available => None,
            flags::Availability::BuiltIn(reason) => Some(reason),
        };
        let ident = def.ident();
        let mut aria = format!("{}, {ident}", def.title);
        if let Some(reason) = unavailable_reason {
            aria.push_str(&format!(", unavailable in this build: {reason}"));
        } else if env_locked {
            aria.push_str(", set by CORVANE_FLAGS");
        }

        let mut chips = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(SPACING_HALF())
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(FONT_SIZE())
                    .child(highlighted(def.title, &hit.title)),
            )
            .child(
                pill(
                    "",
                    None,
                    if hit.ident.is_empty() {
                        t.list_item_badge_background
                    } else {
                        t.box_border_accent
                    },
                    t.list_item_badge_text,
                    cx,
                )
                .font_family(mono_font())
                .child(highlighted(&ident, &hit.ident)),
            );
        if def.is_bug_fix() {
            chips = chips.child(pill(
                Nature::BugFix.label(),
                None,
                t.box_alt_background,
                t.text_secondary,
                cx,
            ));
        }
        if def.restart {
            chips = chips.child(pill(
                "Restart required",
                Some(Octicon::SyncClockwise),
                t.banner_warning_background,
                t.banner_warning_text,
                cx,
            ));
        }
        if env_locked {
            chips = chips.child(pill(
                "Set by CORVANE_FLAGS",
                Some(Octicon::Lock),
                t.box_alt_background,
                t.text_secondary,
                cx,
            ));
        }
        if unavailable_reason.is_some() {
            chips = chips.child(pill(
                "Unavailable",
                Some(Octicon::Stop),
                t.form_error_background,
                t.form_error_text,
                cx,
            ));
        }
        for (ix, upstream) in def.upstream.iter().enumerate() {
            let url = upstream.url();
            chips = chips.child(
                link_button(
                    SharedString::from(format!("flag-upstream-{}-{ix}", id.0)),
                    upstream.label(),
                    cx,
                )
                .text_size(FONT_SIZE_SM())
                .when(hit.issue, |d| d.font_weight(FontWeight::SEMIBOLD))
                .flex()
                .flex_row()
                .items_center()
                .gap(zpx(2.))
                .child(octicon(Octicon::LinkExternal, t.link).size(zpx(12.)))
                .on_click(move |_, _, cx| corvane_core::Dispatcher::open_url(&url, cx)),
            );
        }

        let text_column = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(zpx(4.))
            .when(!available, |d| d.opacity(0.7))
            .child(chips)
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .line_height(zpx(16.))
                    .text_color(t.text_secondary)
                    .child(highlighted(def.summary, &hit.summary)),
            )
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .line_height(zpx(16.))
                    .text_color(t.text_secondary_muted)
                    .italic()
                    .child(format!("GitHub Desktop: {}", def.ghd_behaviour)),
            )
            .when_some(unavailable_reason, |d, reason| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .line_height(zpx(16.))
                        .text_color(t.error)
                        .child(format!("Unavailable in this build: {reason}")),
                )
            });

        let control_column = div()
            .flex_none()
            .w(zpx(220.))
            .flex()
            .flex_col()
            .items_end()
            .gap(zpx(4.))
            .child(self.control(def, &value, disabled, window, cx))
            .when_some(self.errors.get(&id.0).cloned(), |d, message| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.error)
                        .text_right()
                        .child(message),
                )
            })
            .when(overridden && !disabled, |d| {
                d.child(
                    link_button(
                        SharedString::from(format!("flag-reset-{}", id.0)),
                        "Reset",
                        cx,
                    )
                    .text_size(FONT_SIZE_SM())
                    .on_click(move |_, _, cx| Dispatcher::reset_flag(id, cx)),
                )
            });

        div()
            .id(("flag", id.0 as usize))
            .role(Role::Group)
            .aria_label(aria)
            .relative()
            .w_full()
            .px(SPACING_DOUBLE())
            .py(SPACING())
            .gap(SPACING_DOUBLE())
            .flex()
            .flex_row()
            .items_start()
            .border_b_1()
            .border_color(t.box_border)
            .hover(|d| d.bg(t.list_item_hover_background))
            .when(overridden, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(zpx(3.))
                        .bg(t.accent),
                )
            })
            .child(text_column)
            .child(control_column)
            .into_any_element()
    }

    fn empty_state(&self, query: &str, hidden: usize, cx: &App) -> AnyElement {
        let t = cx.ghd();
        let mut message = if !query.trim().is_empty() {
            format!("No flags match “{}”.", query.trim())
        } else {
            match self.nav {
                Nav::Modified => "No flags differ from the preset.".to_string(),
                Nav::Unavailable => "Every flag is available in this build.".to_string(),
                _ => "No flags here yet.".to_string(),
            }
        };
        if hidden > 0 {
            message.push_str(&format!(
                " {hidden} bug-fix flag{} hidden: tick Show bug fixes to see {}.",
                if hidden == 1 { " is" } else { "s are" },
                if hidden == 1 { "it" } else { "them" },
            ));
        }
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(SPACING_DOUBLE())
            .text_color(t.text_secondary)
            .child(message)
            .into_any_element()
    }
}

impl Render for FlagsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // a copy: the nav rows below need `cx` mutably for their listeners
        let t = cx.ghd().clone();
        let viewport = window.viewport_size();
        let width = (viewport.width - zpx(80.)).max(zpx(720.)).min(zpx(960.));
        let height = (viewport.height - zpx(80.)).max(zpx(480.)).min(zpx(700.));

        let show_bug_fixes = self.show_bug_fixes;
        let (preset, preset_from_env, all_modified, modified, restart_pending, nav_counts) = {
            let s = self.state.read(cx);
            let flags = &s.flags;
            let count = |nav: Nav| {
                REGISTRY
                    .iter()
                    .filter(|def| def.is_shown(show_bug_fixes) && Self::in_nav(def, nav, flags))
                    .count()
            };
            let mut counts: Vec<(Nav, usize)> = vec![(Nav::All, count(Nav::All))];
            counts.push((Nav::Modified, count(Nav::Modified)));
            for category in Category::ALL {
                counts.push((Nav::Category(category), count(Nav::Category(category))));
            }
            counts.push((Nav::Unavailable, count(Nav::Unavailable)));
            (
                flags.preset(),
                flags.preset_from_env(),
                flags.modified_count(),
                flags.modified_count_shown(show_bug_fixes),
                s.flags_restart_pending(),
                counts,
            )
        };
        let count_of = |nav: Nav| {
            nav_counts
                .iter()
                .find(|(n, _)| *n == nav)
                .map(|(_, c)| *c)
                .unwrap_or(0)
        };
        let query = self.search.read(cx).value().to_string();
        let (rows, hidden_matches) = self.visible_rows(cx);
        // "Custom" and Reset all cover hidden bug fixes too; the counts do not
        let custom = all_modified > 0;
        let hidden_modified = all_modified - modified;

        // ---- header ----
        let header = div()
            .flex_none()
            .h(zpx(50.))
            .px(SPACING_DOUBLE())
            .flex()
            .flex_row()
            .items_center()
            .border_b_1()
            .border_color(t.box_border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(
                        div()
                            .text_size(FONT_SIZE_MD())
                            .line_height(zpx(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Flags"),
                    )
                    .child(
                        div()
                            .text_size(FONT_SIZE_SM())
                            .line_height(zpx(16.))
                            .text_color(t.text_secondary)
                            .truncate()
                            .child(
                                "Switches for the ways Corvane differs from GitHub Desktop. \
                                 Changes apply immediately.",
                            ),
                    ),
            )
            .child(
                div()
                    .id("flags-close")
                    .icon_button_label("Close")
                    .flex_none()
                    .size(ICON_SIZE())
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(octicon(Octicon::X, t.text_secondary))
                    .on_click(|_, _, cx| FlagsDialog::close(cx)),
            );

        // ---- toolbar ----
        let preset_labels: Vec<SharedString> =
            Preset::ALL.iter().map(|p| p.title().into()).collect();
        let preset_ix = Preset::ALL.iter().position(|p| *p == preset);
        let preset_shown: SharedString = if custom {
            format!("Custom ({})", preset.title()).into()
        } else {
            preset.title().into()
        };
        let on_preset: SelectHandler = Rc::new(move |ix, _, cx| {
            if let Some(preset) = Preset::ALL.get(ix) {
                Dispatcher::apply_preset(*preset, cx);
            }
        });
        let weak = cx.weak_entity();
        let more = button("flags-more", "", cx)
            .icon_button_label("More options")
            .px(SPACING_HALF())
            .child(octicon(Octicon::KebabHorizontal, t.secondary_button_text))
            .on_click(move |ev: &ClickEvent, window, cx| {
                let weak = weak.clone();
                let items = vec![
                    MenuItem::new("Copy as JSON", |_, cx| {
                        let json = Dispatcher::export_flags_json(cx);
                        cx.write_to_clipboard(ClipboardItem::new_string(json));
                    }),
                    MenuItem::new("Copy as CORVANE_FLAGS", |_, cx| {
                        let spec = Dispatcher::export_env_string(cx);
                        cx.write_to_clipboard(ClipboardItem::new_string(spec));
                    }),
                    MenuItem::separator(),
                    MenuItem::new("Paste JSON", move |_, cx| {
                        weak.update(cx, |this, cx| this.paste_json(cx)).ok();
                    }),
                ];
                let position = ev.mouse_position().unwrap_or_default();
                #[cfg(target_os = "macos")]
                crate::native_menu::show_context_menu(items, position, window, cx);
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = (items, position, window, cx);
                }
            });
        let toolbar = div()
            .flex_none()
            .h(zpx(45.))
            .px(SPACING_DOUBLE())
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING())
            .border_b_1()
            .border_color(t.box_border)
            .child(div().flex_1().max_w(zpx(360.)).child(filter_text_box(
                "flags-search",
                &self.search,
                Some(octicon(Octicon::Search, t.text_secondary)),
                window,
                cx,
            )))
            .child(div().flex_none().w(zpx(190.)).child(select_button(
                "flags-preset",
                preset_shown,
                preset_labels,
                preset_ix,
                preset_from_env,
                on_preset,
                cx,
            )))
            .child(
                button("flags-reset-all", "Reset all", cx)
                    .when(!custom, |d| d.opacity(0.6).cursor_default())
                    .when(custom, |d| {
                        d.on_click(|_, _, cx| Dispatcher::reset_all_flags(cx))
                    }),
            )
            .child({
                let weak = cx.weak_entity();
                div().flex_none().child(checkbox_row(
                    "flags-show-bug-fixes",
                    show_bug_fixes,
                    "Show bug fixes",
                    move |on, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.show_bug_fixes = on;
                            cx.notify();
                        })
                        .ok();
                    },
                    cx,
                ))
            })
            .child(div().flex_1())
            .when(modified > 0, |d| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child(format!("{modified} modified")),
                )
            })
            .child(more);

        // ---- navigation ----
        let mut nav = div()
            .id("flags-nav")
            .flex_none()
            .w(zpx(250.))
            .py(SPACING())
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.box_border)
            .role(Role::List)
            .aria_label("Flag categories")
            .child(self.nav_row(
                Nav::All,
                "All",
                Octicon::ListUnordered,
                count_of(Nav::All),
                cx,
            ))
            .child(self.nav_row(
                Nav::Modified,
                "Modified",
                Octicon::Pencil,
                count_of(Nav::Modified),
                cx,
            ))
            .child(self.nav_separator(cx));
        for category in Category::ALL {
            let count = count_of(Nav::Category(category));
            if category == Category::Experimental && count == 0 {
                continue;
            }
            nav = nav.child(self.nav_row(
                Nav::Category(category),
                category.title(),
                category_icon(category),
                count,
                cx,
            ));
        }
        if count_of(Nav::Unavailable) > 0 {
            nav = nav.child(self.nav_separator(cx)).child(self.nav_row(
                Nav::Unavailable,
                "Unavailable",
                Octicon::Stop,
                count_of(Nav::Unavailable),
                cx,
            ));
        }

        // ---- list ----
        let grouped = matches!(self.nav, Nav::All | Nav::Modified | Nav::Unavailable);
        let mut list = div()
            .id("flags-list")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .role(Role::List)
            .aria_label("Flags");
        if rows.is_empty() {
            list = list.child(self.empty_state(&query, hidden_matches, cx));
        } else {
            let mut last_category = None;
            for (def, hit) in &rows {
                if grouped && last_category != Some(def.category()) {
                    last_category = Some(def.category());
                    list = list.child(self.group_header(def.category(), cx));
                }
                list = list.child(self.flag_row(def, hit, window, cx));
            }
        }
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_row()
            .items_stretch()
            .child(nav)
            .child(list.with_scrollbar());

        // ---- restart bar ----
        let restart_bar = (!restart_pending.is_empty()).then(|| {
            let names = restart_pending
                .iter()
                .map(|id| id.display())
                .collect::<Vec<_>>()
                .join(", ");
            let message = "Your changes will take effect the next time you relaunch Corvane.";
            div()
                .id("flags-restart-bar")
                .a11y_live(message)
                .flex_none()
                .h(zpx(40.))
                .px(SPACING_DOUBLE())
                .gap(SPACING())
                .flex()
                .flex_row()
                .items_center()
                .border_t_1()
                .border_color(t.box_border)
                .bg(t.banner_warning_background)
                .text_color(t.banner_warning_text)
                .text_size(FONT_SIZE())
                .child(octicon(Octicon::Alert, t.banner_warning_icon).size(ICON_SIZE()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(format!("{message} ({names})")),
                )
                .child(
                    primary_button("flags-relaunch", "Relaunch", false, cx)
                        .min_w(zpx(100.))
                        .on_click(|_, _, cx| Dispatcher::relaunch(cx)),
                )
        });

        // ---- footer ----
        let footer = div()
            .flex_none()
            .p(SPACING_DOUBLE())
            .flex()
            .flex_row()
            .items_center()
            .border_t_1()
            .border_color(t.box_border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(if custom {
                        format!(
                            "Preset: {} · {modified} modified{}{}",
                            preset.title(),
                            if hidden_modified > 0 {
                                format!(
                                    " (and {hidden_modified} hidden bug fix{})",
                                    if hidden_modified == 1 { "" } else { "es" }
                                )
                            } else {
                                String::new()
                            },
                            if preset_from_env {
                                " · preset set by CORVANE_FLAGS"
                            } else {
                                ""
                            }
                        )
                    } else {
                        format!(
                            "Preset: {}{}",
                            preset.title(),
                            if preset_from_env {
                                " · set by CORVANE_FLAGS"
                            } else {
                                ""
                            }
                        )
                    }),
            )
            .child(
                primary_button("flags-done", "Done", false, cx)
                    .min_w(zpx(120.))
                    .on_click(|_, _, cx| FlagsDialog::close(cx)),
            );

        let banner = self
            .import_error
            .clone()
            .map(|message| dialog_error_banner(message, cx).into_any_element())
            .or_else(|| {
                self.import_note.clone().map(|note| {
                    div()
                        .flex_none()
                        .px(SPACING_DOUBLE())
                        .py(SPACING_HALF())
                        .border_b_1()
                        .border_color(t.box_border)
                        .bg(t.banner_warning_background)
                        .text_color(t.banner_warning_text)
                        .text_size(FONT_SIZE_SM())
                        .child(note)
                        .into_any_element()
                })
            });

        deferred(
            anchored().position(point(zpx(0.), zpx(0.))).child(
                div()
                    .id("flags-overlay")
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(t.dialog_backdrop)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| FlagsDialog::close(cx))
                    .child(
                        div()
                            .id("flags-dialog")
                            .role(Role::Dialog)
                            .aria_label("Flags")
                            .child(window_title("Flags"))
                            .track_focus(&self.focus_handle)
                            .key_context("FlagsDialog")
                            .on_action(cx.listener(|this, _: &Find, window, cx| {
                                let handle = this.search.read(cx).focus_handle(cx);
                                window.focus(&handle, cx);
                                cx.stop_propagation();
                            }))
                            .on_action(cx.listener(|_, _: &CloseFoldout, _, cx| {
                                FlagsDialog::close(cx);
                                cx.stop_propagation();
                            }))
                            .w(width)
                            .h(height)
                            .flex()
                            .flex_col()
                            .rounded(BORDER_RADIUS())
                            .bg(t.background)
                            .text_color(t.text)
                            .text_size(FONT_SIZE())
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
                            .child(header)
                            .child(toolbar)
                            .children(banner)
                            .child(body)
                            .children(restart_bar)
                            .child(footer),
                    ),
            ),
        )
        .with_priority(20)
    }
}
