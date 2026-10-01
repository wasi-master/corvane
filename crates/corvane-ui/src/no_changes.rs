//! `NoChanges` blankslate (`.changes-interstitial`): "No local changes" header
//! with the paper-stack illustration, then suggested-action groups.
//! `styles/ui/changes/_changes-interstitial.scss`, `ui/suggested-actions/*.scss`.
//!
//! Deviation (GHD `ui/changes/no-changes.tsx`): while the branch has an open
//! pull request a primary "View Pull Request" card leads the list (built in
//! `workspace.rs`, `725-no-changes-view-pull-request`); GHD shows no
//! remote action then.
//! Deviation (`726-restore-stash-suggestion`): with a stash on the branch the
//! first card is "Restore your stashed changes" with a primary Restore button
//! in place of GHD's "View your stashed changes" ([`primary_action`]).
//! Not built yet: GHD's Create / Preview Pull Request dropdown card for a
//! published branch, and the `Ref` styling of branch names in descriptions.

use corvane_core::{AppState, Branch, Dispatcher, Tip};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, kbd_group, primary_button};

pub struct SuggestedAction {
    pub id: &'static str,
    pub on_click: crate::widgets::ClickAction,
    pub title: SharedString,
    pub description: Option<SharedString>,
    /// `p.discoverability` text before the key caps, e.g. "Repository menu or".
    pub hint: SharedString,
    /// Key caps rendered as `kbd` elements, e.g. `["⌘", "⇧", "A"]`.
    pub keys: &'static [&'static str],
    pub button_label: SharedString,
    pub primary: bool,
}

/// GHD `NoChanges` primary group, `renderViewStashAction() ||
/// renderRemoteAction()`: the branch's stash once its files are loaded,
/// else publishing the repository or branch, pulling or pushing.
pub fn primary_action(state: &AppState, id: u64) -> Option<SuggestedAction> {
    let rs = state.repo_states.get(&id)?;
    let info = rs.info.as_ref()?;
    let Tip::Valid { branch } = &info.tip else {
        return None;
    };
    stash_action(state, id).or_else(|| remote_action(state, id, branch))
}

/// `renderViewStashAction`
fn stash_action(state: &AppState, id: u64) -> Option<SuggestedAction> {
    let rs = state.repo_states.get(&id)?;
    rs.stash.as_ref()?;
    // Corvane (`726-restore-stash-suggestion`): restore it from here instead
    if state
        .flags
        .bool(corvane_core::flags::ids::RESTORE_STASH_SUGGESTION)
    {
        return Some(SuggestedAction {
            id: "suggested-restore-stash",
            on_click: std::rc::Rc::new(move |_, cx| Dispatcher::pop_stash(id, cx)),
            title: "Restore your stashed changes".into(),
            description: Some(
                "This branch has stashed changes that you have not yet committed.".into(),
            ),
            hint: "When a stash exists, access it at the bottom of the Changes tab to the left."
                .into(),
            keys: &[],
            button_label: "Restore".into(),
            primary: true,
        });
    }
    let count = rs.stash_files.as_ref()?.len();
    Some(SuggestedAction {
        id: "suggested-view-stash",
        on_click: std::rc::Rc::new(move |_, cx| Dispatcher::toggle_stash_view(id, cx)),
        title: "View your stashed changes".into(),
        description: Some(
            format!(
                "You have {count} {} in progress that you have not yet committed.",
                if count == 1 { "change" } else { "changes" }
            )
            .into(),
        ),
        hint: "When a stash exists, access it at the bottom of the Changes tab to the left.".into(),
        keys: &[],
        button_label: "View stash".into(),
        primary: true,
    })
}

