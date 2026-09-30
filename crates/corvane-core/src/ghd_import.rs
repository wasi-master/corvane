//! Import Repositories from GitHub Desktop - a Corvane extra (flag
//! `206-import-from-github-desktop`): GHD's repository list
//! (`corvane_platform::ghd_import`) less what Corvane already lists and
//! folders that are gone, added in GHD's order with their aliases.

use std::path::PathBuf;

pub use corvane_platform::ghd_import::GhdRepository;
use gpui_kit::App;

use crate::dispatcher::{Dispatcher, same_path};
use crate::remote::spawn_bg;

/// GitHub Desktop's data directory exists on this machine (looked up once
/// per launch).
pub fn github_desktop_installed() -> bool {
    static INSTALLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *INSTALLED.get_or_init(|| !corvane_platform::ghd_import::data_dirs().is_empty())
}

impl Dispatcher {
    /// Read GHD's list off the main thread; `done` gets the repositories
    /// Corvane could add (working trees still on disk that it does not list
    /// yet).
    pub fn find_ghd_repositories(
        cx: &mut App,
        done: impl FnOnce(Vec<GhdRepository>, &mut App) + 'static,
    ) {
        let known: Vec<PathBuf> = Self::state(cx)
            .read(cx)
            .repositories
            .iter()
            .map(|r| r.path.clone())
            .collect();
        spawn_bg(
            cx,
            move || {
                corvane_platform::ghd_import::repositories()
                    .into_iter()
                    .filter(|r| r.path.join(".git").exists())
                    .filter(|r| !known.iter().any(|k| same_path(k, &r.path)))
                    .collect::<Vec<_>>()
            },
            done,
        );
    }

    /// Add `repos` one after another (each is probed like Add Local
    /// Repository, so one that fails shows the usual error and ends the
    /// run), carrying GHD's alias over; the first one ends up selected.
    pub fn import_ghd_repositories(repos: Vec<GhdRepository>, cx: &mut App) {
        Self::import_next(repos, None, cx);
    }

    fn import_next(mut repos: Vec<GhdRepository>, first: Option<u64>, cx: &mut App) {
        if repos.is_empty() {
            if let Some(first) = first {
                Self::select_repository(first, cx);
            }
            return;
        }
        let repo = repos.remove(0);
        let alias = repo.alias.clone();
        Self::add_repository_then(repo.path, cx, move |id, cx| {
            if alias.is_some() {
                Self::change_repository_alias(id, alias, cx);
            }
            Self::import_next(repos, first.or(Some(id)), cx);
        });
    }
}
