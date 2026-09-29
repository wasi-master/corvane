//! Changes sidebar: filter header, "N changed files" row, file list, commit form.
//! `styles/ui/changes/{_changes-list,_commit-message}.scss`.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use corvane_core::filter::{filtered_files, no_results_message, option_count};
use corvane_core::{
    AppState, Author, DiffSelectionType, Dispatcher, FileListFilter, FileStatusKind, FilterOption,
    Foldout, Popup, RepoRuleEnforced, RepoRulesMetadataFailures, RepoRulesMetadataStatus, Tip,
    UnknownAuthorState, WorkingDirectoryFileChange, failed_rules, legacy_stealth_email,
};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::{
    Copy, Cut, Enter, Escape, IndentInline, InlineToken, InputEvent, InputState, MoveDown, MoveUp,
    Paste, Redo, SelectAll, Textarea, TextareaState, Undo,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::IconButtonA11y;
use crate::widgets::ListRowA11y;

use crate::actions::{
    Commit, ExtendSelectionDown, ExtendSelectionUp, SelectAllFiles, SelectNextFile,
    SelectPreviousFile, SpellAddToDictionary, SpellSuggestion0, SpellSuggestion1, SpellSuggestion2,
    SpellSuggestion3, SpellSuggestion4, ToggleCoAuthors, ToggleCommitSpellcheck,
};
use crate::autocompletion::{self, Autocompletion, Hit, PickHandler};
use crate::context_menu::{ContextMenu, MenuItem};
use crate::diff_view::status_icon;
use crate::icons::{Octicon, octicon, spin};
use crate::relative_time::relative;
use crate::scrollbar::ScrollbarExt;
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
    /// GHD `AuthorInput`: the co-author token field under the description.
    CoAuthors,
}

/// Window-space rectangles of a field's misspellings (see `summary_rects`).
type RectCache = Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>;

/// One misspelled word of a commit-form field.
struct Misspelling {
    range: Range<usize>,
    word: String,
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
    /// `AuthorInput`: authors are inline tokens (id = login, lower-cased).
    co_authors: Entity<TextareaState>,
    state: Entity<AppState>,
    seen_commit_nonce: u64,
    seen_amend_nonce: u64,
    /// Repository and `commit.template` text the form was last prefilled for.
    seen_template: (Option<u64>, Option<String>),
    context_menu: Option<Entity<ContextMenu>>,
    /// GHD `ChangesListFilterOptions` popover.
    filter_popover_open: bool,
    filter_button_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Focus target for arrow-key navigation of the list.
    list_focus: FocusHandle,
    /// Keeps the row an arrow key moved to in view (`scrollRowToVisible`).
    list_scroll: UniformListScrollHandle,
    /// View › Hide Changes Filter (`isChangesFilterVisible`).
    filter_visible: bool,
    /// GHD `AutocompletingTextInput` state for whichever field has the popup.
    autocomplete: Option<(CommitField, Autocompletion)>,
    /// Misspelled words per field (`NSSpellChecker`), refreshed on change.
    summary_misspelled: Vec<Misspelling>,
    description_misspelled: Vec<Misspelling>,
    /// Window-space rectangles of those words, written by the overlay's
    /// prepaint. The context-menu builder runs while the kit holds the input
    /// entity, so it must read these instead of the input.
    summary_rects: RectCache,
    description_rects: RectCache,
    summary_focus: FocusHandle,
    description_focus: FocusHandle,
    co_authors_focus: FocusHandle,
    pending_spell: Option<PendingSpell>,
    /// A handle typed with a trailing space, turned into a token on the next
    /// render (needs a window).
    pending_author: Option<(Range<usize>, Author)>,
    /// `isRuleFailurePopoverOpen`: the commit-message rule failures popover.
    rule_failure_popover_open: bool,
    rule_hint_bounds: Rc<Cell<Bounds<Pixels>>>,
}

