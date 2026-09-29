//! The gemoji table behind `:smile:` autocompletion (GHD `lib/emoji.ts` +
//! `ui/autocompletion/emoji-autocompletion-provider.tsx`). GitHub's custom
//! image-only emoji (`:shipit:`, `:octocat:`…) come from the `/emojis` API in
//! GHD and are not bundled here.

use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::App;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;

/// One alias of a Unicode emoji, keyed the way GHD keys its map: `:alias:`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emoji {
    /// `:smile:` - the completion text.
    pub key: String,
    /// The emoji itself.
    pub emoji: String,
    pub description: String,
}

/// GHD `IEmojiHit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiHit {
    pub key: String,
    /// The Unicode emoji; empty for GitHub's image-only emoji.
    pub emoji: String,
    /// Cached image of an image-only emoji (`emoji.url` in GHD).
    pub image: Option<PathBuf>,
    /// Offset of the match inside `key` (includes the leading colon).
    pub match_start: usize,
    /// Zero when the filter was empty and every emoji is listed.
    pub match_length: usize,
}

static TABLE: LazyLock<Vec<Emoji>> = LazyLock::new(|| {
    let raw: Vec<(String, String, String)> =
        serde_json::from_str(include_str!("../../../assets/emoji/gemoji.json")).unwrap_or_default();
    raw.into_iter()
        .map(|(alias, emoji, description)| Emoji {
            key: format!(":{alias}:"),
            emoji,
            description,
        })
        .collect()
});

/// Every bundled emoji in gemoji order.
pub fn all() -> &'static [Emoji] {
    &TABLE
}

/// GitHub's image-only emoji (`:shipit:`, `:octocat:`, …) from the `/emojis`
/// API, cached as PNGs under the cache directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomEmoji {
    pub name: String,
    pub path: PathBuf,
}

static CUSTOM: RwLock<Vec<CustomEmoji>> = RwLock::new(Vec::new());

/// The image-only emoji loaded so far (empty until [`Dispatcher::load_custom_emoji`] ran).
pub fn custom() -> Vec<CustomEmoji> {
    CUSTOM.read().map(|c| c.clone()).unwrap_or_default()
}

/// `cache_dir/emoji/index.json`: which names were downloaded and when.
#[derive(Debug, Serialize, Deserialize)]
struct CustomIndex {
    fetched_at: u64,
    names: Vec<String>,
}

const CUSTOM_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

fn emoji_cache_dir() -> PathBuf {
    corvane_platform::paths::cache_dir().join("emoji")
}

/// Load the cached image-only emoji, refreshing them from `/emojis` (public,
/// GitHub.com only) once a week. Done off the main thread at startup.
fn load_custom_emoji_blocking() -> Vec<CustomEmoji> {
    let dir = emoji_cache_dir();
    let index_path = dir.join("index.json");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cached: Option<CustomIndex> = std::fs::read(&index_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let from_index = |index: &CustomIndex| -> Vec<CustomEmoji> {
        index
            .names
            .iter()
            .map(|n| CustomEmoji {
                name: n.clone(),
                path: dir.join(format!("{n}.png")),
            })
            .filter(|e| e.path.exists())
            .collect()
    };
    if let Some(index) = &cached
        && now.saturating_sub(index.fetched_at) < CUSTOM_MAX_AGE.as_secs()
    {
        return from_index(index);
    }
    let endpoint = corvane_github::Endpoint::from_api_base("https://api.github.com");
    let all_emoji = match corvane_github::public_emojis(&endpoint) {
        Ok(map) => map,
        Err(err) => {
            warn!(%err, "could not fetch GitHub emoji");
            return cached.as_ref().map(from_index).unwrap_or_default();
        }
    };
    let bundled: std::collections::HashSet<&str> =
        all().iter().map(|e| e.key.trim_matches(':')).collect();
    let _ = std::fs::create_dir_all(&dir);
    let mut names = Vec::new();
    for (name, url) in all_emoji {
        // unicode emoji are bundled; the rest are GitHub's own images
        if bundled.contains(name.as_str()) || url.contains("/unicode/") {
            continue;
        }
        let path = dir.join(format!("{name}.png"));
        if !path.exists() {
            match corvane_github::download(&url) {
                Ok(bytes) => {
                    if std::fs::write(&path, bytes).is_err() {
                        continue;
                    }
                }
                Err(err) => {
                    warn!(%err, name, "could not download emoji image");
                    continue;
                }
            }
        }
        names.push(name);
    }
    names.sort();
    let index = CustomIndex {
        fetched_at: now,
        names,
    };
    if let Ok(json) = serde_json::to_vec(&index) {
        let _ = std::fs::write(&index_path, json);
    }
    info!(count = index.names.len(), "custom emoji ready");
    from_index(&index)
}

impl Dispatcher {
    /// Startup: make `:shipit:` and friends available to the autocompletion.
    pub fn load_custom_emoji(cx: &mut App) {
        spawn_bg(cx, load_custom_emoji_blocking, |list, cx| {
            if let Ok(mut c) = CUSTOM.write() {
                *c = list;
            }
            Self::state(cx).update(cx, |_, cx| cx.notify());
        });
    }
}

/// GHD `getAutocompletionItems`: substring match on the `:alias:` key,
/// sorted by match position, then key length, then alphabetically.
pub fn matches(text: &str, max_hits: usize) -> Vec<EmojiHit> {
    let custom = custom();
    let entries = all()
        .iter()
        .map(|e| (e.key.clone(), e.emoji.clone(), None))
        .chain(
            custom
                .iter()
                .map(|c| (format!(":{}:", c.name), String::new(), Some(c.path.clone()))),
        );
    if text.is_empty() {
        return entries
            .take(max_hits)
            .map(|(key, emoji, image)| EmojiHit {
                key,
                emoji,
                image,
                match_start: 0,
                match_length: 0,
            })
            .collect();
    }
    let needle = text.to_lowercase();
    let mut hits: Vec<EmojiHit> = entries
        .filter_map(|(key, emoji, image)| {
            key.find(&needle).map(|ix| EmojiHit {
                match_start: ix,
                match_length: needle.len(),
                key,
                emoji,
                image,
            })
        })
        .collect();
    hits.sort_by(|x, y| {
        x.match_start
            .cmp(&y.match_start)
            .then(x.key.len().cmp(&y.key.len()))
            .then(x.key.cmp(&y.key))
    });
    hits.truncate(max_hits);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_loads() {
        assert!(all().len() > 1500);
        assert!(all().iter().any(|e| e.key == ":smile:" && e.emoji == "😄"));
    }

    #[test]
    fn empty_filter_lists_in_table_order() {
        let hits = matches("", 3);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].key, ":grinning:");
        assert_eq!(hits[0].match_length, 0);
    }

    #[test]
    fn shorter_keys_win_ties() {
        let hits = matches("heart", 25);
        assert_eq!(hits[0].key, ":heart:");
        let pos = |key: &str| hits.iter().position(|h| h.key == key);
        // :heart_eyes: (match at 1) sorts before :broken_heart: (match at 8)
        assert!(pos(":heart_eyes:") < pos(":broken_heart:"));
    }
}
