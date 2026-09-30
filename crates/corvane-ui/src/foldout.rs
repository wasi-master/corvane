//! Foldouts: panels anchored under toolbar buttons, closed by clicking the
//! overlay or Esc (`#foldout-container`, `styles/ui/_foldout.scss`).

use corvane_core::{Dispatcher, Foldout};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::branch_list::BranchFoldout;
use crate::icons::{Octicon, octicon};
use crate::repository_list::RepositoryFoldout;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::worktree_list::WorktreeFoldout;

/// The views the foldout layer can show.
pub struct FoldoutPanels<'a> {
    pub repository: &'a Entity<RepositoryFoldout>,
    pub branch: &'a Entity<BranchFoldout>,
    pub worktree: &'a Entity<WorktreeFoldout>,
}

/// Renders the overlay + the open foldout panel, positioned below the toolbar.
/// `panel_x` / `panel_width` come from the toolbar button geometry.
pub fn foldout_layer(
    foldout: Foldout,
    panel_x: Pixels,
    panel_width: Pixels,
    panels: FoldoutPanels<'_>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let top = TITLE_BAR_HEIGHT() + TOOLBAR_HEIGHT();
    let viewport = window.viewport_size();
    let panel: AnyElement = match foldout {
        Foldout::Repository => panels.repository.clone().into_any_element(),
        Foldout::Branch => panels.branch.clone().into_any_element(),
        Foldout::Worktree => panels.worktree.clone().into_any_element(),
        Foldout::PushPull => push_pull_dropdown(cx),
    };
    let full_height = foldout != Foldout::PushPull;
    deferred(
        anchored().position(point(zpx(0.), top)).child(
            div()
                .id("foldout-container")
                .relative()
                .w(viewport.width)
                .h(viewport.height - top)
                .child(
                    // `.overlay`: click anywhere outside the panel closes it
                    div()
                        .id("foldout-overlay")
                        .absolute()
                        .inset_0()
                        .bg(t.overlay)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| Dispatcher::close_foldout(cx)),
                )
                .child(
                    // `.foldout`
                    div()
                        .id("foldout")
                        .absolute()
                        .top_0()
                        .when(full_height, |d| d.bottom_0())
                        .left(panel_x)
                        .w(panel_width)
                        .flex()
                        .flex_col()
                        .bg(t.background)
                        .text_color(t.text)
                        .border_r_1()
                        .border_color(t.box_border)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(panel),
                ),
        ),
    )
    .with_priority(10)
}

type ItemClick = Box<dyn Fn(&mut Window, &mut App)>;

/// GHD `PushPullButtonDropDown`: Fetch, and Force push when the branch has
/// diverged from its upstream (`styles/ui/toolbar/_push-pull-button.scss`).
/// Corvane addition (`229-reset-to-remote`): "Reset to <upstream>" while the
/// branch has commits the upstream lacks.
fn push_pull_dropdown(cx: &App) -> AnyElement {
    let t = cx.ghd();
    let state = corvane_core::AppState::global(cx).read(cx);
    let Some(id) = state.selected else {
        return div().into_any_element();
    };
    let remote = Dispatcher::current_remote(id, cx)
        .map(|r| r.name)
        .unwrap_or_else(|| "origin".to_string());
    let force_push = Dispatcher::force_push_state(id, cx);
    let confirm = state.settings.confirm_force_push;
    let item = |id: &'static str,
                icon: Octicon,
                title: String,
                detail: AnyElement,
                last: bool,
                on_click: ItemClick| {
        let hover_bg = t.box_hover_background;
        div()
            .id(id)
            .flex()
            .flex_row()
            .items_start()
            .gap(SPACING())
            .p(SPACING())
            .bg(t.box_background)
            .when(!last, |d| d.border_b_1().border_color(t.box_border))
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_click(move |_, window, cx| on_click(window, cx))
            .child(octicon(icon, t.text).flex_none().mt(zpx(1.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(zpx(3.))
                    .text_size(FONT_SIZE())
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().text_color(t.text_secondary).child(detail)),
            )
    };
    let has_force = force_push != corvane_core::ForcePushState::NotAvailable;
    let reset_upstream = state
        .flags
        .bool(corvane_core::flags::ids::RESET_TO_REMOTE)
        .then(|| state.repo_states.get(&id))
        .flatten()
        .filter(|rs| rs.ahead_behind.is_some_and(|ab| ab.ahead > 0))
        .and_then(|rs| rs.info.as_ref()?.current_branch()?.upstream_short())
        .map(str::to_string);
    let has_reset = reset_upstream.is_some();
    div()
        .flex()
        .flex_col()
        .child(item(
            "push-pull-fetch",
            Octicon::SyncClockwise,
            format!("Fetch {remote}"),
            div()
                .child(format!("Fetch the latest changes from {remote}"))
                .into_any_element(),
            !has_force && !has_reset,
            Box::new(move |_, cx| {
                Dispatcher::close_foldout(cx);
                Dispatcher::fetch(id, false, cx);
            }),
        ))
        .when(has_force, |d| {
            d.child(item(
                "push-pull-force",
                Octicon::ArrowUp,
                format!("Force push {remote}"),
                div()
                    .flex()
                    .flex_col()
                    .child(format!(
                        "Overwrite any changes on {remote} with your local changes"
                    ))
                    .when(!confirm, |d| {
                        d.child(
                            div()
                                .mt(SPACING())
                                .text_color(t.toolbar_dropdown_text_warning)
                                .child(
                                    "Warning: A force push will rewrite history on the remote. Any collaborators working on this branch will need to reset their own local branch to match the history of the remote.",
                                ),
                        )
                    })
                    .into_any_element(),
                !has_reset,
                Box::new(move |_, cx| {
                    Dispatcher::close_foldout(cx);
                    Dispatcher::confirm_or_force_push(id, cx);
                }),
            ))
        })
        .when_some(reset_upstream, |d, upstream| {
            d.child(item(
                "push-pull-reset-to-remote",
                Octicon::History,
                format!("Reset to {upstream}"),
                div()
                    .child(format!(
                        "Discard your local commits and changes and match {upstream}"
                    ))
                    .into_any_element(),
                true,
                Box::new(move |_, cx| {
                    Dispatcher::close_foldout(cx);
                    Dispatcher::request_reset_to_remote(id, cx);
                }),
            ))
        })
        .into_any_element()
}
