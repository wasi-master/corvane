//! Changes sidebar: filter header, "N changed files" row, file list, commit form.
//! `styles/ui/changes/{_changes-list,_commit-message}.scss`.

use std::cell::Cell;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use corvane_core::filter::{filtered_files, no_results_message, option_count};
use corvane_core::{
    AppState, DiffSelectionType, Dispatcher, FileListFilter, FileStatusKind, FilterOption, Tip,
    WorkingDirectoryFileChange,
};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::{
    Copy, Cut, Enter, Escape, IndentInline, InputEvent, InputState, MoveDown, MoveUp, Paste, Redo,
    SelectAll, Textarea, TextareaState, Undo,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{
    Commit, SelectAllFiles, SelectNextFile, SelectPreviousFile, SpellAddToDictionary,
    SpellSuggestion0, SpellSuggestion1, SpellSuggestion2, SpellSuggestion3, SpellSuggestion4,
    ToggleCommitSpellcheck,
};
use crate::autocompletion::{self, Autocompletion, PickHandler};
use crate::context_menu::{ContextMenu, MenuItem};
use crate::diff_view::status_icon;
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    InputMenuBuilder, avatar_image, avatar_lookup, button, checkbox, checkbox_tristate,
    primary_button, text_box, text_box_with_menu,
};

/// Which commit-form field an autocompletion / spellcheck result belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommitField {
    Summary,
    Description,
}

/// The misspelled word under the last right-click and its suggestions; the
/// context menu's items are index actions (`SpellSuggestionN`).
struct PendingSpell {
    field: CommitField,
    range: Range<usize>,
    word: String,
    suggestions: Vec<String>,
}

pub struct ChangesSidebar {
    filter: Entity<InputState>,
    summary: Entity<InputState>,
    description: Entity<TextareaState>,
    state: Entity<AppState>,
    seen_commit_nonce: u64,
    seen_amend_nonce: u64,
    context_menu: Option<Entity<ContextMenu>>,
    /// GHD `ChangesListFilterOptions` popover.
    filter_popover_open: bool,
    filter_button_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Focus target for arrow-key navigation of the list.
    list_focus: FocusHandle,
    /// View › Hide Changes Filter (`isChangesFilterVisible`).
    filter_visible: bool,
    /// GHD `AutocompletingTextInput` state for whichever field has the popup.
    autocomplete: Option<(CommitField, Autocompletion)>,
    /// Misspelled words per field (`NSSpellChecker`), refreshed on change.
    summary_misspelled: Vec<Range<usize>>,
    description_misspelled: Vec<Range<usize>>,
    pending_spell: Option<PendingSpell>,
}

