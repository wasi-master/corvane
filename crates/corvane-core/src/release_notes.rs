//! Release notes - GHD `lib/release-notes.ts` (`parseReleaseEntries`,
//! `getReleaseSummary`) and `models/release-notes.ts`, fed from the GitHub
//! Releases API instead of GHD's changelog feed: the release tagged
//! `v<running version>` of Corvane's repository, fetched without a token.
//!
//! GHD's feed gives one string per entry (`[Fixed] Message - #123`). A release
//! body is Markdown, so each list item is one entry: `[Kind] message` keeps
//! GHD's kinds; an untagged item takes its kind from the `##` heading above it
//! ("Fixed", "Bug fixes" → Bugfixes; "New", "Added", "Improved", "Features",
//! "Enhancements" → Enhancements) and is Other otherwise (GHD drops untagged
//! entries). Plain paragraphs before the first list are the pretext.

use std::time::SystemTime;

use corvane_github::{Client, Endpoint};
use gpui_kit::App;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// Corvane's repository (release feed).
const RELEASES_OWNER: &str = "wasi-master";
const RELEASES_REPO: &str = "corvane";
/// GHD `ReleaseNotesUri`.
pub const RELEASE_NOTES_URL: &str = "https://github.com/wasi-master/corvane/releases";

/// GHD `ItemEntryKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseNoteKind {
    New,
    Fixed,
    Improved,
    Removed,
    Added,
    Pretext,
    Other,
}

/// GHD `ReleaseNote`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseNote {
    pub kind: ReleaseNoteKind,
    pub message: String,
}

/// GHD `ReleaseSummary`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseSummary {
    pub latest_version: String,
    pub date_published: Option<SystemTime>,
    pub pretext: Vec<ReleaseNote>,
    pub enhancements: Vec<ReleaseNote>,
    pub bugfixes: Vec<ReleaseNote>,
    pub other: Vec<ReleaseNote>,
    pub thank_yous: Vec<ReleaseNote>,
}

fn kind_for(tag: &str) -> ReleaseNoteKind {
    match tag.to_ascii_lowercase().as_str() {
        "new" => ReleaseNoteKind::New,
        "fixed" => ReleaseNoteKind::Fixed,
        "improved" => ReleaseNoteKind::Improved,
        "removed" => ReleaseNoteKind::Removed,
        "added" => ReleaseNoteKind::Added,
        "pretext" => ReleaseNoteKind::Pretext,
        _ => ReleaseNoteKind::Other,
    }
}

/// The kind an untagged entry gets from the heading it sits under.
fn kind_for_heading(heading: &str) -> ReleaseNoteKind {
    let h = heading.to_ascii_lowercase();
    if h.contains("fix") || h.contains("bug") {
        ReleaseNoteKind::Fixed
    } else if h.contains("new") || h.contains("feature") || h.contains("enhancement") {
        ReleaseNoteKind::New
    } else if h.contains("added") {
        ReleaseNoteKind::Added
    } else if h.contains("improve") {
        ReleaseNoteKind::Improved
    } else if h.contains("removed") {
        ReleaseNoteKind::Removed
    } else {
        ReleaseNoteKind::Other
    }
}

/// GHD `parseEntry`: `[Kind] message`.
fn parse_tagged(text: &str) -> Option<ReleaseNote> {
    let rest = text.strip_prefix('[')?;
    let (tag, message) = rest.split_once(']')?;
    if tag.is_empty() || !tag.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let message = message.strip_prefix(char::is_whitespace)?;
    Some(ReleaseNote {
        kind: kind_for(tag),
        message: message.trim().to_string(),
    })
}

/// A Markdown release body → entries (see the module doc).
pub fn parse_release_body(body: &str) -> Vec<ReleaseNote> {
    let mut entries = Vec::new();
    let mut heading: Option<String> = None;
    let mut pretext: Vec<String> = Vec::new();
    let mut seen_list = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(h) = trimmed.strip_prefix('#') {
            heading = Some(h.trim_start_matches('#').trim().to_string());
            continue;
        }
        let item = ["- ", "* ", "+ "]
            .iter()
            .find_map(|b| trimmed.strip_prefix(b));
        if let Some(item) = item {
            seen_list = true;
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            entries.push(parse_tagged(item).unwrap_or_else(|| {
                ReleaseNote {
                    kind: heading
                        .as_deref()
                        .map(kind_for_heading)
                        .unwrap_or(ReleaseNoteKind::Other),
                    message: item.to_string(),
                }
            }));
        } else if !seen_list && heading.is_none() && !trimmed.is_empty() {
            pretext.push(trimmed.to_string());
        }
    }
    if !pretext.is_empty() {
        entries.insert(
            0,
            ReleaseNote {
                kind: ReleaseNoteKind::Pretext,
                message: pretext.join(" "),
            },
        );
    }
    entries
}