/// `renderRemoteAction`, less `renderCreatePullRequestAction`
fn remote_action(state: &AppState, id: u64, branch: &Branch) -> Option<SuggestedAction> {
    let rs = state.repo_states.get(&id)?;
    let info = rs.info.as_ref()?;
    let is_github = state.repository(id).is_some_and(|r| r.github.is_some());
    // `renderPublishRepositoryAction`
    if info.remotes.is_empty() {
        return Some(SuggestedAction {
            id: "suggested-publish-repository",
            on_click: std::rc::Rc::new(move |_, cx| Dispatcher::push_pull_action(id, cx)),
            title: "Publish your repository to GitHub".into(),
            description: Some(
                "This repository is currently only available on your local machine. By \
                 publishing it on GitHub you can share it, and collaborate with others."
                    .into(),
            ),
            hint: "Always available in the toolbar for local repositories or".into(),
            keys: &["⌘", "P"],
            button_label: "Publish repository".into(),
            primary: true,
        });
    }
    let remote = Dispatcher::current_remote_in(state, id)?.name;
    // `renderPublishBranchAction` (GHD's `aheadBehind` is null without an
    // upstream)
    if branch.upstream.is_none() {
        return Some(SuggestedAction {
            id: "suggested-publish-branch",
            on_click: std::rc::Rc::new(move |_, cx| Dispatcher::push(id, false, None, cx)),
            title: "Publish your branch".into(),
            description: Some(
                format!(
                    "The current branch ({}) hasn't been published to the remote yet. By \
                     publishing it {}you can share it, {}and collaborate with others.",
                    branch.name,
                    if is_github { "to GitHub " } else { "" },
                    if is_github {
                        "open a pull request, "
                    } else {
                        ""
                    },
                )
                .into(),
            ),
            hint: "Always available in the toolbar or".into(),
            keys: &["⌘", "P"],
            button_label: "Publish branch".into(),
            primary: true,
        });
    }
    let ab = rs.ahead_behind?;
    // no action after a rebase: pulling would tangle the history
    if Dispatcher::force_push_state_in(state, id) == corvane_core::ForcePushState::Recommended {
        return None;
    }
    let host = if is_github { "GitHub" } else { "the remote" };
    if ab.behind > 0 {
        let one = ab.behind == 1;
        return Some(SuggestedAction {
            id: "suggested-pull",
            on_click: std::rc::Rc::new(move |_, cx| Dispatcher::pull(id, cx)),
            title: format!(
                "Pull {} {} from the {remote} remote",
                crate::format::format_count(ab.behind.into()),
                if one { "commit" } else { "commits" }
            )
            .into(),
            description: Some(
                format!(
                    "The current branch ({}) has {} on {host} that {} exist on your machine.",
                    branch.name,
                    if one { "a commit" } else { "commits" },
                    if one { "does not" } else { "do not" },
                )
                .into(),
            ),
            hint: "Always available in the toolbar when there are remote changes or".into(),
            keys: &["⌘", "⇧", "P"],
            button_label: format!("Pull {remote}").into(),
            primary: true,
        });
    }
    let tags = state.repository(id).map_or(0, |r| r.tags_to_push.len());
    if ab.ahead > 0 || tags > 0 {
        let mut kinds = Vec::new();
        let mut counts = Vec::new();
        if ab.ahead > 0 {
            kinds.push("commits");
            counts.push(if ab.ahead == 1 {
                "1 local commit".to_string()
            } else {
                format!(
                    "{} local commits",
                    crate::format::format_count(ab.ahead.into())
                )
            });
        }
        if tags > 0 {
            kinds.push("tags");
            counts.push(if tags == 1 {
                "1 tag".to_string()
            } else {
                format!("{} tags", crate::format::format_count(tags as u64))
            });
        }
        return Some(SuggestedAction {
            id: "suggested-push",
            on_click: std::rc::Rc::new(move |_, cx| Dispatcher::push(id, false, None, cx)),
            title: format!("Push {} to the {remote} remote", kinds.join(" and ")).into(),
            description: Some(
                format!(
                    "You have {} waiting to be pushed to {host}.",
                    counts.join(" and ")
                )
                .into(),
            ),
            hint: "Always available in the toolbar when there are local commits waiting to be \
                   pushed or"
                .into(),
            keys: &["⌘", "P"],
            button_label: format!("Push {remote}").into(),
            primary: true,
        });
    }
    None
}