impl ChangesSidebar {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        // Clear the form after a successful commit (GHD resets `commitMessage`).
        cx.observe_in(&state, window, |this, state, window, cx| {
            let nonce = state
                .read(cx)
                .selected_state()
                .map(|rs| rs.commit_nonce)
                .unwrap_or(0);
            if nonce != this.seen_commit_nonce {
                this.seen_commit_nonce = nonce;
                this.summary.update(cx, |s, cx| s.set_value("", window, cx));
                this.description
                    .update(cx, |s, cx| s.set_value("", window, cx));
                this.summary_misspelled.clear();
                this.description_misspelled.clear();
                this.autocomplete = None;
                cx.notify();
            }
            // GHD `prepareToAmendCommit`: load the commit's message into the form.
            let (amend_nonce, to_amend) = state
                .read(cx)
                .selected_state()
                .map(|rs| (rs.amend_nonce, rs.commit_to_amend.clone()))
                .unwrap_or((0, None));
            if amend_nonce != this.seen_amend_nonce {
                this.seen_amend_nonce = amend_nonce;
                if let Some(commit) = to_amend {
                    this.summary
                        .update(cx, |s, cx| s.set_value(commit.summary.clone(), window, cx));
                    this.description
                        .update(cx, |s, cx| s.set_value(commit.body.clone(), window, cx));
                    this.refresh_spelling(CommitField::Summary, cx);
                    this.refresh_spelling(CommitField::Description, cx);
                    cx.notify();
                }
            }
        })
        .detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        let summary = cx.new(|cx| InputState::new(window, cx).placeholder("Summary (required)"));
        let description = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("Description")
        });
        cx.subscribe(&summary, |this, _, ev: &InputEvent, cx| {
            this.on_input_event(CommitField::Summary, ev, cx)
        })
        .detach();
        cx.subscribe(&description, |this, _, ev: &InputEvent, cx| {
            this.on_input_event(CommitField::Description, ev, cx)
        })
        .detach();
        Self {
            filter,
            summary,
            description,
            state,
            seen_commit_nonce: 0,
            seen_amend_nonce: 0,
            context_menu: None,
            filter_popover_open: false,
            filter_button_bounds: Rc::new(Cell::new(Bounds::default())),
            list_focus: cx.focus_handle(),
            filter_visible: true,
            autocomplete: None,
            summary_misspelled: Vec::new(),
            description_misspelled: Vec::new(),
            pending_spell: None,
        }
    }

    // ---- autocompletion + spellcheck (GHD `AutocompletingTextInput`) ----

    fn on_input_event(&mut self, field: CommitField, ev: &InputEvent, cx: &mut Context<Self>) {
        match ev {
            InputEvent::Change => {
                self.refresh_spelling(field, cx);
                self.open_autocomplete(field, cx);
            }
            InputEvent::Blur if self.autocomplete.as_ref().is_some_and(|(f, _)| *f == field) => {
                self.autocomplete = None;
                cx.notify();
            }
            _ => {}
        }
    }

    fn field_text_and_caret(&self, field: CommitField, cx: &App) -> (String, usize) {
        match field {
            CommitField::Summary => {
                let s = self.summary.read(cx);
                (s.value().to_string(), s.cursor())
            }
            CommitField::Description => {
                let s = self.description.read(cx);
                (s.value().to_string(), s.cursor())
            }
        }
    }

    fn field_focus_handle(&self, field: CommitField, cx: &App) -> FocusHandle {
        match field {
            CommitField::Summary => self.summary.read(cx).focus_handle(cx),
            CommitField::Description => self.description.read(cx).focus_handle(cx),
        }
    }

    /// Replace a byte range of a field's text and put the caret after it.
    fn replace_range(
        &mut self,
        field: CommitField,
        range: Range<usize>,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match field {
            CommitField::Summary => self.summary.update(cx, |s, cx| {
                s.set_selected_range(range, cx);
                s.replace(text, window, cx);
            }),
            CommitField::Description => self.description.update(cx, |s, cx| {
                s.set_selected_range(range, cx);
                s.replace(text, window, cx);
            }),
        }
        let handle = self.field_focus_handle(field, cx);
        window.focus(&handle, cx);
        self.refresh_spelling(field, cx);
        cx.notify();
    }

    /// GHD `open`: re-run the providers against the text at the caret.
    fn open_autocomplete(&mut self, field: CommitField, cx: &mut Context<Self>) {
        let (text, caret) = self.field_text_and_caret(field, cx);
        let github = {
            let s = self.state.read(cx);
            s.selected
                .and_then(|id| s.repository(id))
                .and_then(|r| r.github.clone())
        };
        self.autocomplete =
            autocompletion::attempt(&text, caret, github.as_ref(), cx).map(|ac| (field, ac));
        cx.notify();
    }

    /// ↑/↓ while the popup is open; `false` lets the key reach the field.
    fn autocomplete_move(&mut self, delta: i64, cx: &mut Context<Self>) -> bool {
        match self.autocomplete.as_mut() {
            Some((_, ac)) => {
                ac.move_selection(delta);
                cx.notify();
                true
            }
            None => false,
        }
    }

    /// Enter / Tab: insert the highlighted item (GHD `insertCompletion`).
    fn autocomplete_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match self.autocomplete.as_ref().and_then(|(_, ac)| ac.selected) {
            Some(ix) => {
                self.autocomplete_insert(ix, window, cx);
                true
            }
            None => false,
        }
    }

    fn autocomplete_insert(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((field, ac)) = self.autocomplete.take() else {
            return;
        };
        let Some(hit) = ac.hits.get(ix) else {
            return;
        };
        // The trigger character sits right before the filter text; GHD
        // appends its `completionSuffix` (a space).
        let range = ac.range.start.saturating_sub(1)..ac.range.end;
        let text = format!("{} ", hit.completion_text());
        self.replace_range(field, range, text, window, cx);
    }

    fn refresh_spelling(&mut self, field: CommitField, cx: &mut Context<Self>) {
        let enabled = self.state.read(cx).settings.commit_spellcheck_enabled;
        let ranges = if enabled {
            let (text, _) = self.field_text_and_caret(field, cx);
            corvane_platform::spell::misspelled_ranges(&text)
        } else {
            Vec::new()
        };
        match field {
            CommitField::Summary => self.summary_misspelled = ranges,
            CommitField::Description => self.description_misspelled = ranges,
        }
        cx.notify();
    }

    /// Window-space rectangle of a byte range in a field, scroll included.
    fn range_rect(
        &self,
        field: CommitField,
        range: &Range<usize>,
        cx: &App,
    ) -> Option<Bounds<Pixels>> {
        let (bounds, scroll) = match field {
            CommitField::Summary => {
                let s = self.summary.read(cx);
                (s.range_to_bounds(range)?, s.scroll_offset())
            }
            CommitField::Description => {
                let s = self.description.read(cx);
                (s.range_to_bounds(range)?, s.scroll_offset())
            }
        };
        Some(Bounds {
            origin: point(bounds.origin.x + scroll.x, bounds.origin.y),
            size: bounds.size,
        })
    }

    /// The misspelled word under `position`, for the context menu.
    fn misspelled_at(
        &self,
        field: CommitField,
        position: Point<Pixels>,
        cx: &App,
    ) -> Option<(Range<usize>, String)> {
        let ranges = match field {
            CommitField::Summary => &self.summary_misspelled,
            CommitField::Description => &self.description_misspelled,
        };
        let (text, _) = self.field_text_and_caret(field, cx);
        ranges
            .iter()
            .find(|r| {
                self.range_rect(field, r, cx)
                    .is_some_and(|rect| rect.contains(&position))
            })
            .and_then(|r| text.get(r.clone()).map(|w| (r.clone(), w.to_string())))
    }

    /// Red dotted underline beneath each misspelled word (Chromium's marker).
    fn spell_overlay(&self, field: CommitField, cx: &Context<Self>) -> Option<AnyElement> {
        let ranges = match field {
            CommitField::Summary => self.summary_misspelled.clone(),
            CommitField::Description => self.description_misspelled.clone(),
        };
        if ranges.is_empty() {
            return None;
        }
        let color = cx.ghd().error;
        let weak = cx.weak_entity();
        Some(
            canvas(
                move |_, _, cx| {
                    weak.upgrade().map(|this| {
                        let this = this.read(cx);
                        ranges
                            .iter()
                            .filter_map(|r| this.range_rect(field, r, cx))
                            .collect::<Vec<_>>()
                    })
                },
                move |bounds, rects, window, _| {
                    let Some(rects) = rects else { return };
                    window.with_content_mask(Some(ContentMask { bounds }), |window| {
                        for rect in rects {
                            let y = rect.bottom() - px(3.);
                            let mut x = rect.left();
                            while x < rect.right() {
                                window.paint_quad(fill(
                                    Bounds::new(point(x, y), size(px(2.), px(2.))),
                                    color,
                                ));
                                x += px(4.);
                            }
                        }
                    });
                },
            )
            .absolute()
            .inset_0()
            .into_any_element(),
        )
    }

    /// GHD `onAutocompletingInputContextMenu` + Chromium's spelling items:
    /// suggestions, Add to Dictionary, the edit menu, the spellcheck toggle.
    fn input_menu(&self, field: CommitField, cx: &Context<Self>) -> InputMenuBuilder {
        let weak = cx.weak_entity();
        Rc::new(move |mut menu, window, cx| {
            let mut suggestions: Option<Vec<String>> = None;
            let mut enabled = true;
            let mut has_selection = false;
            weak.update(cx, |this, cx| {
                this.pending_spell = None;
                let handle = this.field_focus_handle(field, cx);
                window.focus(&handle, cx);
                has_selection = match field {
                    CommitField::Summary => !this.summary.read(cx).selected_range().is_empty(),
                    CommitField::Description => {
                        !this.description.read(cx).selected_range().is_empty()
                    }
                };
                enabled = this.state.read(cx).settings.commit_spellcheck_enabled;
                if let Some((range, word)) = this.misspelled_at(field, window.mouse_position(), cx)
                {
                    let guesses = corvane_platform::spell::guesses(&word);
                    this.pending_spell = Some(PendingSpell {
                        field,
                        range,
                        word,
                        suggestions: guesses.clone(),
                    });
                    suggestions = Some(guesses);
                }
            })
            .ok();
            if let Some(guesses) = suggestions {
                if guesses.is_empty() {
                    menu = menu.menu_with_disabled(
                        "No Guesses Found",
                        true,
                        Box::new(SpellSuggestion0),
                    );
                }
                for (ix, guess) in guesses.into_iter().enumerate() {
                    let action: Box<dyn Action> = match ix {
                        0 => Box::new(SpellSuggestion0),
                        1 => Box::new(SpellSuggestion1),
                        2 => Box::new(SpellSuggestion2),
                        3 => Box::new(SpellSuggestion3),
                        _ => Box::new(SpellSuggestion4),
                    };
                    menu = menu.menu(guess, action);
                }
                menu = menu
                    .menu("Add to Dictionary", Box::new(SpellAddToDictionary))
                    .separator();
            }
            menu.menu("Undo", Box::new(Undo))
                .menu("Redo", Box::new(Redo))
                .separator()
                .menu_with_disabled("Cut", !has_selection, Box::new(Cut))
                .menu_with_disabled("Copy", !has_selection, Box::new(Copy))
                .menu("Paste", Box::new(Paste))
                .menu("Select All", Box::new(SelectAll))
                .separator()
                .menu(
                    if enabled {
                        "Disable Commit Spellcheck"
                    } else {
                        "Enable Commit Spellcheck"
                    },
                    Box::new(ToggleCommitSpellcheck),
                )
        })
    }

    fn apply_spell_suggestion(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_spell.take() else {
            return;
        };
        let Some(word) = pending.suggestions.get(ix).cloned() else {
            return;
        };
        self.replace_range(pending.field, pending.range, word, window, cx);
    }

    fn add_to_dictionary(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_spell.take() else {
            return;
        };
        corvane_platform::spell::learn_word(&pending.word);
        self.refresh_spelling(CommitField::Summary, cx);
        self.refresh_spelling(CommitField::Description, cx);
    }

    fn toggle_spellcheck(&mut self, cx: &mut Context<Self>) {
        Dispatcher::update_settings(cx, |s| {
            s.commit_spellcheck_enabled = !s.commit_spellcheck_enabled
        });
        self.refresh_spelling(CommitField::Summary, cx);
        self.refresh_spelling(CommitField::Description, cx);
    }

    /// View › Go to Summary.
    pub fn focus_summary(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.summary.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        cx.notify();
    }

    /// Edit › Find.
    pub fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter_visible = true;
        let handle = self.filter.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        cx.notify();
    }

    /// View › Show / Hide Changes Filter.
    pub fn toggle_filter(&mut self, cx: &mut Context<Self>) {
        self.filter_visible = !self.filter_visible;
        cx.notify();
    }

    /// Arrow keys move the selection through the visible files.
    fn select_relative(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (files, _) = self.visible_files(cx);
        if files.is_empty() {
            return;
        }
        let (id, current) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            (
                id,
                s.selected_state().and_then(|rs| rs.selected_file.clone()),
            )
        };
        let index = current
            .and_then(|p| files.iter().position(|f| f.path == p))
            .map(|i| i as isize + delta)
            .unwrap_or(0)
            .clamp(0, files.len() as isize - 1) as usize;
        Dispatcher::select_file(id, files[index].path.clone(), cx);
    }

    /// Commit form gear: GHD's native checkbox menu (`onCommitOptionsButtonClick`).
    fn open_commit_options_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (id, options) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            (
                id,
                s.repository(id)
                    .map(|r| r.commit_options)
                    .unwrap_or_default(),
            )
        };
        let items = vec![
            MenuItem::checkbox(
                "Bypass Commit Hooks",
                options.skip_commit_hooks,
                move |_, cx| {
                    Dispatcher::update_commit_options(
                        id,
                        |o| o.skip_commit_hooks = !o.skip_commit_hooks,
                        cx,
                    )
                },
            ),
            MenuItem::checkbox(
                "Add Signed-off-by Trailer",
                options.sign_off_commits,
                move |_, cx| {
                    Dispatcher::update_commit_options(
                        id,
                        |o| o.sign_off_commits = !o.sign_off_commits,
                        cx,
                    )
                },
            ),
            MenuItem::checkbox(
                "Allow Empty Commit",
                options.allow_empty_commit,
                move |_, cx| {
                    Dispatcher::update_commit_options(
                        id,
                        |o| o.allow_empty_commit = !o.allow_empty_commit,
                        cx,
                    )
                },
            ),
        ];
        self.open_menu(items, position, window, cx);
    }

    /// Files that pass the text + option filters, plus the unfiltered total.
    fn visible_files(&self, cx: &App) -> (Vec<WorkingDirectoryFileChange>, usize) {
        let text = self.filter.read(cx).value().to_string();
        let s = self.state.read(cx);
        let Some(rs) = s.selected_state() else {
            return (Vec::new(), 0);
        };
        let Some(status) = rs.status.as_ref() else {
            return (Vec::new(), 0);
        };
        let visible = filtered_files(&status.files, &text, &rs.file_list_filter)
            .into_iter()
            .cloned()
            .collect();
        (visible, status.files.len())
    }

    fn filter_options(&self, cx: &App) -> FileListFilter {
        self.state
            .read(cx)
            .selected_state()
            .map(|rs| rs.file_list_filter)
            .unwrap_or_default()
    }

    /// `ChangesListFilterOptions` popover: header, five option checkboxes,
    /// "Clear filters" when anything is active. Anchored under the button.
    fn filter_popover(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        if !self.filter_popover_open {
            return None;
        }
        let t = cx.ghd();
        let s = self.state.read(cx);
        let id = s.selected?;
        let files = s
            .selected_state()
            .and_then(|rs| rs.status.as_ref())
            .map(|st| st.files.clone())
            .unwrap_or_default();
        let filter = self.filter_options(cx);
        let text_active = !self.filter.read(cx).value().trim().is_empty();
        let active = filter.count_active() > 0 || text_active;
        let bounds = self.filter_button_bounds.get();
        let close =
            |this: &mut Self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>| {
                this.filter_popover_open = false;
                cx.notify();
            };
        let option_row = |option: FilterOption, label: &str| {
            let checked = filter.get(option);
            let count = option_count(option, &files);
            div()
                .id(SharedString::from(format!("filter-opt-{label}")))
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF)
                .py(px(3.))
                .cursor_pointer()
                // GHD closes the popover after every option change
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter_popover_open = false;
                    Dispatcher::toggle_filter_option(id, option, cx)
                }))
                .child(checkbox(
                    SharedString::from(format!("filter-check-{label}")),
                    checked,
                    false,
                    cx,
                ))
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .child(format!("{label} ({count})")),
                )
        };
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("filter-popover-overlay")
                        .relative()
                        .size_full()
                        .on_mouse_down(MouseButton::Left, cx.listener(close))
                        .on_mouse_down(MouseButton::Right, cx.listener(close))
                        .child(
                            div()
                                .id("filter-popover")
                                .absolute()
                                .left(bounds.origin.x)
                                .top(bounds.origin.y + bounds.size.height + px(8.))
                                .min_w(px(200.))
                                .flex()
                                .flex_col()
                                .px(SPACING)
                                .pt(SPACING)
                                .rounded(BORDER_RADIUS)
                                .bg(t.background)
                                .text_color(t.text)
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
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(FONT_SIZE_MD)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child("Filter Options"),
                                        )
                                        .child(
                                            div()
                                                .id("filter-popover-close")
                                                .size(px(16.))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.filter_popover_open = false;
                                                    cx.notify();
                                                }))
                                                .child(octicon(Octicon::X, t.text_secondary)),
                                        ),
                                )
                                .child(
                                    div()
                                        .my(SPACING)
                                        .flex()
                                        .flex_col()
                                        .child(option_row(
                                            FilterOption::IncludedInCommit,
                                            "Included in commit",
                                        ))
                                        .child(option_row(
                                            FilterOption::ExcludedFromCommit,
                                            "Excluded from commit",
                                        ))
                                        .child(option_row(FilterOption::NewFiles, "New files"))
                                        .child(option_row(
                                            FilterOption::ModifiedFiles,
                                            "Modified files",
                                        ))
                                        .child(option_row(
                                            FilterOption::DeletedFiles,
                                            "Deleted files",
                                        )),
                                )
                                .when(active, |d| {
                                    d.child(div().pt(SPACING_HALF).pb(SPACING).child(
                                        button("filter-clear", "Clear filters", cx).on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.filter.update(cx, |s, cx| {
                                                    s.set_value("", window, cx)
                                                });
                                                this.filter_popover_open = false;
                                                Dispatcher::clear_filter_options(id, cx);
                                            }),
                                        ),
                                    ))
                                })
                                .when(!active, |d| d.pb(SPACING_HALF)),
                        ),
                ),
            )
            .with_priority(25),
        )
    }

    fn open_menu(
        &mut self,
        items: Vec<MenuItem>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // macOS: a real NSMenu (GHD's Electron `Menu.popup`); the GPUI menu is the fallback.
        #[cfg(target_os = "macos")]
        {
            crate::native_menu::show_context_menu(items, position, window, cx);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let menu = cx.new(|cx| ContextMenu::new(position, items, window, cx));
            cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
                this.context_menu = None;
                cx.notify();
            })
            .detach();
            self.context_menu = Some(menu);
            cx.notify();
        }
    }

    /// GHD `onItemContextMenu`: the default menu, or the reduced one while a
    /// rebase is stopped on conflicts (`getRebaseContextMenu`). Right-clicking
    /// inside the current selection applies the items to every selected file.
    fn open_file_menu(
        &mut self,
        file: WorkingDirectoryFileChange,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (id, confirm, repo_path, selected_files, rebase_conflict, status_files) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            let Some(rs) = s.selected_state() else { return };
            if rs.committing {
                return;
            }
            let Some(repo) = s.repository(id) else { return };
            (
                id,
                s.settings.confirm_discard_changes,
                repo.path.clone(),
                rs.selected_files.clone(),
                rs.conflict_state
                    .as_ref()
                    .is_some_and(|c| matches!(c.kind, corvane_core::ConflictKind::Rebase { .. })),
                rs.status
                    .as_ref()
                    .map(|st| st.files.clone())
                    .unwrap_or_default(),
            )
        };
        let path = file.path.clone();
        let full = repo_path.join(&path);
        let deleted = file.status.kind == FileStatusKind::Deleted;
        let editor_label = self.state.read(cx).editor_label();
        let discard_item = |paths: Vec<String>| {
            // `getDiscardChangesMenuItemLabel`
            let label = match (paths.len(), confirm) {
                (1, true) => "Discard Changes…".to_string(),
                (1, false) => "Discard Changes".to_string(),
                (n, true) => format!("Discard {n} Selected Changes…"),
                (n, false) => format!("Discard {n} Selected Changes"),
            };
            MenuItem::new(label, move |_, cx| {
                Dispatcher::request_discard_changes(id, paths.clone(), cx)
            })
        };
        let copy_items = |files: Vec<WorkingDirectoryFileChange>| {
            let multi = files.len() > 1;
            let absolute = files
                .iter()
                .map(|f| repo_path.join(&f.path).to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("\n");
            let relative = files
                .iter()
                .map(|f| f.path.clone())
                .collect::<Vec<_>>()
                .join("\n");
            vec![
                MenuItem::new(
                    if multi {
                        "Copy Paths"
                    } else {
                        "Copy File Path"
                    },
                    move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(absolute.clone())),
                ),
                MenuItem::new(
                    if multi {
                        "Copy Relative Paths"
                    } else {
                        "Copy Relative File Path"
                    },
                    move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(relative.clone())),
                ),
            ]
        };
        let open_items = |full: PathBuf, deleted: bool| {
            let reveal = full.clone();
            let editor = full.clone();
            let default = full;
            vec![
                MenuItem::new("Reveal in Finder", move |_, cx| cx.reveal_path(&reveal))
                    .enabled(!deleted),
                MenuItem::new(format!("Open in {editor_label}"), move |_, cx| {
                    Dispatcher::open_in_editor(editor.clone(), cx)
                })
                .enabled(!deleted),
                MenuItem::new("Open with Default Program", move |_, cx| {
                    cx.open_with_system(&default)
                })
                .enabled(!deleted),
            ]
        };

        if rebase_conflict {
            let mut items = Vec::new();
            if file.status.kind == FileStatusKind::Untracked {
                items.push(discard_item(vec![path.clone()]));
                items.push(MenuItem::separator());
            }
            items.extend(copy_items(vec![file.clone()]));
            items.push(MenuItem::separator());
            items.extend(open_items(full, deleted));
            self.open_menu(items, position, window, cx);
            return;
        }

        // `getDefaultContextMenu`
        let targets: Vec<WorkingDirectoryFileChange> = if selected_files.contains(&path) {
            status_files
                .into_iter()
                .filter(|f| selected_files.contains(&f.path))
                .collect()
        } else {
            vec![file.clone()]
        };
        let paths: Vec<String> = targets.iter().map(|f| f.path.clone()).collect();
        let mut items = vec![discard_item(paths.clone()), MenuItem::separator()];
        if paths.len() == 1 {
            let is_gitignore = file.file_name() == ".gitignore";
            items.push(
                MenuItem::new("Ignore File (Add to .gitignore)", {
                    let p = path.clone();
                    move |_, cx| Dispatcher::ignore_files(id, vec![p.clone()], cx)
                })
                .enabled(!is_gitignore),
            );
            let parents: Vec<&str> = {
                let mut components: Vec<&str> = path.split('/').collect();
                components.pop();
                components
            };
            if !parents.is_empty() {
                let folders: Vec<MenuItem> = (0..parents.len())
                    .map(|index| {
                        let label = format!("/{}", parents[..parents.len() - index].join("/"));
                        let pattern = label.clone();
                        MenuItem::new(label, move |_, cx| {
                            Dispatcher::ignore_files(id, vec![pattern.clone()], cx)
                        })
                    })
                    .collect();
                items.push(
                    MenuItem::submenu("Ignore Folder (Add to .gitignore)", folders)
                        .enabled(!is_gitignore),
                );
            }
        } else {
            let ignorable: Vec<String> = paths
                .iter()
                .filter(|p| !p.ends_with(".gitignore"))
                .cloned()
                .collect();
            let enabled = !ignorable.is_empty();
            items.push(
                MenuItem::new(
                    format!("Ignore {} Selected Files (Add to .gitignore)", paths.len()),
                    move |_, cx| Dispatcher::ignore_files(id, ignorable.clone(), cx),
                )
                .enabled(enabled),
            );
        }
        // "Five menu items should be enough for everyone"
        let mut extensions: Vec<String> = Vec::new();
        for p in &paths {
            if let Some(ext) = Path::new(p)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                && !extensions.contains(&ext)
            {
                extensions.push(ext);
            }
        }
        for ext in extensions.into_iter().take(5) {
            let pattern = format!("*{ext}");
            items.push(MenuItem::new(
                format!("Ignore All {ext} Files (Add to .gitignore)"),
                move |_, cx| Dispatcher::ignore_patterns(id, vec![pattern.clone()], cx),
            ));
        }
        if paths.len() > 1 {
            items.push(MenuItem::separator());
            let include = paths.clone();
            items.push(MenuItem::new("Include Selected Files", move |_, cx| {
                Dispatcher::set_files_included(id, include.clone(), true, cx)
            }));
            let exclude = paths.clone();
            items.push(MenuItem::new("Exclude Selected Files", move |_, cx| {
                Dispatcher::set_files_included(id, exclude.clone(), false, cx)
            }));
        }
        items.push(MenuItem::separator());
        items.extend(copy_items(targets));
        items.push(MenuItem::separator());
        items.extend(open_items(full, deleted));
        self.open_menu(items, position, window, cx);
    }

    /// GHD `onContextMenu` on the list itself: Discard All / Stash All.
    fn open_list_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (id, confirm, paths) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            let Some(rs) = s.selected_state() else { return };
            if rs.committing {
                return;
            }
            let paths: Vec<String> = rs
                .status
                .as_ref()
                .map(|st| st.files.iter().map(|f| f.path.clone()).collect())
                .unwrap_or_default();
            (id, s.settings.confirm_discard_changes, paths)
        };
        let has_changes = !paths.is_empty();
        let items = vec![
            MenuItem::new(
                if confirm {
                    "Discard All Changes…"
                } else {
                    "Discard All Changes"
                },
                move |_, cx| Dispatcher::request_discard_changes(id, paths.clone(), cx),
            )
            .enabled(has_changes),
            // TODO(M4): stashes; disabled until then.
            MenuItem::new("Stash All Changes", |_, _| {}).enabled(false),
        ];
        self.open_menu(items, position, window, cx);
    }

    fn do_commit(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let summary = self.summary.read(cx).value().to_string();
        let description = self.description.read(cx).value().to_string();
        Dispatcher::commit(id, summary, description, cx);
    }

    /// `.filtered-changes-list .header`: filter row + check-all row.
    fn header(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .px(SPACING)
            .py(SPACING_HALF)
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .when(self.filter_visible, |d| {
                d.child(
                    // Filter row: [Filter Options ▾][Filter…] as a joined button group
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(TEXT_FIELD_HEIGHT)
                        .child({
                            let active = self.filter_options(cx).count_active() > 0;
                            let bounds_cell = self.filter_button_bounds.clone();
                            div()
                                .id("filter-options")
                                .relative()
                                .h(TEXT_FIELD_HEIGHT)
                                .w(px(48.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(2.))
                                .border_1()
                                .border_color(t.secondary_button_border)
                                .rounded_l(BORDER_RADIUS)
                                .bg(t.secondary_button_background)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.filter_popover_open = !this.filter_popover_open;
                                    cx.notify();
                                }))
                                .child(
                                    canvas(move |b, _, _| bounds_cell.set(b), |_, _, _, _| {})
                                        .absolute()
                                        .size_full(),
                                )
                                // `.active span:first-child { color: box-selected-active-background }`
                                .child(octicon(
                                    Octicon::Filter,
                                    if active {
                                        t.box_selected_active_background
                                    } else {
                                        t.secondary_button_text
                                    },
                                ))
                                .child(
                                    octicon(Octicon::TriangleDown, t.secondary_button_text)
                                        .size(px(12.)),
                                )
                                // `.active-badge`: 5 px dot with a 1 px ring, right 18 / top 4
                                .when(active, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .top(px(4.))
                                            .right(px(18.))
                                            .p(px(1.))
                                            .rounded_full()
                                            .bg(t.secondary_button_background)
                                            .child(
                                                div()
                                                    .size(px(5.))
                                                    .rounded_full()
                                                    .bg(t.box_selected_active_background),
                                            ),
                                    )
                                })
                        })
                        .child(
                            text_box("changes-filter", &self.filter, None, window, cx)
                                .rounded_l(px(0.))
                                .border_l_0(),
                        ),
                )
            })
            .child(
                // "☑ N changed files"
                div()
                    .h(ROW_HEIGHT)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    // GHD shows the include-all box checked but disabled when there is nothing to commit.
                    .child({
                        let (visible, total, include_all, repo_id) = self.header_state(cx);
                        let paths: Vec<String> = visible.iter().map(|f| f.path.clone()).collect();
                        let disabled = total == 0 || visible.is_empty();
                        let include = include_all != Some(true);
                        checkbox_tristate("check-all", include_all, disabled, cx).when_some(
                            repo_id.filter(|_| !disabled),
                            |d, id| {
                                d.on_click(move |_, _, cx| {
                                    Dispatcher::set_files_included(id, paths.clone(), include, cx)
                                })
                            },
                        )
                    })
                    .child({
                        let (visible, total, _, _) = self.header_state(cx);
                        // GHD: "3 of 10 changed files" while a filter hides some
                        let prefix = if visible.len() != total {
                            format!("{} of ", crate::format::format_count(visible.len() as u64))
                        } else {
                            String::new()
                        };
                        div().text_size(FONT_SIZE).truncate().child(if total == 1 {
                            format!("{prefix}1 changed file")
                        } else {
                            format!(
                                "{prefix}{} changed files",
                                crate::format::format_count(total as u64)
                            )
                        })
                    }),
            )
    }

    fn branch_name(&self, cx: &App) -> SharedString {
        self.state
            .read(cx)
            .selected_state()
            .and_then(|s| s.info.as_ref())
            .and_then(|i| match &i.tip {
                Tip::Valid { branch } => Some(branch.name.clone()),
                Tip::Unborn { name } => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_default()
            .into()
    }

    /// (visible files, total, include-all tri-state of the visible files, repo id)
    fn header_state(
        &self,
        cx: &App,
    ) -> (
        Vec<WorkingDirectoryFileChange>,
        usize,
        Option<bool>,
        Option<u64>,
    ) {
        let (visible, total) = self.visible_files(cx);
        let id = self.state.read(cx).selected;
        // `getCheckAllValue`: the box reflects only the files passing the filter
        let include_all = if visible.is_empty() {
            Some(true)
        } else {
            let all = visible
                .iter()
                .all(|f| f.selection.kind() == DiffSelectionType::All);
            let none = visible
                .iter()
                .all(|f| f.selection.kind() == DiffSelectionType::None);
            if all {
                Some(true)
            } else if none {
                Some(false)
            } else {
                None
            }
        };
        (visible, total, include_all, id)
    }

    /// `ChangesList`: 29 px rows - checkbox, dimmed directory + bold name, status icon.
    /// `ChangesList`: 29 px rows - checkbox, dimmed directory + bold name,
    /// status icon. Virtualized with `uniform_list` (GHD uses react-virtualized).
    fn list(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let repo_id = s.selected;
        let rs = s.selected_state();
        let (files, _) = self.visible_files(cx);
        let empty_message = if files.is_empty() {
            no_results_message(&self.filter.read(cx).value(), &self.filter_options(cx))
        } else {
            None
        };
        let selected: Rc<Vec<String>> =
            Rc::new(rs.map(|r| r.selected_files.clone()).unwrap_or_default());
        let order: Rc<Vec<String>> = Rc::new(files.iter().map(|f| f.path.clone()).collect());
        let files = Rc::new(files);
        let weak = cx.weak_entity();
        let list_focus = self.list_focus.clone();
        div()
            .id("changes-list")
            .flex_1()
            .min_h(px(100.))
            .bg(t.background)
            .flex()
            .flex_col()
            .when_some(empty_message, |d, message| {
                d.child(
                    div()
                        .p(SPACING_DOUBLE)
                        .text_size(FONT_SIZE)
                        .text_color(t.text_secondary)
                        .child(message),
                )
            })
            .child(
                uniform_list("changes-list-rows", files.len(), move |range, _, cx| {
                    range
                        .map(|ix| {
                            let file = &files[ix];
                            let is_selected = selected.contains(&file.path);
                            file_row(
                                file,
                                is_selected,
                                order.clone(),
                                repo_id,
                                weak.clone(),
                                list_focus.clone(),
                                cx,
                            )
                        })
                        .collect()
                })
                .flex_1()
                .min_h_0(),
            )
    }

    /// `.stashed-changes-button`: shown when the current branch has a stash.
    fn stash_button(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let id = s.selected?;
        let rs = s.selected_state()?;
        rs.stash.as_ref()?;
        let showing = rs.showing_stash;
        let hover_bg = t.box_selected_background;
        let (bg, text, icon) = if showing {
            (
                t.box_selected_active_background,
                t.box_selected_active_text,
                t.box_selected_active_text,
            )
        } else {
            (
                t.secondary_button_background,
                t.secondary_button_text,
                t.color_modified,
            )
        };
        Some(
            div()
                .id("stashed-changes-button")
                .flex_none()
                .w_full()
                .min_h(ROW_HEIGHT)
                .px(SPACING)
                .flex()
                .flex_row()
                .items_center()
                .border_t_1()
                .border_color(t.box_border)
                .bg(bg)
                .text_color(text)
                .text_size(FONT_SIZE)
                .cursor_pointer()
                .when(!showing, move |d| d.hover(move |s| s.bg(hover_bg)))
                .on_click(move |_, _, cx| Dispatcher::toggle_stash_view(id, cx))
                .child(octicon(Octicon::Stash, icon))
                .child(
                    div()
                        .flex_1()
                        .mx(SPACING_HALF)
                        .truncate()
                        .child("Stashed Changes"),
                )
                .child(octicon(Octicon::ChevronRight, text)),
        )
    }

    fn commit_disabled(&self, cx: &App) -> bool {
        let s = self.state.read(cx);
        let rs = s.selected_state();
        let any_included = rs
            .and_then(|r| r.status.as_ref())
            .map(|st| {
                st.files
                    .iter()
                    .any(|f| f.selection.kind() != DiffSelectionType::None)
            })
            .unwrap_or(false);
        let committing = rs.map(|r| r.committing).unwrap_or(false);
        let allow_empty = s
            .selected
            .and_then(|id| s.repository(id))
            .map(|r| r.commit_options.allow_empty_commit)
            .unwrap_or(false);
        let amending = rs.is_some_and(|r| r.commit_to_amend.is_some());
        self.summary.read(cx).value().trim().is_empty()
            || (!any_included && !allow_empty && !amending)
            || committing
    }

    /// `CommitWarning` with the information icon: "Your changes will modify
    /// your most recent commit. Stop amending to make these changes as a new commit."
    fn amend_notice(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let id = s.selected?;
        s.selected_state()?.commit_to_amend.as_ref()?;
        Some(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .mb(SPACING)
                .bg(t.box_alt_background)
                .child(
                    // `.warning-icon-container`: icon centred on a rule
                    div()
                        .relative()
                        .h(px(20.))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .top(px(10.))
                                .h(px(1.))
                                .bg(t.box_border),
                        )
                        .child(
                            div()
                                .px(SPACING_HALF)
                                .bg(t.box_alt_background)
                                .child(octicon(Octicon::Info, t.dialog_information)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .justify_center()
                        .text_size(FONT_SIZE)
                        .text_color(t.text_secondary)
                        .child("Your changes will modify your\u{a0}")
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.text)
                                .child("most recent commit"),
                        )
                        .child(".\u{a0}")
                        .child(
                            div()
                                .id("stop-amending")
                                .text_color(t.link)
                                .cursor_pointer()
                                .on_click(move |_, _, cx| Dispatcher::stop_amending(id, cx))
                                .child("Stop amending"),
                        )
                        .child("\u{a0}to make these changes as a new commit."),
                ),
        )
    }

    /// `#undo-commit`: "Committed N ago / summary" + Undo, after a commit.
    fn undo_bar(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let id = s.selected?;
        let last = s.selected_state()?.last_commit.clone()?;
        Some(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .mt(SPACING)
                .mx(px(-10.))
                .mb(px(-10.))
                .border_t_1()
                .border_color(t.box_border)
                .bg(t.box_alt_background)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .py(SPACING_HALF)
                        .pl(SPACING)
                        .pr(SPACING_HALF)
                        .text_size(FONT_SIZE_SM)
                        .child(
                            div()
                                .text_color(t.text_secondary)
                                .truncate()
                                .child(format!("Committed {}", relative(last.at))),
                        )
                        .child(div().truncate().child(last.summary.clone())),
                )
                .child(
                    div().p(SPACING).pl(px(0.)).child(
                        button("undo-commit", "Undo", cx)
                            .on_click(move |_, _, cx| Dispatcher::undo_commit(id, cx)),
                    ),
                ),
        )
    }

    /// `.commit-message-component`
    /// GHD `ContinueRebase` (`#continue-rebase`): while a rebase is stopped
    /// on conflicts the commit form gives way to a single "Continue rebase"
    /// button, enabled once every conflict is resolved.
    fn continue_rebase(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let id = s.selected?;
        let rs = s.repo_states.get(&id)?;
        let conflict = rs.conflict_state.as_ref()?;
        if !matches!(conflict.kind, corvane_core::ConflictKind::Rebase { .. }) {
            return None;
        }
        let status = rs.status.as_ref()?;
        let conflicted = corvane_core::conflicted_files(status, &conflict.manual_resolutions).len();
        let untracked = status
            .files
            .iter()
            .any(|f| f.status.kind == FileStatusKind::Untracked);
        let in_progress = rs
            .mco
            .as_ref()
            .is_some_and(|m| m.step == corvane_core::McoStep::ShowProgress);
        let enabled = conflicted == 0 && !in_progress;
        Some(
            div()
                .id("continue-rebase")
                .flex_none()
                .flex()
                .flex_col()
                .p(SPACING)
                .bg(t.box_alt_background)
                .border_t_1()
                .border_color(t.box_border)
                .child(
                    primary_button(
                        "continue-rebase-button",
                        if in_progress {
                            "Rebasing"
                        } else {
                            "Continue rebase"
                        },
                        !enabled,
                        cx,
                    )
                    .w_full()
                    .when(enabled, |d| {
                        d.on_click(move |_, _, cx| Dispatcher::continue_after_conflicts(id, cx))
                    }),
                )
                .when(untracked, |d| {
                    d.child(
                        div()
                            .pt(SPACING_HALF)
                            .text_align(TextAlign::Center)
                            .text_size(FONT_SIZE)
                            .child("Untracked files will be excluded"),
                    )
                })
                .into_any_element(),
        )
    }

    fn commit_form(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let avatar = self
            .state
            .read(cx)
            .selected_state()
            .and_then(|rs| rs.info.as_ref())
            .and_then(|i| i.identity.email.as_deref())
            .and_then(|email| avatar_lookup(email, cx));
        // Autocompletion popup anchored at the caret's bottom-left.
        let popup = self.autocomplete.as_ref().and_then(|(field, ac)| {
            let (bounds, line_height) = match field {
                CommitField::Summary => self.summary.read(cx).cursor_layout()?,
                CommitField::Description => self.description.read(cx).cursor_layout()?,
            };
            let anchor = point(bounds.origin.x, bounds.origin.y + line_height);
            let weak = cx.weak_entity();
            let on_pick: PickHandler = Rc::new(move |ix, window, cx| {
                weak.update(cx, |this, cx| this.autocomplete_insert(ix, window, cx))
                    .ok();
            });
            Some(autocompletion::popup(ac, anchor, on_pick, cx))
        });
        div()
            .id("commit-message")
            .key_context("CommitMessage")
            .on_action(cx.listener(|this, _: &Commit, _, cx| {
                if !this.commit_disabled(cx) {
                    this.do_commit(cx)
                }
            }))
            // Popup keys win over the field's own bindings (GHD `onKeyDown`).
            .capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                if this.autocomplete_move(-1, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                if this.autocomplete_move(1, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, ev: &Enter, window, cx| {
                if !ev.secondary && !ev.shift && this.autocomplete_accept(window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                if this.autocomplete_accept(window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &Escape, _, cx| {
                if this.autocomplete.take().is_some() {
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|this, _: &SpellSuggestion0, window, cx| {
                this.apply_spell_suggestion(0, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SpellSuggestion1, window, cx| {
                this.apply_spell_suggestion(1, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SpellSuggestion2, window, cx| {
                this.apply_spell_suggestion(2, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SpellSuggestion3, window, cx| {
                this.apply_spell_suggestion(3, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SpellSuggestion4, window, cx| {
                this.apply_spell_suggestion(4, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &SpellAddToDictionary, _, cx| this.add_to_dictionary(cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleCommitSpellcheck, _, cx| this.toggle_spellcheck(cx)),
            )
            .children(popup)
            .flex_none()
            .flex()
            .flex_col()
            .p(SPACING)
            .bg(t.box_alt_background)
            .border_t_1()
            .border_color(t.box_border)
            .child(
                // `.summary`: avatar + summary field
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .mb(SPACING)
                    .child(avatar_image(avatar, AVATAR_SIZE, cx))
                    .child(
                        text_box_with_menu(
                            "commit-summary",
                            &self.summary,
                            None,
                            Some(self.input_menu(CommitField::Summary, cx)),
                            window,
                            cx,
                        )
                        .relative()
                        .children(self.spell_overlay(CommitField::Summary, cx)),
                    ),
            )
            .child(
                // `.description-focus-container`: textarea + action bar
                div()
                    .flex()
                    .flex_col()
                    .mb(SPACING)
                    .border_1()
                    .border_color(t.box_border_contrast)
                    .rounded(BORDER_RADIUS)
                    .bg(t.box_background)
                    .overflow_hidden()
                    .child({
                        let menu = self.input_menu(CommitField::Description, cx);
                        div()
                            .relative()
                            .child(
                                Textarea::new(&self.description)
                                    .appearance(false)
                                    .small()
                                    .text_size(FONT_SIZE)
                                    .h(px(80.))
                                    .context_menu(move |m, window, cx| menu(m, window, cx)),
                            )
                            .children(self.spell_overlay(CommitField::Description, cx))
                    })
                    .child(
                        // `.action-bar`: add co-authors | commit options
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF)
                            .px(SPACING)
                            .pb(px(8.))
                            .child(octicon(Octicon::PersonAdd, t.text_secondary))
                            .child(div().w(px(1.)).h(px(16.)).bg(t.box_border_contrast))
                            .child(
                                div()
                                    .id("commit-options-button")
                                    .size(px(18.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, ev: &ClickEvent, window, cx| {
                                        this.open_commit_options_menu(ev.position(), window, cx)
                                    }))
                                    .child(octicon(Octicon::Gear, t.text_secondary)),
                            ),
                    ),
            )
            .children(self.amend_notice(cx))
            .child({
                let (amending, committing) = self
                    .state
                    .read(cx)
                    .selected_state()
                    .map(|r| (r.commit_to_amend.is_some(), r.committing))
                    .unwrap_or((false, false));
                let label = if amending {
                    div().flex().flex_row().child(if committing {
                        "Amending last commit"
                    } else {
                        "Amend last commit"
                    })
                } else {
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.))
                        .child(if committing {
                            "Committing to"
                        } else {
                            "Commit to"
                        })
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.branch_name(cx)),
                        )
                };
                primary_button("commit", label, self.commit_disabled(cx), cx)
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.commit_disabled(cx) {
                            this.do_commit(cx)
                        }
                    }))
            })
            .when_some(self.undo_bar(cx), |d, bar| d.child(bar))
    }
}

impl Render for ChangesSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // `CommitMessageAvatar`: the committer's avatar next to the summary.
        let identity_email = self
            .state
            .read(cx)
            .selected_state()
            .and_then(|rs| rs.info.as_ref())
            .and_then(|i| i.identity.email.clone());
        if let Some(email) = identity_email {
            Dispatcher::request_avatar_for_email(&email, cx);
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .id("changes-list-container")
                    .track_focus(&self.list_focus)
                    .key_context("ChangesList")
                    .on_action(
                        cx.listener(|this, _: &SelectNextFile, _, cx| this.select_relative(1, cx)),
                    )
                    .on_action(cx.listener(|this, _: &SelectPreviousFile, _, cx| {
                        this.select_relative(-1, cx)
                    }))
                    .on_action(cx.listener(|this, _: &SelectAllFiles, _, cx| {
                        let (files, _) = this.visible_files(cx);
                        if let Some(id) = this.state.read(cx).selected {
                            Dispatcher::select_all_files(
                                id,
                                files.into_iter().map(|f| f.path).collect(),
                                cx,
                            );
                        }
                    }))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                            this.open_list_menu(ev.position, window, cx)
                        }),
                    )
                    .child(self.header(window, cx))
                    .child(self.list(cx))
                    .children(self.stash_button(cx)),
            )
            .child(match self.continue_rebase(cx) {
                Some(block) => block,
                None => self.commit_form(window, cx).into_any_element(),
            })
            .children(self.context_menu.clone())
            .children(self.filter_popover(cx))
    }
}

/// One changes-list row (`ChangedFile`).
fn file_row(
    file: &WorkingDirectoryFileChange,
    is_selected: bool,
    order: Rc<Vec<String>>,
    repo_id: Option<u64>,
    weak: WeakEntity<ChangesSidebar>,
    list_focus: FocusHandle,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let (icon, color) = status_icon(file.status.kind, t);
    let path_for_select = file.path.clone();
    let path_for_toggle = file.path.clone();
    let include_value = match file.selection.kind() {
        DiffSelectionType::All => Some(true),
        DiffSelectionType::None => Some(false),
        DiffSelectionType::Partial => None,
    };
    let file_for_menu = file.clone();
    div()
        .id(SharedString::from(format!("file-{}", file.path)))
        .w_full()
        .h(ROW_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF)
        .px(SPACING)
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                let position = ev.position;
                let file = file_for_menu.clone();
                weak.update(cx, |this, cx| {
                    this.open_file_menu(file, position, window, cx)
                })
                .ok();
            },
        )
        .when(is_selected, |d| {
            d.bg(t.box_selected_background)
                .text_color(t.box_selected_text)
        })
        .when(!is_selected, move |d| d.hover(move |s| s.bg(hover_bg)))
        .when_some(repo_id, move |d, id| {
            // ⌘-click toggles, ⇧-click extends (GHD `SelectionSource`)
            d.on_click(move |ev: &ClickEvent, window, cx| {
                window.focus(&list_focus, cx);
                let modifiers = ev.modifiers();
                if modifiers.secondary() {
                    Dispatcher::toggle_file_selection(id, path_for_select.clone(), cx)
                } else if modifiers.shift {
                    Dispatcher::extend_file_selection(
                        id,
                        path_for_select.clone(),
                        order.as_ref().clone(),
                        cx,
                    )
                } else {
                    Dispatcher::select_file(id, path_for_select.clone(), cx)
                }
            })
        })
        .child(
            checkbox_tristate(
                SharedString::from(format!("include-{}", file.path)),
                include_value,
                false,
                cx,
            )
            .when_some(repo_id, move |d, id| {
                d.on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    Dispatcher::toggle_file_included(id, path_for_toggle.clone(), cx)
                })
            }),
        )
        .child(
            // GHD `PathText` keeps the file name visible and truncates the
            // directory part when the row is too narrow.
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .text_size(FONT_SIZE)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_secondary)
                        .child(file.directory().to_string()),
                )
                .child(
                    div()
                        .flex_none()
                        .max_w_full()
                        .truncate()
                        .child(file.file_name().to_string()),
                ),
        )
        .child(octicon(icon, color))
        .into_any_element()
}
