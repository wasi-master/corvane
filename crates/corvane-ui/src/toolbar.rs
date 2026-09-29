//! `#desktop-app-toolbar`: Repository / Branch / Push-Pull buttons.
//! Geometry from `styles/ui/toolbar/{_toolbar,_button,_dropdown}.scss`.
//!
//! The worktree and branch buttons are resizable (`enableResizingToolbarButtons`,
//! `ui/resizable/resizable.tsx`, `_resizable.scss`): a 6 px handle straddles
//! their right edge; dragging sets the width within
//! `corvane_core::toolbar_widths`, double-clicking resets it to 230 px. The
//! width is saved when the drag ends (GHD writes it on every move).

use std::cell::Cell;
use std::rc::Rc;

use corvane_core::toolbar_widths::{ConstrainedWidth, ToolbarWidths};
use corvane_core::{AheadBehind, AppState, Dispatcher, Foldout, Tip};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::IconButtonA11y;

use crate::icons::{Octicon, octicon, spin};
use crate::relative_time::relative;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// One toolbar button, derived from `AppState` by [`toolbar_models`].
pub struct ToolbarButtonModel {
    pub id: &'static str,
    pub icon: Octicon,
    /// Small secondary line ("Current Repository").
    pub description: SharedString,
    /// Bold main line (repository / branch name).
    pub title: SharedString,
    pub width: Option<Pixels>,
    /// Opens this foldout when clicked; `None` = plain action button.
    pub foldout: Option<Foldout>,
    pub open: bool,
    pub disabled: bool,
    pub badge: Option<AheadBehind>,
    /// Push/pull button: the main click runs the network action instead of a foldout.
    pub push_pull: bool,
    /// Show the 39 px ▾ button that opens `Foldout::PushPull`.
    pub arrow: bool,
    /// `progressValue`: fill the button background up to this fraction.
    pub progress: Option<f32>,
    /// `iconClassName = 'spin'` (checkout / network action in progress).
    pub spin: bool,
    /// `PullRequestBadge` on the branch button (`#N` + CI status).
    pub pr_badge: Option<PrBadge>,
    /// Resizable worktree / branch button and its width constraints.
    pub resize: Option<(ResizeTarget, ConstrainedWidth)>,
}

/// Which toolbar button a resize handle belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeTarget {
    Worktree,
    Branch,
}

#[derive(Clone, Copy)]
struct ResizeDrag {
    target: ResizeTarget,
    start_x: Pixels,
    start_width: f32,
    constraint: ConstrainedWidth,
}

/// Drag state shared by the toolbar's resize handles (owned by the workspace).
#[derive(Default)]
pub struct ToolbarResize {
    drag: Cell<Option<ResizeDrag>>,
    /// The width being dragged to; saved to the settings on mouse up.
    live: Cell<Option<(ResizeTarget, f32)>>,
}

impl ToolbarResize {
    /// The width a button shows while it is being dragged.
    pub fn live_width(&self, target: ResizeTarget) -> Option<f32> {
        self.live
            .get()
            .filter(|(t, _)| *t == target)
            .map(|(_, w)| w)
    }
}

/// The worktree and branch button widths for this window
/// (`updateResizableConstraints`), with a drag in progress applied.
pub fn toolbar_widths(
    state: &AppState,
    window_width: Pixels,
    sidebar_width: Pixels,
    resize: &ToolbarResize,
) -> (ToolbarWidths, Pixels, Pixels) {
    // the constraint math is in CSS pixels (GHD); widths persist unzoomed
    let widths = corvane_core::toolbar_widths::toolbar_widths(
        unzoom(window_width),
        unzoom(sidebar_width),
        worktree_button_visible(state),
        state.settings.worktree_dropdown_width,
        state.settings.branch_dropdown_width,
    );
    let worktree = resize
        .live_width(ResizeTarget::Worktree)
        .unwrap_or_else(|| widths.worktree.clamped());
    let branch = resize
        .live_width(ResizeTarget::Branch)
        .unwrap_or_else(|| widths.branch.clamped());
    (widths, zpx(worktree), zpx(branch))
}

