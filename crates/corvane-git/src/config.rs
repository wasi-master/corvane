//! Git config reads and writes for the Settings dialogs - GHD `lib/git/config.ts`
//! (`getConfigValue`, `getGlobalConfigValue`, `setConfigValue`,
//! `setGlobalConfigValue`, `removeConfigValue`, `setDefaultBranch`).

use std::path::Path;
use std::sync::Arc;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

fn value_of(cmd: GitCommand) -> Option<String> {
    cmd.allow_exit_code(1)
        .run()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| o.stdout_string().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `git config --local --get <key>` inside `workdir`.
pub fn local_config_value(git: Arc<GitBinary>, workdir: &Path, key: &str) -> Option<String> {
    value_of(
        GitCommand::new(git)
            .args(["config", "--local", "--get", key])
            .current_dir(workdir),
    )
}

/// `git config --global --get <key>`.
pub fn global_config_value(git: Arc<GitBinary>, key: &str) -> Option<String> {
    value_of(GitCommand::new(git).args(["config", "--global", "--get", key]))
}

/// `git config --local <key> <value>`.
pub fn set_local_config_value(
    git: Arc<GitBinary>,
    workdir: &Path,
    key: &str,
    value: &str,
) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--local", key, value])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `git config --global <key> <value>`.
pub fn set_global_config_value(git: Arc<GitBinary>, key: &str, value: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--global", key, value])
        .run()?;
    Ok(())
}

/// `git config --local --unset <key>`; a missing key (exit 5) is not an error.
pub fn remove_local_config_value(git: Arc<GitBinary>, workdir: &Path, key: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--local", "--unset", key])
        .current_dir(workdir)
        .allow_exit_code(5)
        .run()?;
    Ok(())
}

/// GHD `setDefaultBranch`: `init.defaultBranch` in the global config.
pub fn set_default_branch(git: Arc<GitBinary>, name: &str) -> Result<()> {
    set_global_config_value(git, "init.defaultBranch", name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::find_git;

    #[test]
    fn local_values_round_trip() {
        let git = Arc::new(find_git().expect("git"));
        let dir = tempfile::tempdir().unwrap();
        GitCommand::new(git.clone())
            .args(["init", "-q"])
            .current_dir(dir.path())
            .run()
            .unwrap();
        assert_eq!(
            local_config_value(git.clone(), dir.path(), "user.name"),
            None
        );
        set_local_config_value(git.clone(), dir.path(), "user.name", "Local Ada").unwrap();
        assert_eq!(
            local_config_value(git.clone(), dir.path(), "user.name").as_deref(),
            Some("Local Ada")
        );
        remove_local_config_value(git.clone(), dir.path(), "user.name").unwrap();
        remove_local_config_value(git.clone(), dir.path(), "user.name").unwrap();
        assert_eq!(local_config_value(git, dir.path(), "user.name"), None);
    }
}
