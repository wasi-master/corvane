//! Application state, models and actions shared by UI and backends.
//! Mirrors GitHub Desktop's `AppStore` / `RepositoryStateCache` split.

use serde::{Deserialize, Serialize};

/// Which sidebar section is shown (`RepositorySectionTab`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Section {
    #[default]
    Changes,
    History,
}

/// Settings › Appearance › Theme (`ApplicationTheme`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSetting {
    Light,
    Dark,
    #[default]
    System,
}