/// `renderPullRequestInfo`
pub struct PrBadge {
    pub number: u64,
    pub status: Option<(
        corvane_core::CheckStatus,
        Option<corvane_core::CheckConclusion>,
    )>,
    /// Window-space rectangle of the badge, for the check-run popover.
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
}

/// GHD `renderWorktreeToolbarButton`: only with linked worktrees, or while
/// the foldout is open (so it can be reached from the menu).
pub fn worktree_button_visible(state: &AppState) -> bool {
    let has_linked = state
        .selected_state()
        .is_some_and(|rs| rs.worktrees.len() > 1);
    state.selected.is_some() && (has_linked || state.foldout == Some(Foldout::Worktree))
}

/// GHD `Toolbar` render: repository, worktree, branch, push/pull - from the app state.
pub fn toolbar_models(
    state: &AppState,
    sidebar_width: Pixels,
    (widths, worktree_width, branch_width): (ToolbarWidths, Pixels, Pixels),
    pr_badge_bounds: &Rc<Cell<Bounds<Pixels>>>,
) -> Vec<ToolbarButtonModel> {
    let repo = state.selected_repository();
    let repo_state = state.selected_state();
    let info = repo_state.and_then(|s| s.info.as_ref());

    // `WorktreeDropdown`: title = current worktree folder, else the repository name
    let worktree = worktree_button_visible(state).then(|| {
        let title: SharedString = repo
            .and_then(|r| crate::worktree_list::current_worktree(state, r.id))
            .map(|w| w.display_name())
            .or_else(|| repo.map(|r| r.name()))
            .unwrap_or_default()
            .into();
        ToolbarButtonModel {
            id: "toolbar-worktree",
            icon: Octicon::FileDirectory,
            description: "Current Worktree".into(),
            title,
            width: Some(worktree_width),
            foldout: Some(Foldout::Worktree),
            open: state.foldout == Some(Foldout::Worktree),
            disabled: false,
            badge: None,
            push_pull: false,
            arrow: false,
            progress: None,
            spin: false,
            pr_badge: None,
            resize: Some((ResizeTarget::Worktree, widths.worktree)),
        }
    });

    let repository = ToolbarButtonModel {
        id: "toolbar-repository",
        icon: match repo.and_then(|r| r.github.as_ref()) {
            Some(gh) if gh.fork => Octicon::RepoForked,
            Some(gh) if gh.private => Octicon::Lock,
            Some(_) => Octicon::Repo,
            None if repo.is_some() => Octicon::DeviceDesktop,
            None => Octicon::Repo,
        },
        description: "Current Repository".into(),
        title: repo
            .map(|r| r.name().into())
            .unwrap_or_else(|| SharedString::from("Select a repository")),
        width: Some(sidebar_width),
        foldout: Some(Foldout::Repository),
        open: state.foldout == Some(Foldout::Repository),
        disabled: false,
        badge: None,
        push_pull: false,
        arrow: false,
        progress: None,
        spin: false,
        pr_badge: None,
        resize: None,
    };

    // `currentPullRequest`: the icon becomes the PR icon and the badge shows
    let current_pr = repo.and_then(|r| state.current_pull_request(r.id));
    let pr_badge = current_pr.map(|pr| PrBadge {
        number: pr.number,
        status: state.commit_status_summary(pr),
        bounds: pr_badge_bounds.clone(),
    });
    let (branch_icon, branch_desc, branch_title): (Octicon, &str, SharedString) = match info
        .map(|i| &i.tip)
    {
        Some(Tip::Valid { branch }) => (
            if current_pr.is_some() {
                Octicon::GitPullRequest
            } else {
                Octicon::GitBranch
            },
            "Current Branch",
            branch.name.clone().into(),
        ),
        Some(Tip::Unborn { name }) => (Octicon::GitBranch, "Current Branch", name.clone().into()),
        Some(Tip::Detached { sha }) => (
            Octicon::GitCommit,
            "Detached HEAD",
            format!("On {}", sha.chars().take(7).collect::<String>()).into(),
        ),
        _ => (Octicon::GitBranch, "Current Branch", "".into()),
    };
    // `checkoutProgress`: title = target branch, description = "Switching to Branch"
    let switching_to = repo_state.and_then(|s| s.checkout_target.clone());
    let switching = switching_to.is_some();
    let (branch_icon, branch_desc, branch_title) = match switching_to {
        Some(target) => (
            Octicon::SyncClockwise,
            "Switching to Branch",
            SharedString::from(target),
        ),
        None => (branch_icon, branch_desc, branch_title),
    };
    let branch = ToolbarButtonModel {
        id: "toolbar-branch",
        icon: branch_icon,
        description: branch_desc.into(),
        title: branch_title,
        width: Some(branch_width),
        foldout: Some(Foldout::Branch),
        open: state.foldout == Some(Foldout::Branch),
        disabled: repo.is_none(),
        badge: None,
        push_pull: false,
        arrow: false,
        progress: None,
        spin: switching,
        pr_badge,
        resize: Some((ResizeTarget::Branch, widths.branch)),
    };

    // Push/Pull (`PushPullButton.renderButton`)
    let has_remote = info.map(|i| !i.remotes.is_empty()).unwrap_or(false);
    let remote_name = repo
        .and_then(|r| Dispatcher::current_remote_in(state, r.id))
        .map(|r| r.name)
        .unwrap_or_else(|| "origin".to_string());
    let upstream = info
        .and_then(|i| i.current_branch())
        .and_then(|b| b.upstream.clone());
    let ab = repo_state.and_then(|s| s.ahead_behind);
    let last_fetched: SharedString = match repo_state.and_then(|s| s.last_fetched) {
        Some(at) => format!("Last fetched {}", relative(at)).into(),
        None => "Never fetched".into(),
    };
    let progress = repo_state.and_then(|s| s.push_pull_progress.clone());
    let is_github = repo.is_some_and(|r| r.github.is_some());
    let force_push = repo
        .map(|r| Dispatcher::force_push_state_in(state, r.id))
        .unwrap_or(corvane_core::ForcePushState::NotAvailable);
    let pull_with_rebase = repo_state.is_some_and(|s| s.pull_with_rebase);
    let rebase_in_progress = repo_state
        .and_then(|s| s.status.as_ref())
        .is_some_and(|st| st.rebase_in_progress);
    let base = ToolbarButtonModel {
        id: "toolbar-push-pull",
        icon: Octicon::SyncClockwise,
        description: "".into(),
        title: "".into(),
        width: Some(TOOLBAR_BUTTON_WIDTH()),
        foldout: None,
        open: state.foldout == Some(Foldout::PushPull),
        disabled: false,
        badge: None,
        push_pull: true,
        arrow: false,
        progress: None,
        spin: false,
        pr_badge: None,
        resize: None,
    };
    let push_pull = if repo.is_none() {
        ToolbarButtonModel {
            disabled: true,
            ..base
        }
    } else if let Some(p) = progress {
        ToolbarButtonModel {
            icon: Octicon::SyncClockwise,
            description: p
                .description
                .clone()
                .unwrap_or_else(|| "Hang on…".to_string())
                .into(),
            title: p.title.clone().into(),
            disabled: true,
            progress: Some(p.value),
            spin: true,
            ..base
        }
    } else if !has_remote {
        ToolbarButtonModel {
            icon: Octicon::Upload,
            description: "Publish this repository to GitHub".into(),
            title: "Publish repository".into(),
            ..base
        }
    } else {
        match info.map(|i| &i.tip) {
            Some(Tip::Unborn { .. }) => ToolbarButtonModel {
                icon: Octicon::SyncClockwise,
                description: last_fetched,
                title: format!("Fetch {remote_name}").into(),
                ..base
            },
            Some(Tip::Detached { .. }) | Some(Tip::Unknown) | None => ToolbarButtonModel {
                icon: Octicon::Upload,
                description: if rebase_in_progress {
                    "Rebase in progress".into()
                } else {
                    "Cannot publish detached HEAD".into()
                },
                title: "Publish branch".into(),
                disabled: true,
                ..base
            },
            Some(Tip::Valid { .. }) if upstream.is_none() => ToolbarButtonModel {
                icon: Octicon::Upload,
                description: if is_github {
                    "Publish this branch to GitHub".into()
                } else {
                    "Publish this branch to the remote".into()
                },
                title: "Publish branch".into(),
                arrow: true,
                ..base
            },
            Some(Tip::Valid { .. }) => {
                let ab = ab.unwrap_or_default();
                if ab.ahead == 0 && ab.behind == 0 {
                    ToolbarButtonModel {
                        icon: Octicon::SyncClockwise,
                        description: last_fetched,
                        title: format!("Fetch {remote_name}").into(),
                        ..base
                    }
                } else if force_push == corvane_core::ForcePushState::Recommended {
                    ToolbarButtonModel {
                        icon: Octicon::ArrowUp,
                        description: last_fetched,
                        title: format!("Force push {remote_name}").into(),
                        badge: Some(ab),
                        arrow: true,
                        ..base
                    }
                } else if ab.behind > 0 {
                    ToolbarButtonModel {
                        icon: Octicon::ArrowDown,
                        description: last_fetched,
                        title: if pull_with_rebase {
                            format!("Pull {remote_name} with rebase").into()
                        } else {
                            format!("Pull {remote_name}").into()
                        },
                        badge: Some(ab),
                        arrow: true,
                        ..base
                    }
                } else {
                    ToolbarButtonModel {
                        icon: Octicon::ArrowUp,
                        description: last_fetched,
                        title: format!("Push {remote_name}").into(),
                        badge: Some(ab),
                        arrow: true,
                        ..base
                    }
                }
            }
        }
    };

    let mut buttons = vec![repository];
    buttons.extend(worktree);
    buttons.push(branch);
    buttons.push(push_pull);
    buttons
}