/// GHD `getReleaseSummary`.
pub fn release_summary(
    version: &str,
    published: Option<SystemTime>,
    entries: Vec<ReleaseNote>,
) -> ReleaseSummary {
    use ReleaseNoteKind::*;
    let of = |kinds: &[ReleaseNoteKind]| -> Vec<ReleaseNote> {
        entries
            .iter()
            .filter(|e| kinds.contains(&e.kind))
            .cloned()
            .collect()
    };
    ReleaseSummary {
        latest_version: version.to_string(),
        date_published: published,
        pretext: of(&[Pretext]),
        enhancements: of(&[New, Added, Improved]),
        bugfixes: of(&[Fixed]),
        other: of(&[Removed, Other]),
        thank_yous: entries
            .iter()
            .filter(|e| e.message.contains(" Thanks @"))
            .cloned()
            .collect(),
    }
}

impl Dispatcher {
    /// Help › Show Release Notes: fetch the running version's release and
    /// open the `ReleaseNotes` popup.
    pub fn show_release_notes(cx: &mut App) {
        let version = env!("CARGO_PKG_VERSION").to_string();
        spawn_bg(
            cx,
            {
                let version = version.clone();
                move || {
                    Client::new(Endpoint::github_com(), "")
                        .release_by_tag(RELEASES_OWNER, RELEASES_REPO, &format!("v{version}"))
                        .map_err(|err| err.to_string())
                }
            },
            move |result, cx| match result {
                Ok(Some(release)) => {
                    let published = release
                        .published_at
                        .as_deref()
                        .and_then(corvane_models::parse_iso8601);
                    let entries = parse_release_body(release.body.as_deref().unwrap_or_default());
                    Self::show_popup(
                        Popup::ReleaseNotes {
                            summary: release_summary(&version, published, entries),
                        },
                        cx,
                    );
                }
                Ok(None) => Self::show_error(
                    "Release Notes",
                    format!("There are no release notes for Corvane {version} on GitHub."),
                    cx,
                ),
                Err(err) => Self::show_error(
                    "Release Notes",
                    format!("The release notes could not be loaded: {err}"),
                    cx,
                ),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_entries_follow_ghd() {
        let notes = parse_release_body(
            "- [New] Branch autocompletion - #12\n- [Fixed] Crash on launch. Thanks @octocat!\n- [Removed] Old thing\n- [Weird] Something",
        );
        let summary = release_summary("0.2.0", None, notes);
        assert_eq!(summary.enhancements.len(), 1);
        assert_eq!(
            summary.enhancements[0].message,
            "Branch autocompletion - #12"
        );
        assert_eq!(summary.bugfixes.len(), 1);
        assert_eq!(summary.thank_yous.len(), 1);
        // `removed` and unknown kinds are Other
        assert_eq!(summary.other.len(), 2);
    }

    #[test]
    fn headings_classify_untagged_items_and_pretext_is_kept() {
        let body = "A smaller release.\n\n## Features\n* Split diff\n\n### Bug fixes\n- Scrolling jank\n\n## Misc\n- Docs";
        let summary = release_summary("0.2.0", None, parse_release_body(body));
        assert_eq!(summary.pretext.len(), 1);
        assert_eq!(summary.pretext[0].message, "A smaller release.");
        assert_eq!(summary.enhancements[0].message, "Split diff");
        assert_eq!(summary.bugfixes[0].message, "Scrolling jank");
        assert_eq!(summary.other[0].message, "Docs");
    }

    #[test]
    fn tag_needs_letters_and_a_space() {
        assert_eq!(parse_tagged("[x]y"), None);
        assert_eq!(parse_tagged("[12] y"), None);
        assert_eq!(
            parse_tagged("[fixed] y").map(|n| n.kind),
            Some(ReleaseNoteKind::Fixed)
        );
    }
}
