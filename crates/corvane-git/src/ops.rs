//! Write-side operations via the git CLI: init, initial commit, clone with
//! progress, path validation. Semantics follow GitHub Desktop's `lib/git/*`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::info;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// What a user-typed path looks like for Add / Create dialogs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathStatus {
    /// Nothing there (or not a directory).
    Missing,
    /// Directory exists but has no `.git`.
    NotARepository,
    /// Bare repository (GHD: "Bare repositories are not currently supported").
    Bare,
    /// A worktree with a `.git` directory or file (submodule/worktree pointer).
    Repository,
}

pub fn path_status(path: &Path) -> PathStatus {
    if !path.is_dir() {
        return PathStatus::Missing;
    }
    let dot_git = path.join(".git");
    if dot_git.exists() {
        return PathStatus::Repository;
    }
    // bare layout: HEAD + objects + refs at the top level
    if path.join("HEAD").is_file() && path.join("objects").is_dir() && path.join("refs").is_dir() {
        return PathStatus::Bare;
    }
    PathStatus::NotARepository
}

/// Turn `owner/name` or a GitHub URL without scheme into a clone URL
/// (GHD `parseRepositoryIdentifier` + `getDefaultDir`).
pub fn normalize_clone_url(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let looks_like_url = input.contains("://")
        || input.starts_with("git@")
        || input.contains(':') && !input.contains('/');
    if looks_like_url {
        return Some(input.to_string());
    }
    let trimmed = input.trim_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    match parts.as_slice() {
        [owner, name] if !owner.is_empty() && !name.is_empty() => {
            let name = name.strip_suffix(".git").unwrap_or(name);
            Some(format!("https://github.com/{owner}/{name}.git"))
        }
        ["github.com", owner, name] | ["www.github.com", owner, name] => {
            let name = name.strip_suffix(".git").unwrap_or(name);
            Some(format!("https://github.com/{owner}/{name}.git"))
        }
        _ => None,
    }
}

/// Last path segment of a clone URL without `.git` (GHD `getDefaultDir`).
pub fn repository_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub struct InitOptions {
    pub path: PathBuf,
    pub default_branch: Option<String>,
    pub description: Option<String>,
    pub readme: bool,
}

/// `git init` (+ README + initial commit when requested). Returns the workdir.
pub fn init_repository(git: Arc<GitBinary>, opts: InitOptions) -> Result<PathBuf> {
    std::fs::create_dir_all(&opts.path).map_err(crate::error::GitError::Spawn)?;
    let mut args = vec!["init".to_string()];
    if let Some(branch) = &opts.default_branch {
        args.push("-b".into());
        args.push(branch.clone());
    }
    GitCommand::new(git.clone())
        .args(args)
        .current_dir(&opts.path)
        .run()?;
    if let Some(desc) = &opts.description {
        let _ = std::fs::write(opts.path.join(".git/description"), format!("{desc}\n"));
    }
    if opts.readme {
        let name = opts
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut readme = format!("# {name}\n");
        if let Some(desc) = &opts.description {
            if !desc.trim().is_empty() {
                readme.push_str(&format!("\n{desc}\n"));
            }
        }
        std::fs::write(opts.path.join("README.md"), readme)
            .map_err(crate::error::GitError::Spawn)?;
        GitCommand::new(git.clone())
            .args(["add", "--", "README.md"])
            .current_dir(&opts.path)
            .run()?;
        GitCommand::new(git)
            .args(["commit", "-q", "-m", "Initial commit"])
            .current_dir(&opts.path)
            .run()?;
    }
    info!(path = %opts.path.display(), "initialised repository");
    Ok(opts.path)
}