pub fn toolbar_button(
    model: ToolbarButtonModel,
    resize_state: &Rc<ToolbarResize>,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let resize = model.resize.zip(model.width);
    let hover_bg = t.toolbar_button_hover_background;
    let hover_text = t.toolbar_button_hover_text;
    let (bg, text, secondary) = if model.open {
        (
            t.toolbar_button_active_background,
            t.toolbar_button_active_text,
            t.text_secondary,
        )
    } else {
        (
            t.toolbar_background,
            t.toolbar_text,
            t.toolbar_text_secondary,
        )
    };
    let foldout = model.foldout;
    let disabled = model.disabled;
    let push_pull = model.push_pull;
    let arrow = model.arrow;
    let progress = model.progress;
    let arrow_open = model.open && push_pull;
    let (arrow_bg, arrow_text) = if arrow_open {
        (
            t.toolbar_button_active_background,
            t.toolbar_button_active_text,
        )
    } else {
        (t.toolbar_background, t.toolbar_text)
    };
    let bg = if push_pull { t.toolbar_background } else { bg };
    let text = if push_pull { t.toolbar_text } else { text };
    let secondary = if push_pull {
        t.toolbar_text_secondary
    } else {
        secondary
    };
    let button = div()
        .id(model.id)
        .h(TOOLBAR_BUTTON_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .p(SPACING())
        .border_r_1()
        .border_color(t.toolbar_button_border)
        .bg(bg)
        .text_color(text)
        .overflow_hidden()
        .when(disabled, |d| d.opacity(0.6))
        .relative()
        .when(!disabled && (!model.open || push_pull), move |d| {
            d.cursor_pointer()
                .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        })
        .when(!disabled, move |d| {
            d.on_click(move |_, _, cx| {
                if push_pull {
                    if let Some(id) = corvane_core::AppState::global(cx).read(cx).selected {
                        Dispatcher::close_foldout(cx);
                        Dispatcher::push_pull_action(id, cx);
                    }
                } else if let Some(foldout) = foldout {
                    Dispatcher::toggle_foldout(foldout, cx);
                }
            })
        })
        .when_some(progress, |d, value| {
            // `.progress`: fills the button from the left while an operation runs
            d.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(gpui_kit::relative(value.clamp(0., 1.)))
                    .bg(t.toolbar_button_progress),
            )
        })
        .when(foldout == Some(Foldout::Worktree), |d| {
            // `WorktreeDropdown.onContextMenu`
            d.on_mouse_down(MouseButton::Right, |ev, window, cx| {
                crate::worktree_list::toolbar_button_menu(ev.position, window, cx)
            })
        })
        .when(foldout == Some(Foldout::Branch), |d| {
            // GHD `onDragEnter` on the branch dropdown: dragging commits over
            // it opens the list so they can be dropped on a branch.
            d.on_drag_move::<crate::history::CommitDrag>(move |ev, _, cx| {
                if ev.bounds.contains(&ev.event.position)
                    && corvane_core::AppState::global(cx).read(cx).foldout != Some(Foldout::Branch)
                {
                    Dispatcher::toggle_foldout(Foldout::Branch, cx);
                }
            })
        })
        .when_some(model.width, |d, w| d.w(w))
        .when(model.width.is_none(), |d| d.flex_1().min_w_0())
        .child(if model.spin {
            div()
                .flex_none()
                .mr(SPACING())
                .child(spin(octicon(model.icon, text), "toolbar-button-spin"))
                .into_any_element()
        } else {
            octicon(model.icon, text).mr(SPACING()).into_any_element()
        })
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .mr(SPACING())
                .child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .line_height(zpx(14.))
                        .text_color(secondary)
                        .truncate()
                        .child(model.description),
                )
                .child(
                    div()
                        .text_size(FONT_SIZE())
                        .line_height(zpx(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(model.title),
                ),
        )
        .when_some(model.pr_badge, |d, badge| {
            // `.pr-badge`: 22 px tall, `#N` + the CI status; clickable once
            // a status is known (opens the check-run popover)
            let clickable = badge.status.is_some();
            let bounds = badge.bounds.clone();
            let badge_bg = if model.open {
                gpui_kit::transparent_black()
            } else {
                t.toolbar_background
            };
            d.child(
                div()
                    .id("pr-badge")
                    .relative()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(zpx(22.))
                    .px(SPACING_HALF())
                    .mr(SPACING())
                    .rounded(BORDER_RADIUS())
                    .border_1()
                    .border_color(t.toolbar_badge_background)
                    .bg(badge_bg)
                    .when(clickable, |d| {
                        d.cursor_pointer().hover(move |s| s.bg(hover_bg)).on_click(
                            move |_, _, cx| {
                                let show = corvane_core::AppState::global(cx)
                                    .read(cx)
                                    .show_ci_status_popover;
                                Dispatcher::set_show_ci_status_popover(!show, cx);
                                cx.stop_propagation();
                            },
                        )
                    })
                    .child(
                        canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                            .absolute()
                            .inset_0(),
                    )
                    .child(
                        div()
                            .text_size(FONT_SIZE_SM())
                            .line_height(zpx(22.))
                            .child(format!("#{}", badge.number)),
                    )
                    .when_some(badge.status, |d, (status, conclusion)| {
                        d.child(crate::ci_status::ci_status(status, conclusion).ml(SPACING_HALF()))
                    }),
            )
        })
        .when_some(model.badge, |d, ab| {
            // `.ahead-behind` pill: 13 px tall, radius 8, 9 px text
            d.child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(zpx(2.))
                    .px(zpx(5.))
                    .h(zpx(13.))
                    .mr(SPACING_HALF())
                    .rounded(zpx(8.))
                    .bg(t.toolbar_badge_background)
                    .text_size(FONT_SIZE_XS())
                    .line_height(zpx(11.))
                    .when(ab.ahead > 0, |d| {
                        d.child(format!("{}", ab.ahead))
                            .child(octicon(Octicon::ArrowUp, text).size(zpx(9.)))
                    })
                    .when(ab.behind > 0, |d| {
                        d.child(format!("{}", ab.behind))
                            .child(octicon(Octicon::ArrowDown, text).size(zpx(9.)))
                    }),
            )
        })
        .when(foldout.is_some(), |d| {
            d.child(octicon(Octicon::TriangleDown, text).when(model.open, |s| {
                s.with_transformation(Transformation::rotate(Radians(std::f32::consts::PI)))
            }))
        });
    if let Some(((target, constraint), width)) = resize {
        // `.resizable-component` + `.resize-handle` (6 px, `right: -3px`)
        let state = resize_state.clone();
        return div()
            .relative()
            .flex_none()
            .w(width)
            .child(button.w_full())
            .child(
                div()
                    .id(match target {
                        ResizeTarget::Worktree => "toolbar-worktree-resize-handle",
                        ResizeTarget::Branch => "toolbar-branch-resize-handle",
                    })
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(zpx(-3.))
                    .w(zpx(6.))
                    .occlude()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                        cx.stop_propagation();
                        if ev.click_count >= 2 {
                            // `onReset`
                            state.drag.set(None);
                            state.live.set(None);
                            Dispatcher::update_settings(cx, |s| match target {
                                ResizeTarget::Worktree => s.worktree_dropdown_width = None,
                                ResizeTarget::Branch => s.branch_dropdown_width = None,
                            });
                            return;
                        }
                        state.drag.set(Some(ResizeDrag {
                            target,
                            start_x: ev.position.x,
                            start_width: unzoom(width),
                            constraint,
                        }));
                        window.refresh();
                    }),
            )
            .into_any_element();
    }
    if !arrow {
        return button.into_any_element();
    }
    // `ToolbarDropdownStyle.MultiOption`: a 39 px ▾ button next to the main one
    div()
        .flex()
        .flex_row()
        .flex_none()
        .w(TOOLBAR_BUTTON_WIDTH())
        .child(button.w(TOOLBAR_BUTTON_WIDTH() - TOOLBAR_ARROW_WIDTH()))
        .child(
            div()
                .id("toolbar-push-pull-arrow")
                .icon_button_label("Push, pull, fetch options")
                .h(TOOLBAR_BUTTON_HEIGHT())
                .w(TOOLBAR_ARROW_WIDTH())
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .border_r_1()
                .border_color(t.toolbar_button_border)
                .bg(arrow_bg)
                .text_color(arrow_text)
                .cursor_pointer()
                .when(!arrow_open, move |d| {
                    d.hover(move |s| s.bg(hover_bg).text_color(hover_text))
                })
                .on_click(|_, _, cx| Dispatcher::toggle_foldout(Foldout::PushPull, cx))
                .child(octicon(Octicon::TriangleDown, arrow_text)),
        )
        .into_any_element()
}

