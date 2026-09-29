//! `#desktop-app-toolbar`: Repository / Branch / Push-Pull buttons.
//! Geometry from `styles/ui/toolbar/{_toolbar,_button,_dropdown}.scss`.

use corvane_core::{AheadBehind, AppState, Dispatcher, Foldout, Tip};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
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
}

/// GHD `Toolbar` render: repository, branch, push/pull - from the app state.
pub fn toolbar_models(state: &AppState, sidebar_width: Pixels) -> Vec<ToolbarButtonModel> {
    let repo = state.selected_repository();
    let repo_state = state.selected_state();
    let info = repo_state.and_then(|s| s.info.as_ref());

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
    };

    let (branch_icon, branch_desc, branch_title): (Octicon, &str, SharedString) = match info
        .map(|i| &i.tip)
    {
        Some(Tip::Valid { branch }) => (
            Octicon::GitBranch,
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
    let switching = repo_state.and_then(|s| s.checkout_target.clone());
    let (branch_icon, branch_desc, branch_title) = match switching {
        Some(target) => (
            Octicon::Sync,
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
        width: Some(TOOLBAR_BUTTON_WIDTH),
        foldout: Some(Foldout::Branch),
        open: state.foldout == Some(Foldout::Branch),
        disabled: repo.is_none(),
        badge: None,
        push_pull: false,
        arrow: false,
        progress: None,
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
        icon: Octicon::Sync,
        description: "".into(),
        title: "".into(),
        width: Some(TOOLBAR_BUTTON_WIDTH),
        foldout: None,
        open: state.foldout == Some(Foldout::PushPull),
        disabled: false,
        badge: None,
        push_pull: true,
        arrow: false,
        progress: None,
    };
    let push_pull = if repo.is_none() {
        ToolbarButtonModel {
            disabled: true,
            ..base
        }
    } else if let Some(p) = progress {
        ToolbarButtonModel {
            icon: Octicon::Sync,
            description: p
                .description
                .clone()
                .unwrap_or_else(|| "Hang on…".to_string())
                .into(),
            title: p.title.clone().into(),
            disabled: true,
            progress: Some(p.value),
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
                icon: Octicon::Sync,
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
                        icon: Octicon::Sync,
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

    vec![repository, branch, push_pull]
}

pub fn toolbar_button(model: ToolbarButtonModel, cx: &App) -> AnyElement {
    let t = cx.ghd();
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
        .h(TOOLBAR_BUTTON_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .p(SPACING)
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
        .child(octicon(model.icon, text).mr(SPACING))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .mr(SPACING)
                .child(
                    div()
                        .text_size(FONT_SIZE_SM)
                        .line_height(px(14.))
                        .text_color(secondary)
                        .truncate()
                        .child(model.description),
                )
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .line_height(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(model.title),
                ),
        )
        .when_some(model.badge, |d, ab| {
            // `.ahead-behind` pill: 13 px tall, radius 8, 9 px text
            d.child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .px(px(5.))
                    .h(px(13.))
                    .mr(SPACING_HALF)
                    .rounded(px(8.))
                    .bg(t.toolbar_badge_background)
                    .text_size(FONT_SIZE_XS)
                    .line_height(px(11.))
                    .when(ab.ahead > 0, |d| {
                        d.child(format!("{}", ab.ahead))
                            .child(octicon(Octicon::ArrowUp, text).size(px(9.)))
                    })
                    .when(ab.behind > 0, |d| {
                        d.child(format!("{}", ab.behind))
                            .child(octicon(Octicon::ArrowDown, text).size(px(9.)))
                    }),
            )
        })
        .when(foldout.is_some(), |d| {
            d.child(octicon(Octicon::TriangleDown, text).when(model.open, |s| {
                s.with_transformation(Transformation::rotate(Radians(std::f32::consts::PI)))
            }))
        });
    if !arrow {
        return button.into_any_element();
    }
    // `ToolbarDropdownStyle.MultiOption`: a 39 px ▾ button next to the main one
    div()
        .flex()
        .flex_row()
        .flex_none()
        .w(TOOLBAR_BUTTON_WIDTH)
        .child(button.w(TOOLBAR_BUTTON_WIDTH - TOOLBAR_ARROW_WIDTH))
        .child(
            div()
                .id("toolbar-push-pull-arrow")
                .h(TOOLBAR_BUTTON_HEIGHT)
                .w(TOOLBAR_ARROW_WIDTH)
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
pub fn toolbar(buttons: Vec<ToolbarButtonModel>, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("toolbar")
        .w_full()
        .h(TOOLBAR_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .min_w_0()
        .bg(t.toolbar_background)
        .border_b_1()
        .border_color(t.toolbar_border)
        .text_color(t.toolbar_text)
        .children(buttons.into_iter().map(|b| toolbar_button(b, cx)))
}
