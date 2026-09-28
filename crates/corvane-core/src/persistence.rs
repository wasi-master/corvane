//! Typed accessors over the generic `corvane_store::Store`.

use std::path::PathBuf;

use corvane_store::{Result, Store};
use serde::{Deserialize, Serialize};

use corvane_models::{Account, Repository, ThemeSetting};

/// User settings persisted across launches (subset of GHD's preferences).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeSetting,
    pub sidebar_width: f32,
    /// History file-list width (`commitSummaryWidth`, default 250).
    pub commit_summary_width: f32,
    pub clone_dir: Option<PathBuf>,
    /// GHD `hasShownWelcomeFlow`.
    pub welcome_completed: bool,
    /// GHD `askForConfirmationOnDiscardChanges`.
    pub confirm_discard_changes: bool,
    /// GHD `askForConfirmationOnCheckoutCommit`.
    pub confirm_checkout_commit: bool,
    /// GHD `askForConfirmationOnUndoCommit`.
    pub confirm_undo_commit: bool,
    /// GHD `uncommittedChangesStrategy` ("If I have changes and I switch branches…").
    pub uncommitted_changes_strategy: UncommittedChangesStrategy,
    /// GHD `askForConfirmationOnDiscardStash`.
    pub confirm_discard_stash: bool,
}

/// GHD `UncommittedChangesStrategy`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UncommittedChangesStrategy {
    #[default]
    AskForConfirmation,
    StashOnCurrentBranch,
    MoveToNewBranch,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeSetting::System,
            sidebar_width: 250.0,
            commit_summary_width: 250.0,
            clone_dir: None,
            welcome_completed: false,
            confirm_discard_changes: true,
            confirm_checkout_commit: true,
            confirm_undo_commit: true,
            uncommitted_changes_strategy: UncommittedChangesStrategy::default(),
            confirm_discard_stash: true,
        }
    }
}

/// Keys are namespaced strings; values JSON. Add a key here, never ad hoc.
pub trait StoreExt {
    fn settings(&self) -> Result<Settings>;
    fn save_settings(&self, settings: &Settings) -> Result<()>;

    fn repositories(&self) -> Result<Vec<Repository>>;
    fn save_repositories(&self, repos: &[Repository]) -> Result<()>;
    fn next_repository_id(&self) -> Result<u64>;

    fn recent_repositories(&self) -> Result<Vec<u64>>;
    fn save_recent_repositories(&self, ids: &[u64]) -> Result<()>;
    fn selected_repository(&self) -> Result<Option<u64>>;
    fn save_selected_repository(&self, id: Option<u64>) -> Result<()>;

    fn accounts(&self) -> Result<Vec<Account>>;
    fn save_accounts(&self, accounts: &[Account]) -> Result<()>;
}

impl StoreExt for Store {
    fn settings(&self) -> Result<Settings> {
        Ok(self.get("settings")?.unwrap_or_default())
    }

    fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.set("settings", settings)
    }

    fn repositories(&self) -> Result<Vec<Repository>> {
        Ok(self.get("repositories")?.unwrap_or_default())
    }

    fn save_repositories(&self, repos: &[Repository]) -> Result<()> {
        self.set("repositories", repos)
    }

    /// Monotonic, never reused.
    fn next_repository_id(&self) -> Result<u64> {
        let next: u64 = self.get("repositories.next_id")?.unwrap_or(1);
        self.set("repositories.next_id", &(next + 1))?;
        Ok(next)
    }

    /// Most recent first (GHD keeps 3).
    fn recent_repositories(&self) -> Result<Vec<u64>> {
        Ok(self.get("repositories.recent")?.unwrap_or_default())
    }

    fn save_recent_repositories(&self, ids: &[u64]) -> Result<()> {
        self.set("repositories.recent", ids)
    }

    fn selected_repository(&self) -> Result<Option<u64>> {
        self.get("ui.selected_repository")
    }

    fn save_selected_repository(&self, id: Option<u64>) -> Result<()> {
        self.set("ui.selected_repository", &id)
    }

    fn accounts(&self) -> Result<Vec<Account>> {
        Ok(self.get("accounts")?.unwrap_or_default())
    }

    fn save_accounts(&self, accounts: &[Account]) -> Result<()> {
        self.set("accounts", accounts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        assert_eq!(store.settings().unwrap().sidebar_width, 250.0);
        let s = Settings {
            theme: ThemeSetting::Dark,
            sidebar_width: 300.0,
            clone_dir: None,
            welcome_completed: true,
            commit_summary_width: 250.0,
            confirm_discard_changes: false,
            confirm_checkout_commit: true,
            confirm_undo_commit: true,
            uncommitted_changes_strategy: UncommittedChangesStrategy::default(),
            confirm_discard_stash: true,
        };
        store.save_settings(&s).unwrap();
        let back = store.settings().unwrap();
        assert_eq!(back.theme, ThemeSetting::Dark);
        assert_eq!(back.sidebar_width, 300.0);
    }

    #[test]
    fn repositories_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        assert!(store.repositories().unwrap().is_empty());
        assert_eq!(store.next_repository_id().unwrap(), 1);
        assert_eq!(store.next_repository_id().unwrap(), 2);
        let repos = vec![Repository::new(1, "/tmp/a"), Repository::new(2, "/tmp/b")];
        store.save_repositories(&repos).unwrap();
        assert_eq!(store.repositories().unwrap(), repos);
        store.save_recent_repositories(&[2, 1]).unwrap();
        assert_eq!(store.recent_repositories().unwrap(), vec![2, 1]);
        store.save_selected_repository(Some(2)).unwrap();
        assert_eq!(store.selected_repository().unwrap(), Some(2));
    }
}