/// What the repository rules say about the commit being written
/// (`renderBranchProtectionsRepoRulesCommitWarning` inputs).
struct RulesSnapshot {
    html_url: String,
    branch: Option<String>,
    /// `aheadBehind === null`: the branch is unpublished.
    unpublished: bool,
    protected: bool,
    info: corvane_core::RepoRulesInfo,
    message_failures: RepoRulesMetadataFailures,
    author_failures: RepoRulesMetadataFailures,
    branch_failures: RepoRulesMetadataFailures,
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
                this.co_authors
                    .update(cx, |s, cx| s.set_value("", window, cx));
                this.summary_misspelled.clear();
                this.description_misspelled.clear();
                this.autocomplete = None;
                // the next commit starts from the template again
                let template = this.seen_template.1.clone();
                this.apply_commit_template(None, template, window, cx);
                cx.notify();
            }
            let template = {
                let s = state.read(cx);
                (
                    s.selected,
                    s.selected_state()
                        .and_then(|rs| rs.info.as_ref())
                        .and_then(|i| i.commit_template.clone()),
                )
            };
            if template != this.seen_template {
                let previous = std::mem::replace(&mut this.seen_template, template.clone()).1;
                this.apply_commit_template(previous, template.1, window, cx);
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
        // `AuthorInput` wraps its tokens and grows with them
        let co_authors = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .placeholder("@username")
        });
        cx.subscribe(&co_authors, |this, _, ev: &InputEvent, cx| {
            this.on_input_event(CommitField::CoAuthors, ev, cx)
        })
        .detach();
        let summary_focus = summary.read(cx).focus_handle(cx);
        let description_focus = description.read(cx).focus_handle(cx);
        let co_authors_focus = co_authors.read(cx).focus_handle(cx);
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
            co_authors,
            state,
            seen_commit_nonce: 0,
            seen_amend_nonce: 0,
            seen_template: (None, None),
            context_menu: None,
            filter_popover_open: false,
            filter_button_bounds: Rc::new(Cell::new(Bounds::default())),
            list_focus: cx.focus_handle(),
            list_scroll: UniformListScrollHandle::new(),
            filter_visible: true,
            autocomplete: None,
            summary_misspelled: Vec::new(),
            description_misspelled: Vec::new(),
            summary_rects: Rc::new(RefCell::new(Vec::new())),
            description_rects: Rc::new(RefCell::new(Vec::new())),
            summary_focus,
            description_focus,
            co_authors_focus,
            pending_spell: None,
            pending_author: None,
            rule_failure_popover_open: false,
            rule_hint_bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }

    // ---- autocompletion + spellcheck (GHD `AutocompletingTextInput`) ----

    fn on_input_event(&mut self, field: CommitField, ev: &InputEvent, cx: &mut Context<Self>) {
        match ev {
            InputEvent::Change if field == CommitField::CoAuthors => {
                self.sync_co_authors(cx);
                self.open_autocomplete(field, cx);
            }
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
            CommitField::CoAuthors => {
                let s = self.co_authors.read(cx);
                (s.value().to_string(), s.cursor())
            }
        }
    }

    fn field_focus_handle(&self, field: CommitField) -> FocusHandle {
        match field {
            CommitField::Summary => self.summary_focus.clone(),
            CommitField::Description => self.description_focus.clone(),
            CommitField::CoAuthors => self.co_authors_focus.clone(),
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
            CommitField::CoAuthors => self.co_authors.update(cx, |s, cx| {
                s.set_selected_range(range, cx);
                s.replace(text, window, cx);
            }),
        }
        let handle = self.field_focus_handle(field);
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
        if field == CommitField::CoAuthors {
            // the free text after the last token, `@handle`, caret at the end
            self.autocomplete = None;
            let free_start = self.co_author_free_start(cx).min(text.len());
            let free = &text[free_start..];
            let trimmed = free.trim_start();
            if let (Some(gh), Some(rest)) = (github.as_ref(), trimmed.strip_prefix('@'))
                && caret == text.len()
            {
                let start = free_start + (free.len() - trimmed.len()) + 1;
                let exclude = self.co_author_logins(cx);
                let hits = autocompletion::co_author_hits(&rest.to_lowercase(), gh, &exclude, cx);
                if !hits.is_empty() {
                    self.autocomplete = Some((
                        field,
                        Autocompletion {
                            kind: corvane_core::TriggerKind::User,
                            range: start..text.len(),
                            hits,
                            selected: None,
                            scroll: UniformListScrollHandle::new(),
                        },
                    ));
                }
            }
            cx.notify();
            return;
        }
        self.autocomplete =
            autocompletion::attempt(&text, caret, github.as_ref(), cx).map(|ac| (field, ac));
        cx.notify();
    }

    // ---- co-authors (GHD `AuthorInput`) ----

    /// Byte offset where the free text after the last author token starts.
    fn co_author_free_start(&self, cx: &App) -> usize {
        self.co_authors
            .read(cx)
            .tokens()
            .last()
            .map(|t| t.range().end)
            .unwrap_or(0)
    }

    fn co_author_logins(&self, cx: &App) -> Vec<String> {
        self.state
            .read(cx)
            .selected_state()
            .map(|rs| {
                rs.co_authors
                    .iter()
                    .filter_map(|a| a.username().map(|u| u.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// GHD `onAuthorsUpdated` after the tokens changed (backspace removed one),
    /// plus "Space at the end of the text adds the typed handle".
    fn sync_co_authors(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let ids: Vec<String> = self
            .co_authors
            .read(cx)
            .tokens()
            .iter()
            .map(|t| t.token().id().to_string())
            .collect();
        let current = self
            .state
            .read(cx)
            .selected_state()
            .map(|rs| rs.co_authors.clone())
            .unwrap_or_default();
        let kept: Vec<Author> = ids
            .iter()
            .filter_map(|id| {
                current
                    .iter()
                    .find(|a| a.username().is_some_and(|u| u.to_lowercase() == *id))
                    .cloned()
            })
            .collect();
        if kept != current {
            Dispatcher::set_co_authors(id, kept, cx);
        }
        // `onInputKeyDown`: Space at the end turns the typed handle into an author
        let (text, caret) = self.field_text_and_caret(CommitField::CoAuthors, cx);
        let free_start = self.co_author_free_start(cx).min(text.len());
        let free = &text[free_start..];
        if caret == text.len() && free.ends_with(' ') {
            let handle = free.trim().trim_start_matches('@').to_string();
            if !handle.is_empty() && !handle.contains(char::is_whitespace) {
                let author = Author::Unknown {
                    username: handle,
                    state: UnknownAuthorState::Searching,
                };
                self.pending_author = Some((free_start..text.len(), author));
                cx.notify();
            }
        }
    }

    /// Add an author as a token replacing `range` (GHD `onAutocompleteItemSelected`).
    fn add_co_author(
        &mut self,
        range: Range<usize>,
        author: Author,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let Some(login) = author.username().map(|u| u.to_lowercase()) else {
            return;
        };
        let already = self
            .co_author_logins(cx)
            .iter()
            .any(|u| u.eq_ignore_ascii_case(&login));
        let token = InlineToken::new(login.clone(), author.display_text())
            .with_label(author.display_text());
        let ok = self
            .co_authors
            .update(cx, |s, cx| {
                s.replace_range_with_token(range, token, window, cx)
            })
            .is_ok();
        if !ok || already {
            return;
        }
        let mut authors = self
            .state
            .read(cx)
            .selected_state()
            .map(|rs| rs.co_authors.clone())
            .unwrap_or_default();
        authors.push(author.clone());
        Dispatcher::set_co_authors(id, authors, cx);
        if let (Author::Unknown { username, .. }, Some(gh)) = (
            &author,
            self.state
                .read(cx)
                .repository(id)
                .and_then(|r| r.github.clone()),
        ) {
            Dispatcher::resolve_unknown_author(id, &gh, username.clone(), cx);
        }
        let handle = self.co_authors_focus.clone();
        window.focus(&handle, cx);
        cx.notify();
    }

    /// The "Add Co-Authors" / "Remove Co-Authors" toggle.
    fn toggle_co_authors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let show = self
            .state
            .read(cx)
            .selected_state()
            .is_some_and(|rs| rs.show_co_authored_by);
        Dispatcher::set_show_co_authored_by(id, !show, cx);
        if !show {
            let handle = self.co_authors_focus.clone();
            window.focus(&handle, cx);
        }
    }

    /// Unknown handles still in the list (`onConfirmCommitWithUnknownCoAuthors`).
    fn unknown_co_authors(&self, cx: &App) -> Vec<String> {
        self.state
            .read(cx)
            .selected_state()
            .filter(|rs| rs.show_co_authored_by)
            .map(|rs| {
                rs.co_authors
                    .iter()
                    .filter_map(|a| match a {
                        Author::Unknown { username, .. } => Some(username.clone()),
                        Author::Known { .. } => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `.author-input-component`: the label, the author tokens and the
    /// `@username` box, attached under the description container.
    fn co_author_input(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let authors: Rc<Vec<Author>> = Rc::new(
            self.state
                .read(cx)
                .selected_state()
                .map(|rs| rs.co_authors.clone())
                .unwrap_or_default(),
        );
        let focused = self.co_authors_focus.is_focused(window);
        let (tag_bg, tag_border, error_bg, error_border, error_text, sel_bg, sel_text, text) = (
            t.co_author_tag_background,
            t.co_author_tag_border,
            t.form_error_background,
            t.form_error_border,
            t.form_error_text,
            t.box_selected_active_background,
            t.box_selected_active_text,
            t.text,
        );
        div()
            .id("co-author-input")
            .flex()
            .flex_row()
            .items_start()
            .min_h(TEXT_FIELD_HEIGHT)
            .px(SPACING_HALF)
            .py(px(2.))
            .gap(px(2.))
            .border_1()
            .border_t_0()
            .rounded_b(BORDER_RADIUS)
            .bg(t.box_background)
            .border_color(if focused {
                t.focus
            } else {
                t.box_border_contrast
            })
            .cursor_text()
            .text_size(FONT_SIZE)
            .child(
                div()
                    .flex_none()
                    .h(TEXT_FIELD_HEIGHT - px(6.))
                    .flex()
                    .items_center()
                    .text_color(t.text_secondary)
                    .child("Co-Authors "),
            )
            .child(
                div().flex_1().min_w(px(80.)).child(
                    Textarea::new(&self.co_authors)
                        .appearance(false)
                        .xsmall()
                        .token(move |ctx, _window, cx| {
                            // `.handle`: known / progress / error / focused
                            let id = ctx.token().id().to_string();
                            let author = authors
                                .iter()
                                .find(|a| a.username().is_some_and(|u| u.to_lowercase() == id));
                            let unknown = match author {
                                Some(Author::Unknown { state, .. }) => Some(*state),
                                _ => None,
                            };
                            let (bg, border, fg) = if ctx.is_selected() {
                                (sel_bg, tag_border, sel_text)
                            } else {
                                match unknown {
                                    Some(UnknownAuthorState::Error) => {
                                        (error_bg, error_border, error_text)
                                    }
                                    Some(UnknownAuthorState::Searching) => {
                                        (gpui_kit::transparent_black(), tag_border, text)
                                    }
                                    None => (tag_bg, tag_border, text),
                                }
                            };
                            let title = match unknown {
                                Some(UnknownAuthorState::Error) => {
                                    Some(format!("Could not find user with username {}", id))
                                }
                                Some(UnknownAuthorState::Searching) => {
                                    Some(format!("Searching for @{id}"))
                                }
                                None => author.map(|a| a.full_text()),
                            };
                            let _ = cx;
                            div()
                                .id(SharedString::from(format!("co-author-{id}")))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(3.))
                                .px(px(2.))
                                .mx(px(2.))
                                .rounded(BORDER_RADIUS)
                                .border_1()
                                .border_color(border)
                                .bg(bg)
                                .text_color(fg)
                                .cursor_pointer()
                                .when_some(title, |d, title| {
                                    d.tooltip(crate::widgets::tooltip(title))
                                })
                                .child(ctx.token().label().clone())
                                .when(unknown == Some(UnknownAuthorState::Searching), |d| {
                                    d.child(spin(
                                        octicon(Octicon::SyncClockwise, fg).size(px(9.)),
                                        "co-author-searching",
                                    ))
                                })
                                .when(unknown == Some(UnknownAuthorState::Error), |d| {
                                    d.child(octicon(Octicon::Stop, fg).size(px(9.)))
                                })
                        }),
                ),
            )
            .into_any_element()
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
        if field == CommitField::CoAuthors {
            let endpoint = self
                .state
                .read(cx)
                .selected
                .and_then(|id| self.state.read(cx).repository(id).cloned())
                .and_then(|r| r.github.map(|gh| gh.endpoint))
                .unwrap_or_else(|| "https://api.github.com".to_string());
            let author = match hit {
                Hit::User(u) => Author::Known {
                    name: u.name.clone().unwrap_or_else(|| u.login.clone()),
                    email: u
                        .email
                        .clone()
                        .filter(|e| !e.is_empty())
                        .unwrap_or_else(|| legacy_stealth_email(&u.login, &endpoint)),
                    username: Some(u.login.clone()),
                },
                Hit::UnknownUser(name) => Author::Unknown {
                    username: name.clone(),
                    state: UnknownAuthorState::Searching,
                },
                _ => return,
            };
            self.add_co_author(range, author, window, cx);
            return;
        }
        let text = format!("{} ", hit.completion_text());
        self.replace_range(field, range, text, window, cx);
    }

    fn refresh_spelling(&mut self, field: CommitField, cx: &mut Context<Self>) {
        let enabled = self.state.read(cx).settings.commit_spellcheck_enabled;
        let items = if enabled {
            let (text, _) = self.field_text_and_caret(field, cx);
            corvane_platform::spell::misspelled_ranges(&text)
                .into_iter()
                .map(|range| Misspelling {
                    word: text.get(range.clone()).unwrap_or("").to_string(),
                    range,
                })
                .collect()
        } else {
            Vec::new()
        };
        match field {
            CommitField::Summary => {
                self.summary_misspelled = items;
                self.summary_rects.borrow_mut().clear();
            }
            CommitField::Description => {
                self.description_misspelled = items;
                self.description_rects.borrow_mut().clear();
            }
            CommitField::CoAuthors => {}
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
            CommitField::CoAuthors => return None,
        };
        Some(Bounds {
            origin: point(bounds.origin.x + scroll.x, bounds.origin.y),
            size: bounds.size,
        })
    }

    /// The misspelled word under `position`, from the last painted
    /// rectangles (no input reads: see `summary_rects`).
    fn misspelled_at(
        &self,
        field: CommitField,
        position: Point<Pixels>,
    ) -> Option<(Range<usize>, String)> {
        let (items, rects) = match field {
            CommitField::Summary => (&self.summary_misspelled, self.summary_rects.borrow()),
            CommitField::Description => (
                &self.description_misspelled,
                self.description_rects.borrow(),
            ),
            CommitField::CoAuthors => return None,
        };
        items
            .iter()
            .zip(rects.iter())
            .find(|(_, rect)| rect.is_some_and(|rect| rect.contains(&position)))
            .map(|(m, _)| (m.range.clone(), m.word.clone()))
    }

    /// Red dotted underline beneath each misspelled word (Chromium's marker).
    fn spell_overlay(&self, field: CommitField, cx: &Context<Self>) -> Option<AnyElement> {
        let (ranges, rects_cell): (Vec<Range<usize>>, RectCache) = match field {
            CommitField::Summary => (
                self.summary_misspelled
                    .iter()
                    .map(|m| m.range.clone())
                    .collect(),
                self.summary_rects.clone(),
            ),
            CommitField::Description => (
                self.description_misspelled
                    .iter()
                    .map(|m| m.range.clone())
                    .collect(),
                self.description_rects.clone(),
            ),
            CommitField::CoAuthors => return None,
        };
        if ranges.is_empty() {
            return None;
        }
        let color = cx.ghd().error;
        let weak = cx.weak_entity();
        Some(
            canvas(
                move |_, _, cx| {
                    let rects: Vec<Option<Bounds<Pixels>>> = weak
                        .upgrade()
                        .map(|this| {
                            let this = this.read(cx);
                            ranges
                                .iter()
                                .map(|r| this.range_rect(field, r, cx))
                                .collect()
                        })
                        .unwrap_or_default();
                    *rects_cell.borrow_mut() = rects.clone();
                    rects
                },
                move |bounds, rects, window, _| {
                    window.with_content_mask(Some(ContentMask { bounds }), |window| {
                        for rect in rects.into_iter().flatten() {
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
    ///
    /// The kit invokes the builder while it holds the input entity, so nothing
    /// in here may read the `InputState` (that panics inside AppKit's event
    /// handler and aborts the process).
    fn input_menu(&self, field: CommitField, cx: &Context<Self>) -> InputMenuBuilder {
        let weak = cx.weak_entity();
        let focus = self.field_focus_handle(field);
        Rc::new(move |mut menu, window, cx| {
            let mut suggestions: Option<Vec<String>> = None;
            let mut enabled = true;
            let mut co_authors: Option<(&'static str, bool)> = None;
            // the menu's actions dispatch through the focused element
            window.focus(&focus, cx);
            let updated = weak.update(cx, |this, cx| {
                this.pending_spell = None;
                enabled = this.state.read(cx).settings.commit_spellcheck_enabled;
                // `getAddRemoveCoAuthorsMenuItem`
                let s = this.state.read(cx);
                if let Some(id) = s.selected
                    && s.repository(id).is_some_and(|r| r.github.is_some())
                {
                    let rs = s.selected_state();
                    let show = rs.is_some_and(|rs| rs.show_co_authored_by);
                    let committing = rs.is_some_and(|rs| rs.committing);
                    co_authors = Some((
                        if show {
                            "Remove Co-Authors"
                        } else {
                            "Add Co-Authors"
                        },
                        !committing,
                    ));
                }
                let position = window.mouse_position();
                tracing::debug!(
                    ?position,
                    rects = ?match field {
                        CommitField::Summary => this.summary_rects.borrow().clone(),
                        CommitField::Description => this.description_rects.borrow().clone(),
                        CommitField::CoAuthors => Vec::new(),
                    },
                    "commit input context menu"
                );
                if let Some((range, word)) = this.misspelled_at(field, position) {
                    let guesses = corvane_platform::spell::guesses(&word);
                    this.pending_spell = Some(PendingSpell {
                        field,
                        range,
                        word,
                        suggestions: guesses.clone(),
                    });
                    suggestions = Some(guesses);
                }
            });
            if updated.is_err() {
                tracing::warn!("commit input context menu: sidebar entity unavailable");
            }
            if let Some((label, enabled)) = co_authors {
                menu = menu
                    .menu_with_disabled(label, !enabled, Box::new(ToggleCoAuthors))
                    .separator();
            }
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
                .menu("Cut", Box::new(Cut))
                .menu("Copy", Box::new(Copy))
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
    /// Prefill the description with the repository's `commit.template`
    /// (`corvane_git::commit_template`) while the form is untouched: summary
    /// empty and the description empty or still holding the `previous`
    /// template text.
    fn apply_commit_template(
        &mut self,
        previous: Option<String>,
        template: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.summary.read(cx).value().is_empty() {
            return;
        }
        let description = self.description.read(cx).value().to_string();
        let untouched = description.is_empty() || previous.as_deref() == Some(description.as_str());
        if !untouched {
            return;
        }
        let text = template.unwrap_or_default();
        if text == description {
            return;
        }
        self.description
            .update(cx, |s, cx| s.set_value(text, window, cx));
        self.refresh_spelling(CommitField::Description, cx);
        cx.notify();
    }

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
            // GHD `moveSelection` starts from the last selected row: the
            // moving end of a ⇧-arrow range
            (
                id,
                s.selected_state().and_then(|rs| {
                    rs.selected_files
                        .last()
                        .or(rs.selected_file.as_ref())
                        .cloned()
                }),
            )
        };
        let index = current
            .and_then(|p| files.iter().position(|f| f.path == p))
            .map(|i| i as isize + delta)
            .unwrap_or(0)
            .clamp(0, files.len() as isize - 1) as usize;
        Dispatcher::select_file(id, files[index].path.clone(), cx);
        self.list_scroll
            .scroll_to_item(index, ScrollStrategy::Nearest);
    }

    /// ⇧↑ / ⇧↓: extend the range selection (GHD `List.addSelection`).
    fn extend_relative(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (files, _) = self.visible_files(cx);
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let order: Vec<String> = files.into_iter().map(|f| f.path).collect();
        Dispatcher::extend_file_selection_by(id, delta, order.clone(), cx);
        let end = self
            .state
            .read(cx)
            .selected_state()
            .and_then(|rs| rs.selected_files.last().cloned());
        if let Some(index) = end.and_then(|p| order.iter().position(|o| *o == p)) {
            self.list_scroll
                .scroll_to_item(index, ScrollStrategy::Nearest);
        }
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
                                                .icon_button_label("Close")
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
        let unknown = self.unknown_co_authors(cx);
        if !unknown.is_empty() {
            Dispatcher::show_popup(
                Popup::UnknownAuthors {
                    repo: id,
                    usernames: unknown,
                    summary,
                    description,
                },
                cx,
            );
            return;
        }
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
                            let active_count = self.filter_options(cx).count_active();
                            let active = active_count > 0;
                            // `buttonTextLabel`
                            let filter_label = if active {
                                format!("Filter Options ({active_count} applied)")
                            } else {
                                "Filter Options".to_string()
                            };
                            let bounds_cell = self.filter_button_bounds.clone();
                            div()
                                .id("filter-options")
                                .icon_button_label(filter_label)
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
                .min_h_0()
                .with_scrollbar_handle(&self.list_scroll),
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

    /// The repository rules that apply to the commit form, `None` for
    /// repositories without a GitHub remote.
    fn rules_snapshot(&self, cx: &App) -> Option<RulesSnapshot> {
        let s = self.state.read(cx);
        let id = s.selected?;
        let github = s.repository(id)?.github.as_ref()?;
        let rs = s.repo_states.get(&id)?;
        let info = rs.repo_rules.clone();
        let branch = rs
            .info
            .as_ref()
            .and_then(|i| i.current_branch())
            .map(|b| b.name.clone());
        // `formatCommitMessage`: summary, blank line, description, trailers
        let summary = self.summary.read(cx).value().trim().to_string();
        let description = self.description.read(cx).value().trim().to_string();
        let message = if description.is_empty() {
            format!("{summary}\n")
        } else {
            format!("{summary}\n\n{description}\n")
        };
        // `getCoAuthorTrailers`, merged like `mergeTrailers`
        let trailers: Vec<(String, String)> = if rs.show_co_authored_by {
            rs.co_authors
                .iter()
                .filter_map(|a| a.trailer_value())
                .map(|v| ("Co-Authored-By".to_string(), v))
                .collect()
        } else {
            Vec::new()
        };
        let message = corvane_core::append_trailers(&message, &trailers);
        let message_failures = if summary.is_empty() {
            RepoRulesMetadataFailures::default()
        } else {
            failed_rules(&info.commit_message_patterns, &message)
        };
        let author_failures = rs
            .info
            .as_ref()
            .and_then(|i| i.identity.email.as_deref())
            .map(|email| failed_rules(&info.commit_author_email_patterns, email))
            .unwrap_or_default();
        let branch_failures = branch
            .as_deref()
            .map(|b| failed_rules(&info.branch_name_patterns, b))
            .unwrap_or_default();
        Some(RulesSnapshot {
            html_url: github.html_url.clone(),
            branch,
            unpublished: rs.ahead_behind.is_none(),
            protected: rs.current_branch_protected,
            info,
            message_failures,
            author_failures,
            branch_failures,
        })
    }

    /// `hasRepoRuleFailure`
    fn has_repo_rule_failure(&self, cx: &App) -> bool {
        let Some(rules) = self.rules_snapshot(cx) else {
            return false;
        };
        rules.info.basic_commit_warning == RepoRuleEnforced::Yes
            || rules.info.signed_commits_required == RepoRuleEnforced::Yes
            || rules.info.pull_request_required == RepoRuleEnforced::Yes
            || rules.message_failures.status() == RepoRulesMetadataStatus::Fail
            || rules.author_failures.status() == RepoRulesMetadataStatus::Fail
            || (rules.unpublished
                && (rules.info.creation_restricted == RepoRuleEnforced::Yes
                    || rules.branch_failures.status() == RepoRulesMetadataStatus::Fail))
    }

    /// `CommitWarning`: an icon centred on a rule above a centred message.
    fn commit_warning(
        &self,
        icon: Octicon,
        color: Hsla,
        message: AnyElement,
        cx: &App,
    ) -> AnyElement {
        let t = cx.ghd();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .mb(SPACING)
            .bg(t.box_alt_background)
            .child(
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
                            .child(octicon(icon, color)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(3.))
                    .text_size(FONT_SIZE)
                    .text_color(t.text_secondary)
                    .child(message),
            )
            .into_any_element()
    }

    /// `showNoWriteAccess` (with changed files): "You don't have write access
    /// to <repo>. Want to create a fork?", shown before any branch warning.
    fn no_write_access_warning(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let (id, name) = {
            let s = self.state.read(cx);
            let id = s.selected?;
            let repo = s.repository(id)?;
            let github = repo.github.as_ref()?;
            let files = s.selected_state()?.status.as_ref()?.files.len();
            if github.has_write_permission() || files == 0 {
                return None;
            }
            (id, repo.name())
        };
        Some(
            self.commit_warning(
                Octicon::Alert,
                t.dialog_warning,
                crate::widgets::paragraph(vec![
                    "You don't have write access to ".into(),
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(name)
                        .into_any_element()
                        .into(),
                    ". Want to ".into(),
                    crate::widgets::link_button("commit-warning-create-fork", "create a fork", cx)
                        .on_click(move |_, _, cx| Dispatcher::show_create_fork_dialog(id, cx))
                        .into_any_element()
                        .into(),
                    "?".into(),
                ])
                .justify_center()
                .into_any_element(),
                cx,
            ),
        )
    }

    /// `renderBranchProtectionsRepoRulesCommitWarning`
    fn branch_protection_warning(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let rules = self.rules_snapshot(cx)?;
        let branch = rules.branch.clone()?;
        let bold = |text: String| {
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(text)
                .into_any_element()
        };
        let switch_link = || {
            crate::widgets::link_button("commit-warning-switch", "switch branches", cx)
                .on_click(|_, _, cx| Dispatcher::toggle_foldout(Foldout::Branch, cx))
                .into_any_element()
        };
        let rulesets_link = |label: &'static str| {
            let url = format!(
                "{}/rules/?ref={}",
                rules.html_url,
                corvane_core::integrations::encode_component(&format!("refs/heads/{branch}"))
            );
            crate::widgets::link_button("commit-warning-rulesets", label, cx)
                .on_click(move |_, _, cx| cx.open_url(&url))
                .into_any_element()
        };
        let message = |parts: Vec<AnyElement>| {
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .justify_center()
                .gap(px(3.))
                .children(parts)
                .into_any_element()
        };
        if rules.protected {
            return Some(self.commit_warning(
                Octicon::Alert,
                t.dialog_warning,
                message(vec![
                    bold(branch.clone()),
                    div().child("is a protected branch. Want to").into_any_element(),
                    switch_link(),
                    div().child("?").into_any_element(),
                ]),
                cx,
            ));
        }
        // which rule warning to show: enforced ones first, then bypassable
        let publish = if rules.unpublished {
            if rules.info.creation_restricted == RepoRuleEnforced::Yes
                || rules.branch_failures.status() == RepoRulesMetadataStatus::Fail
            {
                RepoRuleEnforced::Yes
            } else if rules.info.creation_restricted == RepoRuleEnforced::Bypass
                || rules.branch_failures.status() == RepoRulesMetadataStatus::Bypass
            {
                RepoRuleEnforced::Bypass
            } else {
                RepoRuleEnforced::No
            }
        } else {
            RepoRuleEnforced::No
        };
        let statuses = [
            ("publish", publish),
            ("signing", rules.info.signed_commits_required),
            ("basic", rules.info.basic_commit_warning),
        ];
        let warning = statuses
            .iter()
            .find(|(_, e)| *e == RepoRuleEnforced::Yes)
            .or_else(|| {
                statuses
                    .iter()
                    .find(|(_, e)| *e == RepoRuleEnforced::Bypass)
            })
            .copied()?;
        let can_bypass = warning.1 == RepoRuleEnforced::Bypass;
        let (icon, color) = if can_bypass {
            (Octicon::Alert, t.dialog_warning)
        } else {
            (Octicon::Stop, t.dialog_error)
        };
        let bypass_tail = || {
            if can_bypass {
                vec![
                    div()
                        .child(", but you can bypass them. Proceed with caution!")
                        .into_any_element(),
                ]
            } else {
                vec![
                    div().child(". Want to").into_any_element(),
                    switch_link(),
                    div().child("?").into_any_element(),
                ]
            }
        };
        let parts = match warning.0 {
            "publish" => {
                let mut parts = vec![
                    div().child("The branch name").into_any_element(),
                    bold(branch.clone()),
                    div().child("fails").into_any_element(),
                    rulesets_link("one or more rules"),
                    div()
                        .child(format!(
                            "that {} prevent it from being published",
                            if can_bypass { "would" } else { "will" }
                        ))
                        .into_any_element(),
                ];
                parts.extend(bypass_tail());
                parts
            }
            "signing" => vec![
                rulesets_link("One or more rules"),
                div().child("apply to the branch").into_any_element(),
                bold(branch.clone()),
                div()
                    .child(format!(
                        "that require signed commits{}",
                        if can_bypass {
                            ", but you can bypass them. Proceed with caution!"
                        } else {
                            "."
                        }
                    ))
                    .into_any_element(),
                crate::widgets::link_button(
                    "commit-warning-signing-docs",
                    "Learn more about commit signing.",
                    cx,
                )
                .on_click(|_, _, cx| {
                    cx.open_url(
                        "https://docs.github.com/authentication/managing-commit-signature-verification/signing-commits",
                    )
                })
                .into_any_element(),
            ],
            _ => {
                let mut parts = vec![
                    rulesets_link("One or more rules"),
                    div().child("apply to the branch").into_any_element(),
                    bold(branch.clone()),
                    div()
                        .child(format!(
                            "that {} prevent pushing",
                            if can_bypass { "would" } else { "will" }
                        ))
                        .into_any_element(),
                ];
                parts.extend(bypass_tail());
                parts
            }
        };
        Some(self.commit_warning(icon, color, message(parts), cx))
    }

    /// `renderRepoRuleCommitMessageFailureHint`: the stop / alert button at
    /// the end of the summary box.
    fn rule_failure_hint(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let rules = self.rules_snapshot(cx)?;
        let status = rules.message_failures.status();
        if status == RepoRulesMetadataStatus::Pass {
            return None;
        }
        let can_bypass = status == RepoRulesMetadataStatus::Bypass;
        let bounds = self.rule_hint_bounds.clone();
        Some(
            div()
                .id("commit-message-failure-hint")
                .absolute()
                .right(px(6.))
                .top(px(4.))
                .cursor_pointer()
                .tooltip(crate::widgets::tooltip(if can_bypass {
                    "Warning: Commit message fails repository rules, but you can bypass them. View details."
                } else {
                    "Error: Commit message fails repository rules. View details."
                }))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.rule_failure_popover_open = !this.rule_failure_popover_open;
                    cx.notify();
                }))
                .child(
                    canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                        .absolute()
                        .inset_0(),
                )
                .child(if can_bypass {
                    octicon(Octicon::Alert, t.dialog_warning)
                } else {
                    octicon(Octicon::Stop, t.dialog_error)
                })
                .into_any_element(),
        )
    }

    /// `renderRuleFailurePopover` + `RepoRulesMetadataFailureList`.
    fn rule_failure_popover(&self, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let rules = self.rules_snapshot(cx)?;
        let branch = rules.branch.clone()?;
        let failures = &rules.message_failures;
        if failures.status() == RepoRulesMetadataStatus::Pass {
            return None;
        }
        let anchor = self.rule_hint_bounds.get();
        let viewport = window.viewport_size();
        let width = px(360.);
        let x = (anchor.origin.x + anchor.size.width + px(8.)).min(viewport.width - width - px(8.));
        let y = (anchor.origin.y - px(20.)).max(px(8.));
        let total = failures.total();
        let end_text = if failures.status() == RepoRulesMetadataStatus::Bypass {
            format!(
                ", but you can bypass {}. Proceed with caution!",
                if total == 1 { "it" } else { "them" }
            )
        } else {
            ".".to_string()
        };
        let all_url = format!(
            "{}/rules/?ref={}",
            rules.html_url,
            corvane_core::integrations::encode_component(&format!("refs/heads/{branch}"))
        );
        let html_url = rules.html_url.clone();
        let list = |label: &'static str, items: &[corvane_core::RepoRulesMetadataFailure]| {
            if items.is_empty() {
                return None;
            }
            Some(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{label} Rules:")),
                    )
                    .children(items.iter().enumerate().map(|(ix, f)| {
                        let url = format!("{html_url}/rules/{}", f.ruleset_id);
                        div()
                            .flex()
                            .flex_row()
                            .gap(SPACING_HALF)
                            .pl(SPACING_DOUBLE)
                            .child("•")
                            .child(
                                crate::widgets::link_button(
                                    SharedString::from(format!("rule-{label}-{ix}")),
                                    f.description.clone(),
                                    cx,
                                )
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                            )
                    })),
            )
        };
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("rule-failure-layer")
                        .relative()
                        .w(viewport.width)
                        .h(viewport.height)
                        .child(
                            div()
                                .id("rule-failure-overlay")
                                .absolute()
                                .inset_0()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.rule_failure_popover_open = false;
                                        cx.notify();
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .id("rule-failure-popover")
                                .absolute()
                                .left(x)
                                .top(y)
                                .w(width)
                                .min_h(px(200.))
                                .p(SPACING)
                                .flex()
                                .flex_col()
                                .gap(SPACING)
                                .bg(t.box_background)
                                .text_color(t.text)
                                .text_size(FONT_SIZE)
                                .border_1()
                                .border_color(t.box_border)
                                .rounded(BORDER_RADIUS)
                                .shadow_lg()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .text_size(FONT_SIZE_MD)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Commit Message Rule Failures"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .flex_wrap()
                                        .gap(px(3.))
                                        .child(format!(
                                            "This commit message fails {total} rule{}{end_text}",
                                            if total > 1 { "s" } else { "" }
                                        ))
                                        .child(
                                            crate::widgets::link_button(
                                                "rule-failure-all",
                                                "View all rulesets for this branch.",
                                                cx,
                                            )
                                            .on_click(move |_, _, cx| cx.open_url(&all_url)),
                                        ),
                                )
                                .children(list("Failed", &failures.failed))
                                .children(list("Bypassed", &failures.bypassed)),
                        ),
                ),
            )
            .with_priority(12)
            .into_any_element(),
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
            || self.has_repo_rule_failure(cx)
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
        let (is_github, co_authors_visible) = {
            let s = self.state.read(cx);
            let is_github = s
                .selected
                .and_then(|id| s.repository(id))
                .is_some_and(|r| r.github.is_some());
            let show = s.selected_state().is_some_and(|rs| rs.show_co_authored_by);
            (is_github, is_github && show)
        };
        // Autocompletion popup anchored at the caret's bottom-left.
        let popup = self.autocomplete.as_ref().and_then(|(field, ac)| {
            let (bounds, line_height) = match field {
                CommitField::Summary => self.summary.read(cx).cursor_layout()?,
                CommitField::Description => self.description.read(cx).cursor_layout()?,
                CommitField::CoAuthors => self.co_authors.read(cx).cursor_layout()?,
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
                } else if this.co_authors_focus.is_focused(window) {
                    // the author field is one logical line, it only wraps
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
            .on_action(cx.listener(|this, _: &ToggleCoAuthors, window, cx| {
                this.toggle_co_authors(window, cx)
            }))
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
                        .children(self.spell_overlay(CommitField::Summary, cx))
                        .children(self.rule_failure_hint(cx)),
                    ),
            )
            .child(
                // `.description-focus-container`: textarea + action bar
                div()
                    .flex()
                    .flex_col()
                    .when(!co_authors_visible, |d| d.mb(SPACING))
                    .border_1()
                    .border_color(t.box_border_contrast)
                    .rounded_t(BORDER_RADIUS)
                    .when(!co_authors_visible, |d| d.rounded_b(BORDER_RADIUS))
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
                            .when(is_github, |d| {
                                // `.co-authors-toggle`
                                let toggle_label = if co_authors_visible {
                                    "Remove Co-Authors"
                                } else {
                                    "Add Co-Authors"
                                };
                                let color = if co_authors_visible {
                                    t.link
                                } else {
                                    t.text_secondary
                                };
                                let hover = if co_authors_visible {
                                    t.link_hover
                                } else {
                                    t.text
                                };
                                d.child(
                                    div()
                                        .id("co-authors-toggle")
                                        .size(px(18.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .text_color(color)
                                        .hover(move |s| s.text_color(hover))
                                        .a11y_button(toggle_label)
                                        .tooltip(crate::widgets::tooltip(toggle_label))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.toggle_co_authors(window, cx)
                                        }))
                                        .child(octicon(Octicon::PersonAdd, color)),
                                )
                                .child(div().w(px(1.)).h(px(16.)).bg(t.box_border_contrast))
                            })
                            .child(
                                div()
                                    .id("commit-options-button")
                                    .icon_button_label("Configure commit options")
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
            .when(co_authors_visible, |d| {
                d.child(div().mb(SPACING).child(self.co_author_input(window, cx)))
            })
            .children(self.amend_notice(cx))
            .children(
                self.no_write_access_warning(cx)
                    .or_else(|| self.branch_protection_warning(cx)),
            )
            .children(
                self.rule_failure_popover_open
                    .then(|| self.rule_failure_popover(window, cx))
                    .flatten(),
            )
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
        if let Some((range, author)) = self.pending_author.take() {
            self.add_co_author(range, author, window, cx);
        }
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
                    .on_action(cx.listener(|this, _: &ExtendSelectionDown, _, cx| {
                        this.extend_relative(1, cx)
                    }))
                    .on_action(cx.listener(|this, _: &ExtendSelectionUp, _, cx| {
                        this.extend_relative(-1, cx)
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
        .a11y_row(
            format!(
                "{}, {}{}",
                file.path,
                crate::widgets::status_label(file.status.kind),
                match include_value {
                    Some(true) => "",
                    Some(false) => ", not included",
                    None => ", partially included",
                }
            ),
            is_selected,
        )
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