/// `.suggested-action`: base border, 20 px padding, row; primary variant tinted.
pub fn suggested_action_card(action: SuggestedAction, cx: &App) -> impl IntoElement {
    card(action, cx)
}

fn card(action: SuggestedAction, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let (bg, border) = if action.primary {
        (
            t.primary_suggested_action_background,
            t.primary_suggested_action_border,
        )
    } else {
        (t.background, t.box_border)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .p(SPACING_DOUBLE())
        .border_1()
        .border_color(border)
        .rounded(BORDER_RADIUS())
        .bg(bg)
        .child(
            // `.text-wrapper`
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .mr(SPACING_DOUBLE())
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(FONT_SIZE())
                        .line_height(zpx(18.))
                        .child(action.title),
                )
                .when_some(action.description, |d, desc| {
                    d.child(
                        div()
                            .text_size(FONT_SIZE())
                            .line_height(zpx(18.))
                            .mb(SPACING_HALF())
                            .child(desc),
                    )
                })
                .when(!action.hint.is_empty() || !action.keys.is_empty(), |d| {
                    d.child(
                        // `p.discoverability`
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(zpx(4.))
                            .text_size(FONT_SIZE())
                            .line_height(zpx(18.))
                            .text_color(t.text_secondary)
                            .child(action.hint)
                            .child(kbd_group(action.keys, cx)),
                    )
                }),
        )
        .child({
            let on_click = action.on_click.clone();
            if action.primary {
                primary_button(action.id, action.button_label, false, cx)
                    .on_click(move |_, window, cx| on_click(window, cx))
                    .into_any_element()
            } else {
                button(action.id, action.button_label, cx)
                    .on_click(move |_, window, cx| on_click(window, cx))
                    .into_any_element()
            }
        })
}

pub fn no_changes(actions: Vec<SuggestedAction>, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("no-changes")
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_col()
        .items_center()
        .p(zpx(40.))
        .bg(t.background)
        .child(
            // `.content`: full width, max 600 px, centred
            div()
                .w_full()
                .max_w(zpx(600.))
                .flex()
                .flex_col()
                .gap(SPACING())
                .child(
                    // `.interstitial-header`: text left, image bottom-aligned right
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .mb(SPACING())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .mr(SPACING_DOUBLE())
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(zpx(32.))
                                        // `h1` inherits `body`'s `line-height: 1.5`
                                        // (`styles/_globals.scss`)
                                        .line_height(if cfg!(target_os = "macos") {
                                            zpx(38.)
                                        } else {
                                            zpx(32. * 1.5)
                                        })
                                        .font_weight(FontWeight::LIGHT)
                                        .child("No local changes"),
                                )
                                .child(
                                    div()
                                        .text_size(FONT_SIZE())
                                        .line_height(zpx(18.))
                                        .text_color(t.text)
                                        .child(
                                            "There are no uncommitted changes in this repository. Here are some friendly suggestions for what to do next.",
                                        ),
                                ),
                        )
                        .child(
                            // `.blankslate-image`: `flex: 0` leaves the
                            // 70 px `min-width`, `min-height` (73 px) wins
                            // over `height`
                            crate::widgets::blankslate_image("paper-stack.svg", cx)
                                .w(zpx(70.))
                                .h(zpx(73.))
                                .flex_none(),
                        ),
                )
                .children(actions.into_iter().map(|a| card(a, cx))),
        )
}

/// GHD `MultipleSelection` (`.panel.blankslate`): "N files selected".
pub fn multiple_selection(count: usize, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("multiple-selection")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(SPACING_DOUBLE())
        .bg(t.background)
        .text_color(t.text_secondary)
        .text_size(FONT_SIZE())
        .child(
            crate::widgets::blankslate_image("multiple-files-selected.svg", cx)
                .w(zpx(200.))
                .h(zpx(120.)),
        )
        .child(format!("{count} files selected"))
}
