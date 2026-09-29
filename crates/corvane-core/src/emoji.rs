//! The gemoji table behind `:smile:` autocompletion (GHD `lib/emoji.ts` +
//! `ui/autocompletion/emoji-autocompletion-provider.tsx`). GitHub's custom
//! image-only emoji (`:shipit:`, `:octocat:`…) come from the `/emojis` API in
//! GHD and are not bundled here.

use std::sync::LazyLock;

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
    pub emoji: String,
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

/// GHD `getAutocompletionItems`: substring match on the `:alias:` key,
/// sorted by match position, then key length, then alphabetically.
pub fn matches(text: &str, max_hits: usize) -> Vec<EmojiHit> {
    if text.is_empty() {
        return all()
            .iter()
            .take(max_hits)
            .map(|e| EmojiHit {
                key: e.key.clone(),
                emoji: e.emoji.clone(),
                match_start: 0,
                match_length: 0,
            })
            .collect();
    }
    let needle = text.to_lowercase();
    let mut hits: Vec<EmojiHit> = all()
        .iter()
        .filter_map(|e| {
            e.key.find(&needle).map(|ix| EmojiHit {
                key: e.key.clone(),
                emoji: e.emoji.clone(),
                match_start: ix,
                match_length: needle.len(),
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