/// `git config --global --get user.name/email`.
pub fn global_identity(git: Arc<GitBinary>) -> corvane_models::Identity {
    let get = |key: &str| {
        GitCommand::new(git.clone())
            .args(["config", "--global", "--get", key])
            .allow_exit_code(1)
            .run()
            .ok()
            .and_then(|o| o.stdout_string().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    corvane_models::Identity {
        name: get("user.name"),
        email: get("user.email"),
    }
}

/// `git config --global user.name <name>` / `user.email <email>` (skips empty values).
pub fn set_global_identity(git: Arc<GitBinary>, name: &str, email: &str) -> Result<()> {
    if !name.trim().is_empty() {
        GitCommand::new(git.clone())
            .args(["config", "--global", "user.name", name.trim()])
            .run()?;
    }
    if !email.trim().is_empty() {
        GitCommand::new(git)
            .args(["config", "--global", "user.email", email.trim()])
            .run()?;
    }
    Ok(())
}

/// Progress reported while cloning: phase text + overall fraction (0..1).
#[derive(Clone, Debug, PartialEq)]
pub struct CloneProgress {
    pub description: String,
    /// `None` = indeterminate.
    pub value: Option<f32>,
}

/// GHD `CloneProgressParser` phase weights (`lib/progress/clone.ts`).
const PHASES: &[(&str, f32)] = &[
    ("remote: Compressing objects", 0.1),
    ("Receiving objects", 0.6),
    ("Resolving deltas", 0.1),
    ("Checking out files", 0.2),
    ("Updating files", 0.2),
];

pub fn parse_clone_progress(line: &str) -> CloneProgress {
    let line = line.trim();
    let mut start = 0.0f32;
    for (title, weight) in PHASES {
        if let Some(rest) = line.strip_prefix(title) {
            let percent = rest
                .trim_start_matches(':')
                .trim()
                .split('%')
                .next()
                .and_then(|p| p.trim().parse::<f32>().ok())
                .unwrap_or(0.0)
                / 100.0;
            return CloneProgress {
                description: line.to_string(),
                value: Some((start + weight * percent).clamp(0.0, 1.0)),
            };
        }
        start += weight;
    }
    CloneProgress {
        description: line.to_string(),
        value: None,
    }
}

/// `git clone --progress --recurse-submodules <url> <path>` streaming progress.
pub fn clone(
    git: Arc<GitBinary>,
    url: &str,
    path: &Path,
    mut on_progress: impl FnMut(CloneProgress),
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(crate::error::GitError::Spawn)?;
    }
    info!(url, path = %path.display(), "cloning");
    GitCommand::new(git)
        .args(["clone", "--progress", "--recurse-submodules", "--", url])
        .arg(path)
        .run_streaming(|line| on_progress(parse_clone_progress(line)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_clone_inputs() {
        assert_eq!(
            normalize_clone_url("wasi-master/corvane").as_deref(),
            Some("https://github.com/wasi-master/corvane.git")
        );
        assert_eq!(
            normalize_clone_url("github.com/wasi-master/corvane.git").as_deref(),
            Some("https://github.com/wasi-master/corvane.git")
        );
        assert_eq!(
            normalize_clone_url("git@github.com:a/b.git").as_deref(),
            Some("git@github.com:a/b.git")
        );
        assert_eq!(
            normalize_clone_url("https://example.com/x/y").as_deref(),
            Some("https://example.com/x/y")
        );
        assert!(normalize_clone_url("nope").is_none());
        assert!(normalize_clone_url("").is_none());
    }

    #[test]
    fn repository_names() {
        assert_eq!(
            repository_name_from_url("https://github.com/a/b.git").as_deref(),
            Some("b")
        );
        assert_eq!(
            repository_name_from_url("git@github.com:a/b").as_deref(),
            Some("b")
        );
        assert_eq!(
            repository_name_from_url("https://x/y/").as_deref(),
            Some("y")
        );
    }

    #[test]
    fn clone_progress_phases() {
        let p = parse_clone_progress("Receiving objects:  50% (500/1000), 1.2 MiB | 3 MiB/s");
        assert!((p.value.unwrap() - 0.4).abs() < 0.001);
        let p = parse_clone_progress("Resolving deltas: 100% (10/10), done.");
        assert!((p.value.unwrap() - 0.8).abs() < 0.001);
        assert!(parse_clone_progress("Cloning into 'x'...").value.is_none());
    }

    #[test]
    fn path_status_kinds() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(path_status(&dir.path().join("nope")), PathStatus::Missing);
        assert_eq!(path_status(dir.path()), PathStatus::NotARepository);
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert_eq!(path_status(dir.path()), PathStatus::Repository);
    }

    #[test]
    fn init_with_readme_commits() {
        let dir = tempfile::tempdir().unwrap();
        let git = Arc::new(crate::find_git().unwrap());
        let path = dir.path().join("new-repo");
        // identity for the commit
        unsafe {
            std::env::set_var("GIT_AUTHOR_NAME", "T");
            std::env::set_var("GIT_AUTHOR_EMAIL", "t@example.com");
            std::env::set_var("GIT_COMMITTER_NAME", "T");
            std::env::set_var("GIT_COMMITTER_EMAIL", "t@example.com");
        }
        init_repository(
            git,
            InitOptions {
                path: path.clone(),
                default_branch: Some("main".into()),
                description: Some("desc".into()),
                readme: true,
            },
        )
        .unwrap();
        let info = crate::open_repository(&path).unwrap();
        assert_eq!(info.current_branch().unwrap().name, "main");
        assert!(path.join("README.md").exists());
    }
}
