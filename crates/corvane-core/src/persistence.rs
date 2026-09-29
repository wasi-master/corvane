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
    /// GHD `confirmWorktreeRemoval` (Prompts › Removing worktrees).
    pub confirm_worktree_removal: bool,
    /// GHD `askToMoveToApplicationsFolder` ("Do not show this message again"
    /// in the Move to Applications prompt clears it).
    pub ask_to_move_to_applications_folder: bool,
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
    /// Diff Settings › Hide Whitespace Changes (`hideWhitespaceInChangesDiff`).
    pub hide_whitespace_in_changes_diff: bool,
    /// Same for the History tab (`hideWhitespaceInHistoryDiff`).
    pub hide_whitespace_in_history_diff: bool,
    /// Same for the Preview Pull Request dialog (`hideWhitespaceInPullRequestDiff`).
    #[serde(default)]
    pub hide_whitespace_in_pull_request_diff: bool,
    /// Preview Pull Request file-list width (`pullRequestFileListWidth`, default 250).
    #[serde(default = "default_file_list_width")]
    pub pull_request_file_list_width: f32,
    /// Diff Settings › Diff display › Split (`showSideBySideDiff`).
    pub show_side_by_side_diff: bool,
    /// Last chosen tab of a modified-image diff (`imageDiffType`).
    pub image_diff_type: corvane_models::ImageDiffType,
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
    default_time_format_for(locale_country().as_deref())
}
fn default_number_format() -> String {
    default_number_format_for(locale_country().as_deref())
}

/// The OS locale's country, read once.
fn locale_country() -> Option<String> {
    static COUNTRY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    COUNTRY
        .get_or_init(corvane_platform::locale::country_code)
        .clone()
}

/// GHD `defaultDateFormat`: the same everywhere.
pub const DEFAULT_DATE_FORMAT: &str = "MMM d, yyyy";
/// GHD's en-US defaults (used when the locale is unknown).
pub const DEFAULT_TIME_FORMAT: &str = "h:mm aaa";
pub const DEFAULT_NUMBER_FORMAT: &str = ",|.";

/// GHD `twelveHourCountries`.
const TWELVE_HOUR_COUNTRIES: &[&str] = &[
    "GB", "IE", "US", "CA", "AU", "NZ", "ZA", "IN", "PK", "BD", "PH", "MX", "CO",
];
/// GHD `decimalPointCountries`.
const DECIMAL_POINT_COUNTRIES: &[&str] = &[
    "AU", "BS", "BD", "BW", "AI", "AG", "BB", "BM", "VG", "KY", "DM", "GD", "JM", "MS", "KN", "LC",
    "VC", "TT", "TC", "GY", "BZ", "KH", "CA", "CN", "CY", "DO", "EG", "SV", "ET", "GH", "GT", "HN",
    "HK", "IN", "IE", "IL", "JP", "JO", "KE", "KP", "KR", "LY", "LI", "MO", "MY", "MV", "MT", "MX",
    "MM", "NA", "NP", "NZ", "NI", "NG", "PK", "PA", "PH", "RW", "QA", "SA", "SG", "SO", "LK", "CH",
    "SY", "TW", "TZ", "TH", "UG", "AE", "GB", "US",
];
const COMMA_GROUPING_COUNTRIES: &[&str] = &["US", "GB", "TH"];
const SPACE_GROUPING_COUNTRIES: &[&str] = &["CA", "DK", "FI", "SE", "FR", "DE"];
const DOT_GROUPING_COUNTRIES: &[&str] = &["IT", "NO", "ES"];

/// GHD `defaultTimeFormat`: 12-hour in the countries that use it (and when
/// the locale is unknown), 24-hour elsewhere.
pub fn default_time_format_for(country: Option<&str>) -> String {
    match country {
        None => DEFAULT_TIME_FORMAT.to_string(),
        Some(c) if TWELVE_HOUR_COUNTRIES.contains(&c) => DEFAULT_TIME_FORMAT.to_string(),
        Some(_) => "HH:mm".to_string(),
    }
}

/// GHD `defaultNumberFormat` (`thousands|decimal`).
pub fn default_number_format_for(country: Option<&str>) -> String {
    let Some(c) = country else {
        return DEFAULT_NUMBER_FORMAT.to_string();
    };
    let decimal = if DECIMAL_POINT_COUNTRIES.contains(&c) {
        "."
    } else {
        ","
    };
    let thousands = if COMMA_GROUPING_COUNTRIES.contains(&c) {
        ","
    } else if SPACE_GROUPING_COUNTRIES.contains(&c) {
        " "
    } else if DOT_GROUPING_COUNTRIES.contains(&c) {
        "."
    } else {
        ""
    };
    format!("{thousands}|{decimal}")
}

fn default_file_list_width() -> f32 {
    250.0
}

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
            confirm_worktree_removal: true,
            ask_to_move_to_applications_folder: true,
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
            hide_whitespace_in_changes_diff: false,
            hide_whitespace_in_history_diff: false,
            hide_whitespace_in_pull_request_diff: false,
            pull_request_file_list_width: 250.0,
            show_side_by_side_diff: false,
            image_diff_type: corvane_models::ImageDiffType::TwoUp,
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

    #[test]
    fn formatting_defaults_follow_the_country() {
        assert_eq!(default_time_format_for(None), "h:mm aaa");
        assert_eq!(default_time_format_for(Some("US")), "h:mm aaa");
        assert_eq!(default_time_format_for(Some("DE")), "HH:mm");
        assert_eq!(default_number_format_for(None), ",|.");
        assert_eq!(default_number_format_for(Some("US")), ",|.");
        assert_eq!(default_number_format_for(Some("DE")), " |,");
        assert_eq!(default_number_format_for(Some("IT")), ".|,");
        assert_eq!(default_number_format_for(Some("BR")), "|,");
    }
}
