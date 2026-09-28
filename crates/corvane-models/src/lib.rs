//! Domain models, named after GitHub Desktop's `app/src/models/*`.
//! Leaf crate: no gpui, no git, no network.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A repository known to Corvane (`models/repository.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub path: PathBuf,
    /// User-chosen display name (`ChangeRepositoryAlias`).
    #[serde(default)]
    pub alias: Option<String>,
    /// Set when the `origin` remote points at GitHub.com / GHES.
    #[serde(default)]
    pub github: Option<GitHubRepository>,
    /// The directory disappeared; GHD shows "Can't find" and offers Locate/Remove.
    #[serde(default)]
    pub missing: bool,
}

impl Repository {
    pub fn new(id: u64, path: impl Into<PathBuf>) -> Self {
        Self {
            id,
            path: path.into(),
            alias: None,
            github: None,
            missing: false,
        }
    }

    /// Alias, else GitHub repo name, else the directory name.
    pub fn name(&self) -> String {
        if let Some(alias) = &self.alias {
            return alias.clone();
        }
        if let Some(gh) = &self.github {
            return gh.name.clone();
        }
        dir_name(&self.path)
    }
}

pub fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// A GitHub-hosted repository (`models/github-repository.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRepository {
    /// API base, e.g. `https://api.github.com`.
    pub endpoint: String,
    pub owner: String,
    pub name: String,
    pub html_url: String,
    pub clone_url: String,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    #[serde(default)]
    pub parent: Option<Box<GitHubRepository>>,
}

impl GitHubRepository {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchKind {
    Local,
    Remote,
}

/// `models/branch.ts`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// Short name, e.g. `main` or `origin/main`.
    pub name: String,
    pub kind: BranchKind,
    /// Full ref name, e.g. `refs/heads/main`.
    pub full_name: String,
    /// Commit id the branch points at, if resolvable.
    pub tip: Option<String>,
    /// Upstream tracking branch full name (`refs/remotes/origin/main`), local branches only.
    pub upstream: Option<String>,
}

/// `models/tip.ts`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tip {
    Unknown,
    Unborn { name: String },
    Detached { sha: String },
    Valid { branch: Branch },
}

impl Tip {
    pub fn branch_name(&self) -> Option<&str> {
        match self {
            Tip::Valid { branch } => Some(&branch.name),
            Tip::Unborn { name } => Some(name),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
}

/// `models/remote.ts`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

/// A signed-in GitHub account (`models/account.ts`). The token lives in the
/// OS keychain, keyed by `endpoint` + `login`, never in the store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// API base URL, e.g. `https://api.github.com` or `https://ghe.corp/api/v3`.
    pub endpoint: String,
    pub id: u64,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub emails: Vec<String>,
    /// OAuth scopes granted to the token.
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl Account {
    pub fn is_dotcom(&self) -> bool {
        self.endpoint == "https://api.github.com"
    }

    /// Host for keychain keys and remote matching (`github.com`, `ghe.corp`).
    pub fn host(&self) -> String {
        let trimmed = self
            .endpoint
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let host = trimmed.split('/').next().unwrap_or(trimmed);
        if host == "api.github.com" {
            "github.com".to_string()
        } else {
            host.to_string()
        }
    }
}

/// Author identity from git config (`models/commit-identity.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Identity {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// What Corvane knows about an opened repository (subset of GHD's
/// `IRepositoryState`, filled by `corvane-git`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub workdir: PathBuf,
    pub tip: Tip,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub identity: Identity,
    pub ahead_behind: Option<AheadBehind>,
}

impl RepositoryInfo {
    pub fn current_branch(&self) -> Option<&Branch> {
        match &self.tip {
            Tip::Valid { branch } => Some(branch),
            _ => None,
        }
    }

    pub fn remote(&self, name: &str) -> Option<&Remote> {
        self.remotes.iter().find(|r| r.name == name)
    }
}

/// Parse `owner/name` + host out of a remote URL if it points at GitHub.
/// Handles `https://github.com/o/n(.git)`, `git@github.com:o/n(.git)`,
/// `ssh://git@github.com/o/n` and GHES hosts when `ghes_hosts` lists them.
pub fn github_from_remote(url: &str, ghes_hosts: &[String]) -> Option<GitHubRepository> {
    let (host, path) = split_remote(url)?;
    let is_dotcom = host.eq_ignore_ascii_case("github.com");
    if !is_dotcom && !ghes_hosts.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let name = parts.next()?.trim_end_matches('/').to_string();
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    let endpoint = if is_dotcom {
        "https://api.github.com".to_string()
    } else {
        format!("https://{host}/api/v3")
    };
    Some(GitHubRepository {
        endpoint,
        html_url: format!("https://{host}/{owner}/{name}"),
        clone_url: format!("https://{host}/{owner}/{name}.git"),
        owner,
        name,
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
    })
}

fn split_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("git@") {
        // git@host:owner/name
        let (host, path) = rest.split_once(':')?;
        return Some((host.to_string(), path.to_string()));
    }
    for scheme in ["https://", "http://", "ssh://", "git://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
            let (host, path) = rest.split_once('/')?;
            let host = host.split(':').next()?.to_string();
            return Some((host, path.to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_remotes() {
        for url in [
            "https://github.com/wasi-master/corvane.git",
            "https://github.com/wasi-master/corvane",
            "git@github.com:wasi-master/corvane.git",
            "ssh://git@github.com/wasi-master/corvane.git",
            "https://user@github.com/wasi-master/corvane/",
        ] {
            let gh = github_from_remote(url, &[]).unwrap_or_else(|| panic!("{url}"));
            assert_eq!(gh.owner, "wasi-master");
            assert_eq!(gh.name, "corvane");
            assert_eq!(gh.html_url, "https://github.com/wasi-master/corvane");
            assert_eq!(gh.endpoint, "https://api.github.com");
        }
    }

    #[test]
    fn ignores_non_github_and_ghes_without_config() {
        assert!(github_from_remote("https://gitlab.com/a/b.git", &[]).is_none());
        assert!(github_from_remote("git@ghe.corp:a/b.git", &[]).is_none());
        let gh = github_from_remote("git@ghe.corp:a/b.git", &["ghe.corp".into()]).unwrap();
        assert_eq!(gh.endpoint, "https://ghe.corp/api/v3");
    }

    #[test]
    fn repository_name_precedence() {
        let mut repo = Repository::new(1, "/tmp/my-dir");
        assert_eq!(repo.name(), "my-dir");
        repo.github = github_from_remote("git@github.com:o/gh-name.git", &[]);
        assert_eq!(repo.name(), "gh-name");
        repo.alias = Some("Alias".into());
        assert_eq!(repo.name(), "Alias");
    }
}

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
