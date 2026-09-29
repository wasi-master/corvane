//! Commit-form autocompletion popup: `:emoji:`, `#issue` and `@user`
//! (GHD `ui/autocompletion/autocompleting-text-input.tsx`, the three
//! providers next to it and `styles/ui/_autocompletion.scss`).
//!
//! The matching lives in `corvane_core::autocomplete` / `corvane_core::emoji`;
//! this module owns the popup state, its rendering and the wrap-around
//! keyboard selection (`ui/lib/list/selection.ts` `findNextSelectableRow`).

use std::ops::Range;
use std::rc::Rc;

use corvane_core::emoji::EmojiHit;
use corvane_core::{
    AppState, DEFAULT_MAX_HITS, Dispatcher, GitHubRepository, IssueHit, MentionableUser,
    TriggerKind, find_trigger, issues_matching, users_matching,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{avatar_image, avatar_lookup_url};

/// GHD `RowHeight`.
const ROW_HEIGHT: Pixels = px(29.);
/// GHD `DefaultPopupHeight`.
const MAX_HEIGHT: Pixels = px(100.);

/// One row of the popup (`IEmojiHit` / `IIssueHit` / `UserHit`).
#[derive(Clone, Debug)]
pub enum Hit {
    Emoji(EmojiHit),
    Issue(IssueHit),
    User(MentionableUser),
    /// GHD `unknown-user`: a handle nobody in the mentionables matched
    /// (co-author input only; looked up on the API once added).
    UnknownUser(String),
}

impl Hit {
    /// GHD `getCompletionText`.
    pub fn completion_text(&self) -> String {
        match self {
            Hit::Emoji(e) => e.key.clone(),
            Hit::Issue(i) => format!("#{}", i.number),
            Hit::User(u) => format!("@{}", u.login),
            Hit::UnknownUser(name) => format!("@{name}"),
        }
    }
}

/// GHD `CoAuthorAutocompletionProvider.getAutocompletionItems`: mentionable
/// users matching `filter` (minus the ones already added) plus the typed
/// handle as an unknown user when nothing matches it exactly.
pub fn co_author_hits(
    filter: &str,
    github: &GitHubRepository,
    exclude: &[String],
    cx: &mut App,
) -> Vec<Hit> {
    Dispatcher::refresh_mentionables(github, cx);
    let own = Dispatcher::own_login_for(github, cx);
    let Some(state) = AppState::try_global(cx) else {
        return Vec::new();
    };
    let s = state.read(cx);
    let key = corvane_core::autocomplete::cache_key(github);
    let users = s
        .mentionables
        .get(&key)
        .map(|c| c.users.as_slice())
        .unwrap_or(&[]);
    let mut hits: Vec<Hit> = users_matching(users, filter, own.as_deref(), DEFAULT_MAX_HITS)
        .into_iter()
        .filter(|u| !exclude.iter().any(|e| e.eq_ignore_ascii_case(&u.login)))
        .map(Hit::User)
        .collect();
    if !filter.is_empty() {
        let exact = hits.iter().any(|h| match h {
            Hit::User(u) => u.login.eq_ignore_ascii_case(filter),
            _ => false,
        });
        if !exact {
            hits.push(Hit::UnknownUser(filter.to_string()));
        }
    }
    hits
}

/// GHD `IAutocompletionState`.
pub struct Autocompletion {
    pub kind: TriggerKind,
    /// Byte range of the filter text; the trigger character sits just before it.
    pub range: Range<usize>,
    pub hits: Vec<Hit>,
    /// Nothing is selected until the user presses ↑/↓ (GHD `selectedItem: null`).
    pub selected: Option<usize>,
    pub scroll: UniformListScrollHandle,
}

impl Autocompletion {
    /// ↑/↓ with wrap-around (`findNextSelectableRow`, `wrap: true`).
    pub fn move_selection(&mut self, delta: i64) {
        let n = self.hits.len() as i64;
        if n == 0 {
            return;
        }
        let next = match self.selected {
            None if delta > 0 => 0,
            None => n - 1,
            Some(ix) => (ix as i64 + delta).rem_euclid(n),
        } as usize;
        self.selected = Some(next);
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
    }
}

/// GHD `attemptAutocompletion` over the commit-message providers. Issue and
/// user lookups also kick off the throttled cache refreshes.
pub fn attempt(
    text: &str,
    caret: usize,
    github: Option<&GitHubRepository>,
    cx: &mut App,
) -> Option<Autocompletion> {
    let trigger = find_trigger(text, caret)?;
    let hits: Vec<Hit> = match trigger.kind {
        TriggerKind::Emoji => corvane_core::emoji::matches(&trigger.text, DEFAULT_MAX_HITS)
            .into_iter()
            .map(Hit::Emoji)
            .collect(),
        TriggerKind::Issue => {
            let gh = github?;
            Dispatcher::refresh_issues(gh, cx);
            let state = AppState::try_global(cx)?;
            let s = state.read(cx);
            let key = corvane_core::autocomplete::cache_key(gh);
            let issues = s
                .issues
                .get(&key)
                .map(|c| c.issues.as_slice())
                .unwrap_or(&[]);
            issues_matching(issues, &trigger.text, DEFAULT_MAX_HITS)
                .into_iter()
                .map(Hit::Issue)
                .collect()
        }
        TriggerKind::User => {
            let gh = github?;
            Dispatcher::refresh_mentionables(gh, cx);
            let own = Dispatcher::own_login_for(gh, cx);
            let state = AppState::try_global(cx)?;
            let s = state.read(cx);
            let key = corvane_core::autocomplete::cache_key(gh);
            let users = s
                .mentionables
                .get(&key)
                .map(|c| c.users.as_slice())
                .unwrap_or(&[]);
            users_matching(users, &trigger.text, own.as_deref(), DEFAULT_MAX_HITS)
                .into_iter()
                .map(Hit::User)
                .collect()
        }
    };
    if hits.is_empty() {
        return None;
    }
    Some(Autocompletion {
        kind: trigger.kind,
        range: trigger.range,
        hits,
        selected: None,
        scroll: UniformListScrollHandle::new(),
    })
}

pub type PickHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// GHD `renderAutocompletions`: a `Popover` anchored at the caret's
/// bottom-left, `RowHeight` rows, at most `DefaultPopupHeight` tall.
pub fn popup(
    ac: &Autocompletion,
    anchor: Point<Pixels>,
    on_pick: PickHandler,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    // `.autocompletion-popup` widths per provider kind
    let width = match ac.kind {
        TriggerKind::Emoji => px(200.),
        TriggerKind::User => px(220.),
        TriggerKind::Issue => px(300.),
    };
    let n = ac.hits.len();
    let height = (ROW_HEIGHT * n as f32).min(MAX_HEIGHT);
    let hits = Rc::new(ac.hits.clone());
    let selected = ac.selected;
    deferred(
        anchored()
            .position(anchor)
            .snap_to_window_with_margin(px(8.))
            .child(
                div()
                    .id("autocompletion-popup")
                    .occlude()
                    .w(width)
                    .h(height)
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(BORDER_RADIUS)
                    .bg(t.background)
                    .shadow(vec![BoxShadow {
                        color: hsla(0., 0., 0., 0.3),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(8.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .child(
                        uniform_list("autocompletion-rows", n, move |range, _window, cx| {
                            range
                                .map(|ix| {
                                    row(ix, &hits[ix], selected == Some(ix), on_pick.clone(), cx)
                                })
                                .collect::<Vec<_>>()
                        })
                        .track_scroll(&ac.scroll)
                        .size_full(),
                    ),
            ),
    )
    .with_priority(3)
    .into_any_element()
}

/// `.autocompletion-item` + the provider's `renderItem`.
fn row(ix: usize, hit: &Hit, selected: bool, on_pick: PickHandler, cx: &mut App) -> AnyElement {
    let t = cx.ghd();
    let (fg, secondary) = if selected {
        (t.box_selected_active_text, t.box_selected_active_text)
    } else {
        (t.text, t.text_secondary)
    };
    let mut d = div()
        .id(("autocompletion-row", ix))
        .h(ROW_HEIGHT)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .px(SPACING)
        .text_size(FONT_SIZE)
        .text_color(fg)
        .cursor_pointer()
        .when(ix > 0, |d| {
            d.border_t_1().border_color(if selected {
                t.box_selected_active_background
            } else {
                t.box_border
            })
        })
        .when(selected, |d| d.bg(t.box_selected_active_background))
        // GHD's list selects on mouse down and inserts on click; inserting on
        // mouse down keeps the popup alive across the field's blur.
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            on_pick(ix, window, cx)
        });
    d = match hit {
        Hit::Emoji(e) => {
            // `.emoji`: 20 px icon cell at 15 px, then the alias with the
            // matched substring in bold (`<mark>`), colons stripped.
            let alias = e.key.trim_matches(':').to_string();
            let title: AnyElement = if e.match_length == 0 {
                div().truncate().child(alias).into_any_element()
            } else {
                let start = e.match_start.saturating_sub(1).min(alias.len());
                let end = (start + e.match_length).min(alias.len());
                div()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(alias[..start].to_string())
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(alias[start..end].to_string()),
                    )
                    .child(alias[end..].to_string())
                    .into_any_element()
            };
            let icon: AnyElement = match &e.image {
                // GitHub's image-only emoji (`:shipit:`) from the cache
                Some(path) => img(path.clone())
                    .size(px(20.))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => div()
                    .text_size(px(15.))
                    .child(e.emoji.clone())
                    .into_any_element(),
            };
            d.child(
                div()
                    .size(px(20.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .mr(SPACING_HALF)
                    .child(icon),
            )
            .child(div().flex_1().min_w_0().overflow_hidden().child(title))
        }
        Hit::UnknownUser(name) => d
            // `.user.unknown`
            .child(
                div()
                    .flex_none()
                    .max_w_full()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .italic()
                    .mr(SPACING_HALF)
                    .child(format!("@{name}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .italic()
                    .text_size(FONT_SIZE_SM)
                    .text_color(secondary)
                    .child("Search for user"),
            ),
        Hit::Issue(i) => d
            .gap(px(4.))
            .child(
                div()
                    .flex_none()
                    .text_color(secondary)
                    .child(format!("#{}", i.number)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(i.title.clone()),
            ),
        Hit::User(u) => {
            let avatar = u.avatar_url.as_deref().and_then(|url| {
                Dispatcher::request_avatar_url(url, cx);
                avatar_lookup_url(url, cx)
            });
            d.child(
                div()
                    .mr(SPACING_HALF)
                    .child(avatar_image(avatar, px(16.), cx)),
            )
            .child(
                div()
                    .flex_none()
                    .max_w_full()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mr(SPACING_HALF)
                    .child(u.login.clone()),
            )
            .when_some(u.name.clone(), |d, name| {
                d.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(secondary)
                        .child(name),
                )
            })
        }
    };
    d.into_any_element()
}
