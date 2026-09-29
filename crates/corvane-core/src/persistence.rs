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
    /// GHD `askForConfirmationOnForcePush`.
    #[serde(default = "default_true")]
    pub confirm_force_push: bool,
    /// GHD `externalEditor`: the friendly name of the selected editor, `None`
    /// = first installed one.
    #[serde(default)]
    pub external_editor: Option<String>,
    /// GHD `shell`: the label of the selected shell, `None` = Terminal.
    #[serde(default)]
    pub shell: Option<String>,
    /// GHD `notificationsEnabled`.
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    /// GHD `confirmRepoRemoval`.
    #[serde(default = "default_true")]
    pub confirm_repository_removal: bool,
    /// GHD `askForConfirmationOnDiscardChangesPermanently`.
    #[serde(default = "default_true")]
    pub confirm_discard_changes_permanently: bool,
    /// GHD `askForConfirmationOnCommitFilteredChanges`.
    #[serde(default = "default_true")]
    pub confirm_commit_filtered_changes: bool,
    /// GHD `showCommitLengthWarning`.
    #[serde(default = "default_true")]
    pub show_commit_length_warning: bool,
    /// GHD `commitSpellcheckEnabled` (toggled from the commit form's context menu).
    pub commit_spellcheck_enabled: bool,
    /// GHD `repositoryIndicatorsEnabled` (Advanced › Background updates).
    #[serde(default = "default_true")]
    pub repository_indicators_enabled: bool,
    /// GHD `useExternalCredentialHelper` (Git Credential Manager).
    #[serde(default)]
    pub use_external_credential_helper: bool,
    /// GHD `underlineLinks` (Accessibility).
    #[serde(default)]
    pub underline_links: bool,
    /// GHD `showDiffCheckMarks` (Accessibility).
    #[serde(default = "default_true")]
    pub show_diff_check_marks: bool,
    /// GHD `tabSize` for diffs (Appearance › Diff).
    #[serde(default = "default_tab_size")]
    pub tab_size: u32,
    /// Appearance › Formatting (`dateFormat`, date-fns pattern).
    #[serde(default = "default_date_format")]
    pub date_format: String,
    /// `timeFormat`
    #[serde(default = "default_time_format")]
    pub time_format: String,
    /// `numberFormat` key: `<thousands separator>|<decimal separator>`.
    #[serde(default = "default_number_format")]
    pub number_format: String,
    /// `preferAbsoluteDates`
    #[serde(default)]
    pub prefer_absolute_dates: bool,
    /// Git › Hooks: `enableGitHookEnv` / `cacheGitHookEnv`.
    #[serde(default)]
    pub enable_git_hook_env: bool,
    #[serde(default = "default_true")]
    pub cache_git_hook_env: bool,
    /// Integrations › custom editor / shell (`customEditor`, `useCustomEditor`…).
    #[serde(default)]
    pub custom_editor: Option<CustomIntegration>,
    #[serde(default)]
    pub use_custom_editor: bool,
    #[serde(default)]
    pub custom_shell: Option<CustomIntegration>,
    #[serde(default)]
    pub use_custom_shell: bool,
}

/// GHD `ICustomIntegration`: an executable (or macOS app bundle) plus its
/// arguments; `%TARGET_PATH%` in the arguments is replaced by the repository path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomIntegration {
    pub path: String,
    pub arguments: String,
    /// Set when `path` is a `.app` bundle (launched through `open -b`).
    #[serde(default)]
    pub bundle_id: Option<String>,
}

fn default_date_format() -> String {
    DEFAULT_DATE_FORMAT.to_string()
}
fn default_time_format() -> String {
    DEFAULT_TIME_FORMAT.to_string()
}
fn default_number_format() -> String {
    DEFAULT_NUMBER_FORMAT.to_string()
}

/// GHD `defaultDateFormat` / `defaultTimeFormat` / `defaultNumberFormat`
/// (the en-US branch of GHD's locale detection).
pub const DEFAULT_DATE_FORMAT: &str = "MMM d, yyyy";
pub const DEFAULT_TIME_FORMAT: &str = "h:mm aaa";
pub const DEFAULT_NUMBER_FORMAT: &str = ",|.";

fn default_tab_size() -> u32 {
    TAB_SIZE_DEFAULT
}

/// GHD `tabSizeDefault`.
pub const TAB_SIZE_DEFAULT: u32 = 4;

fn default_true() -> bool {
    true
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
            confirm_force_push: true,
            external_editor: None,
            shell: None,
            notifications_enabled: true,
            confirm_repository_removal: true,
            confirm_discard_changes_permanently: true,
            confirm_commit_filtered_changes: true,
            show_commit_length_warning: true,
            commit_spellcheck_enabled: true,
            repository_indicators_enabled: true,
            use_external_credential_helper: false,
            underline_links: false,
            show_diff_check_marks: true,
            tab_size: TAB_SIZE_DEFAULT,
            date_format: default_date_format(),
            time_format: default_time_format(),
            number_format: default_number_format(),
            prefer_absolute_dates: false,
            enable_git_hook_env: false,
            cache_git_hook_env: true,
            custom_editor: None,
            use_custom_editor: false,
            custom_shell: None,
            use_custom_shell: false,
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
    /// Generic git server logins (host → username); passwords live in the keychain.
    fn generic_logins(&self) -> Result<std::collections::HashMap<String, String>>;
    fn save_generic_logins(&self, logins: &std::collections::HashMap<String, String>)
    -> Result<()>;
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

    fn generic_logins(&self) -> Result<std::collections::HashMap<String, String>> {
        Ok(self.get("generic_git_logins")?.unwrap_or_default())
    }

    fn save_generic_logins(
        &self,
        logins: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        self.set("generic_git_logins", logins)
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
            welcome_completed: true,
            confirm_discard_changes: false,
            confirm_force_push: false,
            external_editor: Some("Zed".into()),
            ..Settings::default()
        };
        store.save_settings(&s).unwrap();
        let back = store.settings().unwrap();
        assert_eq!(back.theme, ThemeSetting::Dark);
        assert_eq!(back.sidebar_width, 300.0);
        assert_eq!(back.external_editor.as_deref(), Some("Zed"));
        assert!(back.show_diff_check_marks && back.repository_indicators_enabled);
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
