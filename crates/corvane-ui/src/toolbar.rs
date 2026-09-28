//! `#desktop-app-toolbar`: Repository / Branch / Push-Pull buttons.
//! Geometry from `styles/ui/toolbar/{_toolbar,_button,_dropdown}.scss`.

use corvane_core::{AheadBehind, AppState, Dispatcher, Foldout, Tip};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
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
            sha.chars().take(7).collect::<String>().into(),
        ),
        _ => (Octicon::GitBranch, "Current Branch", "".into()),
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
    };

    // Push/Pull (`PushPullButton`): publish repository → publish branch → pull/push/fetch.
    let has_remote = info.map(|i| !i.remotes.is_empty()).unwrap_or(false);
    let upstream = info
        .and_then(|i| i.current_branch())
        .and_then(|b| b.upstream.clone());
    let ab = repo_state.and_then(|s| s.ahead_behind);
    let push_pull = if repo.is_none() {
        ToolbarButtonModel {
            id: "toolbar-push-pull",
            icon: Octicon::Sync,
            description: "".into(),
            title: "".into(),
            width: Some(TOOLBAR_BUTTON_WIDTH),
            foldout: None,
            open: false,
            disabled: true,
            badge: None,
        }
    } else if !has_remote {
        ToolbarButtonModel {
            id: "toolbar-push-pull",
            icon: Octicon::Upload,
            description: "Publish this repository to GitHub".into(),
            title: "Publish repository".into(),
            width: Some(TOOLBAR_BUTTON_WIDTH),
            foldout: None,
            open: false,
            disabled: false,
            badge: None,
        }
    } else if upstream.is_none() {
        ToolbarButtonModel {
            id: "toolbar-push-pull",
            icon: Octicon::Upload,
            description: "Cannot publish unborn HEAD".into(),
            title: "Publish branch".into(),
            width: Some(TOOLBAR_BUTTON_WIDTH),
            foldout: None,
            open: false,
            disabled: !matches!(info.map(|i| &i.tip), Some(Tip::Valid { .. })),
            badge: None,
        }
    } else {
        let (icon, title) = match ab {
            Some(ab) if ab.behind > 0 => (Octicon::ArrowDown, "Pull origin"),
            Some(ab) if ab.ahead > 0 => (Octicon::ArrowUp, "Push origin"),
            _ => (Octicon::Sync, "Fetch origin"),
        };
        ToolbarButtonModel {
            id: "toolbar-push-pull",
            icon,
            description: "Never fetched".into(),
            title: title.into(),
            width: Some(TOOLBAR_BUTTON_WIDTH),
            foldout: None,
            open: false,
            disabled: false,
            badge: ab.filter(|ab| ab.ahead > 0 || ab.behind > 0),
        }
    };

    vec![repository, branch, push_pull]
}

pub fn toolbar_button(model: ToolbarButtonModel, cx: &App) -> impl IntoElement {
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
    div()
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
        .when(!disabled && !model.open, move |d| {
            d.cursor_pointer()
                .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        })
        .when(!disabled, move |d| {
            d.on_click(move |_, _, cx| {
                if let Some(foldout) = foldout {
                    Dispatcher::toggle_foldout(foldout, cx);
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
        })
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
