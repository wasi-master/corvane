//! History-operation dialogs: `ui/reset/warning-before-reset.tsx`,
//! `ui/checkout/confirm-checkout-commit.tsx`, `ui/create-tag/create-tag-dialog.tsx`,
//! `ui/undo/warn-local-changes-before-undo.tsx`.
//!
//! Deviation (`.docs/deviations.md` › History): Create a Tag has an
//! optional Message field (flag `823`); GHD always tags with an empty message.
//! Undoing a tagged commit warns first (flag `819`); ⌘⏎ submits Create a Tag
//! from its Message field (flag `824`). A pushed tag can be deleted, from the
//! remote too, after a confirmation (flag `826`).

use corvane_core::{AppState, Dispatcher, UnreachableCommitsTab};
use gpui_kit::component::input::{InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox, text_box};

/// `WarningBeforeReset`
pub struct ResetToCommitDialog {
    repo: u64,
    sha: String,
}

impl ResetToCommitDialog {
    pub fn new(repo: u64, sha: String) -> Self {
        Self { repo, sha }
    }
}

impl Render for ResetToCommitDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, sha) = (self.repo, self.sha.clone());
        dialog_with_kind(
            "dialog-reset-to-commit",
            DialogKind::Warning,
            "Reset to Commit",
            div().child(
                "You have changes in progress. Resetting to a previous commit might result in \
                 some of these changes being lost. Do you want to continue anyway?",
            ),
            vec![
                DialogButton {
                    id: "reset-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "reset-continue",
                    label: "Continue".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::reset_to_commit(repo, sha.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// Corvane addition (`261-reset-to-remote`): confirm resetting the current
/// branch to its upstream.
pub struct ResetToRemoteDialog {
    repo: u64,
    branch: String,
    upstream: String,
    ahead: usize,
    dirty: bool,
}

impl ResetToRemoteDialog {
    pub fn new(repo: u64, branch: String, upstream: String, ahead: usize, dirty: bool) -> Self {
        Self {
            repo,
            branch,
            upstream,
            ahead,
            dirty,
        }
    }
}

impl Render for ResetToRemoteDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, upstream) = (self.repo, self.upstream.clone());
        let mut lost = Vec::new();
        if self.ahead > 0 {
            lost.push(if self.ahead == 1 {
                "1 commit that is not on the remote".to_string()
            } else {
                format!("{} commits that are not on the remote", self.ahead)
            });
        }
        if self.dirty {
            lost.push("all uncommitted changes".to_string());
        }
        let mut text = format!("{} will be reset to match {}.", self.branch, self.upstream);
        if !lost.is_empty() {
            text.push_str(&format!(
                " This discards {}. Do you want to continue?",
                lost.join(" and ")
            ));
        }
        dialog_with_kind(
            "dialog-reset-to-remote",
            DialogKind::Warning,
            format!("Reset to {}", self.upstream),
            div().child(text),
            vec![
                DialogButton {
                    id: "reset-remote-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "reset-remote-continue",
                    label: "Reset".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::reset_to_remote(repo, upstream.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `ConfirmCheckoutCommit`
pub struct CheckoutCommitDialog {
    repo: u64,
    sha: String,
    dont_show_again: bool,
}

impl CheckoutCommitDialog {
    pub fn new(repo: u64, sha: String) -> Self {
        Self {
            repo,
            sha,
            dont_show_again: false,
        }
    }
}

impl Render for CheckoutCommitDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, sha, dont_show_again) = (self.repo, self.sha.clone(), self.dont_show_again);
        let content = div()
            .flex()
            .flex_col()
            .child(div().mb(SPACING()).child(
                "Checking out a commit will create a detached HEAD, and you will no longer be on \
                 any branch. Are you sure you want to checkout this commit?",
            ))
            .child(
                div()
                    .id("checkout-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "checkout-dont-show-box",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-checkout-commit",
            DialogKind::Warning,
            "Checkout Commit?",
            content,
            vec![
                DialogButton {
                    id: "checkout-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "checkout-ok",
                    label: "Checkout".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_checkout_commit = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::checkout_commit(repo, sha.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `CreateTag`
pub struct CreateTagDialog {
    repo: u64,
    sha: String,
    name: Entity<InputState>,
    /// Flag `823`: the annotated tag's message.
    message: Entity<TextareaState>,
}

/// GHD `MaxTagNameLength`
const MAX_TAG_NAME_LENGTH: usize = 245;

impl CreateTagDialog {
    pub fn new(repo: u64, sha: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        // `RefNameTextBox` autoFocus
        let handle = name.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        let message = cx.new(|cx| TextareaState::new(window, cx).rows(4));
        // ⏎ in Name submits the form, as GHD's `<form onSubmit>`; `824`: ⌘⏎
        // submits from the Message field too
        let cmd_enter = |cx: &App| {
            AppState::global(cx)
                .read(cx)
                .flags
                .bool(corvane_core::flags::ids::CMD_ENTER_SUBMITS_CREATE_TAG)
        };
        cx.subscribe(&name, move |this, _, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { secondary, .. } = ev
                && (!secondary || cmd_enter(cx))
            {
                this.submit(cx);
            }
        })
        .detach();
        cx.subscribe(&message, move |this, _, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter {
                secondary: true, ..
            } = ev
                && cmd_enter(cx)
            {
                this.submit(cx);
            }
        })
        .detach();
        Self {
            repo,
            sha,
            name,
            message,
        }
    }

    /// The trimmed name and its error (`getCurrentError`).
    fn name_and_error(&self, cx: &App) -> (String, Option<String>) {
        let name = self.name.read(cx).value().trim().to_string();
        let error = (name.len() > MAX_TAG_NAME_LENGTH).then(|| {
            format!("The tag name cannot be longer than {MAX_TAG_NAME_LENGTH} characters")
        });
        (name, error)
    }

    /// The message, empty unless flag `823` shows the Message field.
    fn message_text(&self, cx: &App) -> String {
        if AppState::global(cx)
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::TAG_MESSAGE)
        {
            self.message.read(cx).value().trim().to_string()
        } else {
            String::new()
        }
    }

    /// `createTag`
    fn submit(&mut self, cx: &mut Context<Self>) {
        let (name, error) = self.name_and_error(cx);
        if error.is_some() || name.is_empty() {
            return;
        }
        let message = self.message_text(cx);
        Dispatcher::close_popup(cx);
        Dispatcher::create_tag(self.repo, name, self.sha.clone(), message, cx);
    }
}

impl Render for CreateTagDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (name, error) = self.name_and_error(cx);
        let disabled = error.is_some() || name.is_empty();
        let this = cx.weak_entity();
        let with_message = AppState::global(cx)
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::TAG_MESSAGE);
        let t = cx.ghd();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING_HALF())
            .when_some(error, |d, message| {
                d.child(
                    div()
                        .mb(SPACING_HALF())
                        .text_color(t.form_error_text)
                        .child(message),
                )
            })
            .child(div().child("Name"))
            .child(text_box("tag-name", &self.name, None, window, cx))
            .when(with_message, |d| {
                d.child(div().mt(SPACING_HALF()).child("Message (optional)"))
                    .child(
                        div()
                            .border_1()
                            .border_color(t.box_border_contrast)
                            .rounded(BORDER_RADIUS())
                            .bg(t.box_background)
                            .overflow_hidden()
                            .child(Textarea::new(&self.message)),
                    )
            });
        dialog(
            "dialog-create-tag",
            "Create a Tag",
            content,
            vec![
                DialogButton {
                    id: "tag-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "tag-create",
                    label: "Create Tag".into(),
                    primary: true,
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if !disabled {
                            this.update(cx, |this, cx| this.submit(cx)).ok();
                        }
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `WarnLocalChangesBeforeUndo`
pub struct WarnLocalChangesBeforeUndoDialog {
    repo: u64,
    dont_show_again: bool,
}

impl WarnLocalChangesBeforeUndoDialog {
    pub fn new(repo: u64) -> Self {
        Self {
            repo,
            dont_show_again: false,
        }
    }
}

impl Render for WarnLocalChangesBeforeUndoDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, dont_show_again) = (self.repo, self.dont_show_again);
        let content = div()
            .flex()
            .flex_col()
            .child(div().mb(SPACING()).child(
                "You have changes in progress. Undoing the commit might result in some of these \
                 changes being lost. Do you want to continue anyway?",
            ))
            .child(
                div()
                    .id("undo-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "undo-dont-show-box",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-warn-undo",
            DialogKind::Warning,
            "Undo Commit",
            content,
            vec![
                DialogButton {
                    id: "undo-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "undo-continue",
                    label: "Continue".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_undo_commit = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::undo_commit(repo, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// Flag `819`: the commit being undone carries tags, which would be left on
/// a commit no branch contains (Corvane addition; GHD undoes silently).
pub struct WarnTaggedCommitBeforeUndoDialog {
    repo: u64,
    tags: Vec<String>,
    warn_local: bool,
}

impl WarnTaggedCommitBeforeUndoDialog {
    pub fn new(repo: u64, tags: Vec<String>, warn_local: bool) -> Self {
        Self {
            repo,
            tags,
            warn_local,
        }
    }
}

impl Render for WarnTaggedCommitBeforeUndoDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, warn_local) = (self.repo, self.warn_local);
        let (noun, pronoun) = if self.tags.len() == 1 {
            ("tag", "It stays")
        } else {
            ("tags", "They stay")
        };
        let text = format!(
            "This commit has the {noun} {}. {pronoun} on the commit after it is undone, and \
             that commit will no longer be on any branch. Do you want to continue anyway?",
            self.tags
                .iter()
                .map(|t| format!("\u{201c}{t}\u{201d}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        dialog_with_kind(
            "dialog-warn-undo-tagged",
            DialogKind::Warning,
            "Undo Commit",
            div().child(text),
            vec![
                DialogButton {
                    id: "undo-tagged-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "undo-tagged-continue",
                    label: "Continue".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        if warn_local {
                            Dispatcher::request_undo_commit_after_tags(repo, cx);
                        } else {
                            Dispatcher::undo_commit(repo, cx);
                        }
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// Flag `826`: delete a tag Corvane did not create-and-hold (not in
/// `tagsToPush`), optionally from the remote too (Corvane addition; GHD only
/// deletes unpushed tags). The remote box starts unticked.
pub struct ConfirmDeletePushedTagDialog {
    repo: u64,
    tag: String,
    remote: Option<corvane_core::Remote>,
    from_remote: bool,
}

impl ConfirmDeletePushedTagDialog {
    pub fn new(repo: u64, tag: String, cx: &App) -> Self {
        Self {
            repo,
            tag,
            remote: Dispatcher::current_remote(repo, cx),
            from_remote: false,
        }
    }
}

impl Render for ConfirmDeletePushedTagDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, tag) = (self.repo, self.tag.clone());
        let remote = self.remote.clone().filter(|_| self.from_remote);
        let content = div()
            .flex()
            .flex_col()
            .child(div().mb(SPACING()).child(format!(
                "Are you sure you want to delete the tag {}?",
                self.tag
            )))
            .when_some(self.remote.clone(), |d, r| {
                d.child(
                    div()
                        .id("delete-tag-remote")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF())
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.from_remote = !this.from_remote;
                            cx.notify();
                        }))
                        .child(checkbox(
                            "delete-tag-remote-box",
                            self.from_remote,
                            false,
                            cx,
                        ))
                        .child(format!("Also delete it from {}", r.name)),
                )
            });
        dialog_with_kind(
            "dialog-delete-pushed-tag",
            DialogKind::Warning,
            "Delete Tag",
            content,
            vec![
                DialogButton {
                    id: "delete-tag-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "delete-tag-confirm",
                    label: "Delete".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::delete_pushed_tag(repo, tag.clone(), remote.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `ConfirmDiscardStash`
pub struct ConfirmDiscardStashDialog {
    repo: u64,
    dont_show_again: bool,
}

impl ConfirmDiscardStashDialog {
    pub fn new(repo: u64) -> Self {
        Self {
            repo,
            dont_show_again: false,
        }
    }
}

impl Render for ConfirmDiscardStashDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, dont_show_again) = (self.repo, self.dont_show_again);
        let content = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(SPACING())
                    .child("Are you sure you want to discard these stashed changes?"),
            )
            .child(
                div()
                    .id("discard-stash-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "discard-stash-dont-show-box",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-discard-stash",
            DialogKind::Warning,
            "Discard Stash?",
            content,
            vec![
                DialogButton {
                    id: "discard-stash-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "discard-stash-ok",
                    label: "Discard".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_discard_stash = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::drop_stash(repo, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `UnreachableCommitsDialog` ("Commit Reachability"): which of the selected
/// commits the range diff includes (Reachable) and which it cannot
/// (Unreachable), with the commits listed like history rows.
pub struct UnreachableCommitsDialog {
    state: Entity<AppState>,
    repo: u64,
    tab: UnreachableCommitsTab,
}

impl UnreachableCommitsDialog {
    pub fn new(state: Entity<AppState>, repo: u64, tab: UnreachableCommitsTab) -> Self {
        Self { state, repo, tab }
    }
}

impl Render for UnreachableCommitsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (commits, is_unreachable) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&self.repo);
            let unreachable = self.tab == UnreachableCommitsTab::Unreachable;
            let commits: Vec<corvane_core::Commit> = rs
                .map(|rs| {
                    rs.selected_commits
                        .iter()
                        .filter(|sha| rs.shas_in_diff.contains(sha) != unreachable)
                        .filter_map(|sha| rs.commits.iter().find(|c| &c.sha == sha).cloned())
                        .collect()
                })
                .unwrap_or_default();
            (commits, unreachable)
        };
        let count = commits.len();
        let not = if is_unreachable { "not " } else { "" };
        let message = crate::widgets::paragraph(vec![
            format!(
                "You will {not}see changes from the following {} because {} {not}in the ancestry path of the most recent commit in your selection. ",
                if count > 1 { "commits" } else { "commit" },
                if count > 1 { "they're" } else { "it's" }
            )
            .into(),
            crate::widgets::link_button("unreachable-learn-more", "Learn more about unreachable commits.", cx)
                .on_click(|_, _, cx| {
                    Dispatcher::open_url(
                        "https://github.com/desktop/desktop/blob/development/docs/learn-more/unreachable-commits.md",
                        cx,
                    )
                })
                .into_any_element()
                .into(),
        ]);
        let weak = cx.weak_entity();
        let tabs = crate::tab_bar::tab_bar(
            vec![
                crate::tab_bar::TabModel {
                    dot: false,
                    id: "unreachable-tab-unreachable",
                    label: "Unreachable".into(),
                    count: None,
                },
                crate::tab_bar::TabModel {
                    dot: false,
                    id: "unreachable-tab-reachable",
                    label: "Reachable".into(),
                    count: None,
                },
            ],
            if is_unreachable { 0 } else { 1 },
            move |ix, _, cx| {
                weak.update(cx, |this, cx| {
                    this.tab = if ix == 0 {
                        UnreachableCommitsTab::Unreachable
                    } else {
                        UnreachableCommitsTab::Reachable
                    };
                    cx.notify();
                })
                .ok();
            },
            cx,
        );
        // `.unreachable-commits { max-width: 400px }`, list ≥ 160 px
        let content = div()
            .w(zpx(400.))
            .mx(zpx(-20.))
            .my(zpx(-20.))
            .flex()
            .flex_col()
            .child(tabs)
            .child(
                div()
                    .p(SPACING())
                    .border_b_1()
                    .border_color(t.box_border)
                    .text_size(FONT_SIZE())
                    .child(message),
            )
            .child(
                div()
                    .id("unreachable-commit-list")
                    .min_h(zpx(160.))
                    .max_h(zpx(300.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(commits.iter().map(|commit| {
                        div()
                            .h(zpx(50.))
                            .flex_none()
                            .border_b_1()
                            .border_color(t.box_border)
                            .child(crate::history::commit_row_contents(
                                commit,
                                t.text,
                                t.text_secondary,
                                None,
                                cx,
                            ))
                    }))
                    .with_scrollbar(),
            );
        dialog(
            "dialog-unreachable-commits",
            "Commit Reachability",
            content,
            vec![DialogButton {
                id: "unreachable-ok",
                label: "OK".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            }],
            close,
            window,
            cx,
        )
    }
}
