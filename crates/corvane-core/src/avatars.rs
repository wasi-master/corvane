//! Avatar cache (GHD `ui/lib/avatar.tsx` + `AvatarStore`): commit authors
//! resolve through GitHub's e-mail avatar endpoint, signed-in accounts
//! through their API `avatar_url`. Images land in `~/Library/Caches/Corvane/avatars`
//! and are loaded from there afterwards.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui_kit::App;
use tracing::debug;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;

/// One cached (or in-flight) avatar, keyed by lower-case e-mail or `url:<url>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AvatarEntry {
    pub path: Option<PathBuf>,
    pub loading: bool,
}

pub type Avatars = HashMap<String, AvatarEntry>;

const AVATAR_SIZE: u32 = 64;

/// `getAvatarUrlCandidates` for a commit author e-mail.
fn candidates_for_email(email: &str, accounts: &[corvane_models::Account]) -> Vec<String> {
    let mut out = Vec::new();
    for account in accounts {
        if account.emails.iter().any(|e| e.eq_ignore_ascii_case(email))
            && let Some(url) = &account.avatar_url
        {
            out.push(with_size(url));
        }
    }
    // GitHub's e-mail avatar endpoint falls back to Gravatar server-side.
    out.push(format!(
        "https://avatars.githubusercontent.com/u/e?email={}&s={AVATAR_SIZE}",
        urlencode(email)
    ));
    out
}

fn with_size(url: &str) -> String {
    if url.contains('?') {
        format!("{url}&s={AVATAR_SIZE}")
    } else {
        format!("{url}?s={AVATAR_SIZE}")
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn cache_file(url: &str) -> PathBuf {
    // FNV-1a keeps the file name short and stable per URL.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    corvane_platform::paths::cache_dir()
        .join("avatars")
        .join(format!("{hash:016x}"))
}

/// Download the first candidate that answers; cached files win.
fn fetch(candidates: Vec<String>) -> Option<PathBuf> {
    for url in candidates {
        let file = cache_file(&url);
        if file.is_file() {
            return Some(file);
        }
        match corvane_github::download(&url) {
            Ok(bytes) if !bytes.is_empty() => {
                if let Some(dir) = file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if std::fs::write(&file, bytes).is_ok() {
                    return Some(file);
                }
            }
            Ok(_) => {}
            Err(err) => debug!(%url, %err, "avatar download failed"),
        }
    }
    None
}

impl Dispatcher {
    fn request_avatar(key: String, candidates: Vec<String>, cx: &mut App) {
        let started = Self::state(cx).update(cx, |s, cx| {
            if s.avatars.contains_key(&key) {
                return false;
            }
            s.avatars.insert(
                key.clone(),
                AvatarEntry {
                    path: None,
                    loading: true,
                },
            );
            cx.notify();
            true
        });
        if !started {
            return;
        }
        spawn_bg(
            cx,
            move || fetch(candidates),
            move |path, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.avatars.insert(
                        key,
                        AvatarEntry {
                            path,
                            loading: false,
                        },
                    );
                    cx.notify();
                });
            },
        );
    }

    /// Commit author / committer avatar by e-mail (no-op once requested).
    pub fn request_avatar_for_email(email: &str, cx: &mut App) {
        let email = email.trim().to_lowercase();
        if email.is_empty() {
            return;
        }
        if Self::state(cx).read(cx).avatars.contains_key(&email) {
            return;
        }
        let accounts = Self::state(cx).read(cx).accounts.clone();
        let candidates = candidates_for_email(&email, &accounts);
        Self::request_avatar(email, candidates, cx);
    }

    /// An account's own `avatar_url`.
    pub fn request_avatar_url(url: &str, cx: &mut App) {
        let key = format!("url:{url}");
        if Self::state(cx).read(cx).avatars.contains_key(&key) {
            return;
        }
        Self::request_avatar(key, vec![with_size(url)], cx);
    }
}

/// Cached image path for an e-mail, if resolved.
pub fn avatar_for_email(avatars: &Avatars, email: &str) -> Option<PathBuf> {
    avatars
        .get(&email.trim().to_lowercase())
        .and_then(|e| e.path.clone())
}

/// Cached image path for an account `avatar_url`, if resolved.
pub fn avatar_for_url(avatars: &Avatars, url: &str) -> Option<PathBuf> {
    avatars
        .get(&format!("url:{url}"))
        .and_then(|e| e.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_candidates_end_with_the_github_endpoint() {
        let c = candidates_for_email("a+b@example.com", &[]);
        assert_eq!(
            c,
            vec!["https://avatars.githubusercontent.com/u/e?email=a%2Bb%40example.com&s=64"]
        );
        assert_ne!(cache_file("x"), cache_file("y"));
    }
}