/// The toolbar row: 50 px tall including its 1 px bottom border.
pub fn toolbar(
    buttons: Vec<ToolbarButtonModel>,
    resize: &Rc<ToolbarResize>,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let dragging = resize.drag.get().is_some();
    let listeners = resize.clone();
    div()
        .id("toolbar")
        .w_full()
        .h(TOOLBAR_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .min_w_0()
        .bg(t.toolbar_background)
        .border_b_1()
        .border_color(t.toolbar_border)
        .text_color(t.toolbar_text)
        .children(buttons.into_iter().map(|b| toolbar_button(b, resize, cx)))
        .when(dragging, |d| {
            // `handleDragMove` / `handleDragStop` on the document
            d.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        window.set_window_cursor_style(CursorStyle::ResizeLeftRight);
                        let state = listeners.clone();
                        window.on_mouse_event(move |ev: &MouseMoveEvent, _, window, _| {
                            if let Some(drag) = state.drag.get() {
                                let width = drag
                                    .constraint
                                    .clamp(drag.start_width + unzoom(ev.position.x - drag.start_x));
                                state.live.set(Some((drag.target, width)));
                                window.refresh();
                            }
                        });
                        let state = listeners.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, _, window, cx| {
                            if state.drag.take().is_none() {
                                return;
                            }
                            if let Some((target, width)) = state.live.take() {
                                Dispatcher::update_settings(cx, |s| match target {
                                    ResizeTarget::Worktree => {
                                        s.worktree_dropdown_width = Some(width)
                                    }
                                    ResizeTarget::Branch => s.branch_dropdown_width = Some(width),
                                });
                            }
                            window.refresh();
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
        })
}
