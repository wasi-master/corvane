//! Editors, shells, Finder, GitHub URLs, the Settings and Repository Settings
//! dialogs' load/save, and repository removal - GHD `app-store.ts`
//! (`_openInExternalEditor`, `_openShell`, `_openInBrowser`, `_setRemoteURL`,
//! `_saveGitIgnore`, `_removeRepository`) plus `preferences.tsx#onSave` and
//! `repository-settings.tsx#onSubmit`.

use std::path::{Path, PathBuf};

use gpui_kit::App;
use tracing::{error, info, warn};

use crate::dispatcher::Dispatcher;
use crate::persistence::Settings;
use crate::remote::spawn_bg;
use crate::state::{
    GitConfigLocation, GlobalGitConfig, Popup, PreferencesTab, RepositorySettingsData,
    RepositorySettingsTab,
};
use corvane_models::{GitHubRepository, Identity};
use corvane_platform::{editors, shells, trash};

/// What Settings › Save applies (`preferences.tsx#onSave`).
#[derive(Clone, Debug)]
pub struct PreferencesSave {
    pub settings: Settings,
    pub name: String,
    pub email: String,
    pub default_branch: String,
}

/// What Repository Settings › Save applies (`repository-settings.tsx#onSubmit`).
/// Each field is `Some` only when the user changed it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RepositorySettingsSave {
    /// `(remote name, new url)`
    pub remote_url: Option<(String, String)>,
    pub gitignore: Option<String>,
    /// Where the author identity lives, with the local name/email to store.
    pub git_config: Option<(GitConfigLocation, String, String)>,
}

/// `encodeURIComponent` for branch names in GitHub URLs.
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `${htmlURL}/pull/new/${branch}` (or `${parent}/pull/new/${owner}:${branch}`
/// for a fork) - GHD `_createPullRequest`.
pub fn pull_request_url(gh: &GitHubRepository, branch: &str) -> String {
    match &gh.parent {
        Some(parent) => format!(
            "{}/pull/new/{}:{}",
            parent.html_url,
            encode_component(&gh.owner),
            encode_component(branch)
        ),
        None => format!("{}/pull/new/{}", gh.html_url, encode_component(branch)),
    }
}

impl Dispatcher {
    // ---- integrations ----

    /// Probe LaunchServices for every known editor and shell (background),
    /// then remember them for the menus and Settings › Integrations.
    pub fn detect_integrations(cx: &mut App) {
        spawn_bg(
            cx,
            || (editors::available_editors(), shells::available_shells()),
            |(editors, shells), cx| {
                info!(
                    editors = editors.len(),
                    shells = shells.len(),
                    "integrations detected"
                );
                Self::state(cx).update(cx, |s, cx| {
                    s.editors = editors;
                    s.shells = shells;
                    cx.notify();
                });
            },
        );
    }

    /// Repository › Open in <Editor> (`_openInExternalEditor`).
    pub fn open_in_editor(path: PathBuf, cx: &mut App) {
        let (editors, selected) = {
            let s = Self::state(cx).read(cx);
            (s.editors.clone(), s.settings.external_editor.clone())
        };
        let editor = match editors::find_editor_or_default(&editors, selected.as_deref()) {
            Ok(Some(editor)) => editor.clone(),
            Ok(None) => {
                Self::show_editor_error(editors::no_editor_error(), cx);
                return;
            }
            Err(err) => {
                Self::show_editor_error(err, cx);
                return;
            }
        };
        spawn_bg(
            cx,
            move || editors::launch(&editor, &path),
            |result, cx| {
                if let Err(err) = result {
                    Self::show_editor_error(err, cx);
                }
            },
        );
    }

    fn show_editor_error(err: editors::EditorError, cx: &mut App) {
        warn!(message = %err.message, "external editor");
        Self::show_popup(
            Popup::ExternalEditorError {
                message: err.message,
                suggest_default_editor: err.suggest_default_editor,
                open_preferences: err.open_preferences,
            },
            cx,
        );
    }

    /// Repository › Open in <Shell> (`_openShell`).
    pub fn open_in_shell(path: &Path, cx: &mut App) {
        let (shells, selected) = {
            let s = Self::state(cx).read(cx);
            (s.shells.clone(), s.shell_label())
        };
        let wanted = shells::Shell::parse(&selected);
        let Some(found) = shells
            .iter()
            .find(|s| s.shell == wanted)
            .or_else(|| shells.first())
            .cloned()
        else {
            Self::show_popup(
                Popup::ShellError {
                    message: format!(
                        "Could not find shell '{selected}'. Please open Settings and choose an installed shell."
                    ),
                },
                cx,
            );
            return;
        };
        let path = path.to_path_buf();
        spawn_bg(
            cx,
            move || shells::launch(&found, &path),
            |result, cx| {
                if let Err(err) = result {
                    Self::show_popup(
                        Popup::ShellError {
                            message: format!(
                                "Something went wrong while trying to start the shell: {err}"
                            ),
                        },
                        cx,
                    );
                }
            },
        );
    }

    /// Repository › Show in Finder (`revealInFileManager`).
    pub fn show_in_finder(path: &Path, cx: &mut App) {
        cx.reveal_path(path);
    }

    /// Repository › Open With… (`_openWithSystemDialog`): pick an application,
    /// then `open -a <app> <repository>`.
    pub fn open_with(path: PathBuf, cx: &mut App) {
        let receiver = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
            let app = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            if let Some(app) = app
                && let Err(err) = corvane_platform::apps::open_with_app(&app, &path)
            {
                cx.update(|cx| {
                    Self::show_error(
                        "Unable to Open Repository",
                        format!(
                            "Could not open the repository with {}: {err}",
                            app.display()
                        ),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    // ---- GitHub URLs (`_openInBrowser` callers) ----

    fn github_and_branch(id: u64, cx: &App) -> Option<(GitHubRepository, Option<String>)> {
        let s = Self::state(cx).read(cx);
        let gh = s.repository(id)?.github.clone()?;
        let branch = s
            .repo_states
            .get(&id)
            .and_then(|rs| rs.info.as_ref())
            .and_then(|info| info.current_branch())
            .map(|b| b.name.clone());
        Some((gh, branch))
    }

    /// Repository › View on GitHub.
    pub fn view_on_github(id: u64, cx: &mut App) {
        if let Some((gh, _)) = Self::github_and_branch(id, cx) {
            Self::open_url(&gh.html_url, cx);
        }
    }

    /// Repository › Create Issue on GitHub (`_openIssueCreationPage`).
    pub fn create_issue(id: u64, cx: &mut App) {
        if let Some((gh, _)) = Self::github_and_branch(id, cx) {
            Self::open_url(&format!("{}/issues/new/choose", gh.html_url), cx);
        }
    }

    /// Branch › Compare on GitHub.
    pub fn compare_on_github(id: u64, cx: &mut App) {
        if let Some((gh, Some(branch))) = Self::github_and_branch(id, cx) {
            Self::open_url(
                &format!("{}/compare/{}", gh.html_url, encode_component(&branch)),
                cx,
            );
        }
    }

    /// Branch › View Branch on GitHub.
    pub fn view_branch_on_github(id: u64, cx: &mut App) {
        if let Some((gh, Some(branch))) = Self::github_and_branch(id, cx) {
            Self::open_url(
                &format!("{}/tree/{}", gh.html_url, encode_component(&branch)),
                cx,
            );
        }
    }

    /// Branch › Create Pull Request. An unpublished branch is pushed first
    /// (GHD `_createPullRequest` → `_publishBranch`), then the compare page opens.
    pub fn create_pull_request(id: u64, cx: &mut App) {
        let Some((gh, Some(branch))) = Self::github_and_branch(id, cx) else {
            return;
        };
        let published = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.info.as_ref())
            .and_then(|info| info.current_branch())
            .is_some_and(|b| b.upstream.is_some());
        if !published {
            Self::push(id, false, None, cx);
        }
        Self::open_url(&pull_request_url(&gh, &branch), cx);
    }

    // ---- Settings (`Preferences` popup) ----

    /// Show Settings on `tab`, refresh the installed editors/shells and read the
    /// global git config in the background (`isLoadingGitConfig`).
    pub fn open_preferences(tab: PreferencesTab, cx: &mut App) {
        Self::close_foldout(cx);
        Self::state(cx).update(cx, |s, cx| {
            s.global_git = None;
            cx.notify();
        });
        Self::show_popup(Popup::Preferences { tab }, cx);
        Self::detect_integrations(cx);
        let Some(git) = Self::state(cx).read(cx).git.clone() else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let identity = corvane_git::global_identity(git.clone());
                GlobalGitConfig {
                    name: identity.name,
                    email: identity.email,
                    default_branch: corvane_git::configured_default_branch(git),
                }
            },
            |config, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.global_git = Some(config);
                    cx.notify();
                });
            },
        );
    }

    /// Settings › Save: persist the settings, then write the global git
    /// identity and default branch when they changed.
    pub fn save_preferences(save: PreferencesSave, cx: &mut App) {
        let PreferencesSave {
            settings,
            name,
            email,
            default_branch,
        } = save;
        let (git, previous) = {
            let s = Self::state(cx).read(cx);
            (s.git.clone(), s.global_git.clone().unwrap_or_default())
        };
        Self::update_settings(cx, |s| *s = settings);
        Self::close_popup(cx);
        Self::refresh_indicators(cx);
        let Some(git) = git else { return };
        let name_changed = name.trim() != previous.name.clone().unwrap_or_default().trim();
        let email_changed = email.trim() != previous.email.clone().unwrap_or_default().trim();
        let branch_changed =
            !default_branch.trim().is_empty() && default_branch.trim() != previous.default_branch;
        if !(name_changed || email_changed || branch_changed) {
            return;
        }
        let selected = Self::state(cx).read(cx).selected;
        spawn_bg(
            cx,
            move || -> Result<(), corvane_git::GitError> {
                if name_changed {
                    corvane_git::set_global_config_value(git.clone(), "user.name", name.trim())?;
                }
                if email_changed {
                    corvane_git::set_global_config_value(git.clone(), "user.email", email.trim())?;
                }
                if branch_changed {
                    corvane_git::set_default_branch(git, default_branch.trim())?;
                }
                Ok(())
            },
            move |result, cx| {
                if let Err(err) = result {
                    Self::show_error("Could not save Git configuration", err.to_string(), cx);
                }
                if let Some(id) = selected {
                    Self::refresh_repository(id, cx);
                }
            },
        );
    }

    // ---- Repository Settings ----

    /// Show Repository Settings on `tab`; the remote, `.gitignore` and git
    /// config are read in the background.
    pub fn open_repository_settings(id: u64, tab: RepositorySettingsTab, cx: &mut App) {
        Self::close_foldout(cx);
        Self::state(cx).update(cx, |s, cx| {
            s.repo_settings = None;
            cx.notify();
        });
        Self::show_popup(Popup::RepositorySettings { repo: id, tab }, cx);
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let remotes = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.info.as_ref())
            .map(|info| info.remotes.clone())
            .unwrap_or_default();
        spawn_bg(
            cx,
            move || {
                let remote = corvane_git::find_default_remote(&remotes).cloned();
                let gitignore = corvane_git::read_gitignore(&workdir)
                    .map_err(|err| warn!(%err, "could not read .gitignore"))
                    .ok()
                    .flatten();
                let global = corvane_git::global_identity(git.clone());
                let autocrlf = corvane_git::config_value(git.clone(), &workdir, "core.autocrlf")
                    .is_some_and(|v| v.eq_ignore_ascii_case("true"));
                RepositorySettingsData {
                    repo: id,
                    remote,
                    gitignore,
                    local_name: corvane_git::local_config_value(git.clone(), &workdir, "user.name"),
                    local_email: corvane_git::local_config_value(git, &workdir, "user.email"),
                    global,
                    autocrlf,
                }
            },
            |data, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_settings = Some(data);
                    cx.notify();
                });
            },
        );
    }

    /// Repository Settings › Save.
    pub fn save_repository_settings(id: u64, save: RepositorySettingsSave, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            Self::close_popup(cx);
            return;
        };
        let autocrlf = Self::state(cx)
            .read(cx)
            .repo_settings
            .as_ref()
            .is_some_and(|d| d.autocrlf);
        Self::close_popup(cx);
        spawn_bg(
            cx,
            move || {
                let mut errors: Vec<String> = Vec::new();
                if let Some((name, url)) = save.remote_url
                    && let Err(err) =
                        corvane_git::set_remote_url(git.clone(), &workdir, &name, &url)
                {
                    errors.push(format!("Failed setting the remote URL: {err}"));
                }
                if let Some(text) = save.gitignore
                    && let Err(err) = corvane_git::save_gitignore(&workdir, &text, autocrlf)
                {
                    errors.push(format!("Failed saving the .gitignore file: {err}"));
                }
                if let Some((location, name, email)) = save.git_config {
                    let result = match location {
                        GitConfigLocation::Global => corvane_git::remove_local_config_value(
                            git.clone(),
                            &workdir,
                            "user.name",
                        )
                        .and_then(|_| {
                            corvane_git::remove_local_config_value(
                                git.clone(),
                                &workdir,
                                "user.email",
                            )
                        }),
                        GitConfigLocation::Local => corvane_git::set_local_config_value(
                            git.clone(),
                            &workdir,
                            "user.name",
                            &name,
                        )
                        .and_then(|_| {
                            corvane_git::set_local_config_value(
                                git.clone(),
                                &workdir,
                                "user.email",
                                &email,
                            )
                        }),
                    };
                    if let Err(err) = result {
                        errors.push(format!("Failed saving the Git config: {err}"));
                    }
                }
                errors
            },
            move |errors, cx| {
                if !errors.is_empty() {
                    Self::show_error("Repository Settings", errors.join("\n"), cx);
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// The identity the commit form should show for a repository: local
    /// config first, then global (what `git commit` would use).
    pub fn effective_identity(data: &RepositorySettingsData) -> Identity {
        Identity {
            name: data.local_name.clone().or_else(|| data.global.name.clone()),
            email: data
                .local_email
                .clone()
                .or_else(|| data.global.email.clone()),
        }
    }

    // ---- removal ----

    /// Repository › Remove…: confirm first unless the prompt is turned off.
    pub fn request_remove_repository(id: u64, cx: &mut App) {
        let confirm = Self::state(cx).read(cx).settings.confirm_repository_removal;
        if confirm {
            Self::close_foldout(cx);
            Self::show_popup(Popup::ConfirmRemoveRepository { repo: id }, cx);
        } else {
            Self::remove_repository(id, cx);
        }
    }

    /// Remove from the list and move the directory to the Trash
    /// (`_removeRepository` with `moveToTrash`).
    pub fn remove_repository_and_trash(id: u64, cx: &mut App) {
        let path = Self::state(cx)
            .read(cx)
            .repository(id)
            .map(|r| r.path.clone());
        Self::remove_repository(id, cx);
        let Some(path) = path else { return };
        spawn_bg(
            cx,
            move || trash::move_to_trash(&path).map_err(|e| (path, e)),
            |result, cx| {
                if let Err((path, err)) = result {
                    error!(%err, path = %path.display(), "could not move repository to Trash");
                    Self::show_error(
                        "Unable to Move Repository to Trash",
                        format!("{}: {err}", path.display()),
                        cx,
                    );
                }
            },
        );
    }

    /// Settings › Git › "edit your global Git config file": open `~/.gitconfig`
    /// in the external editor (GHD opens it with the selected editor too).
    pub fn edit_global_git_config(cx: &mut App) {
        let Some(home) = dirs_home() else { return };
        let path = home.join(".gitconfig");
        if !path.exists()
            && let Err(err) = std::fs::write(&path, "")
        {
            warn!(%err, "could not create ~/.gitconfig");
        }
        Self::open_in_editor(path, cx);
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gh(parent: bool) -> GitHubRepository {
        let base = GitHubRepository {
            endpoint: "https://api.github.com".into(),
            owner: "octocat".into(),
            name: "hello".into(),
            html_url: "https://github.com/octocat/hello".into(),
            clone_url: "https://github.com/octocat/hello.git".into(),
            default_branch: Some("main".into()),
            private: false,
            fork: parent,
            parent: None,
            archived: false,
        };
        if parent {
            GitHubRepository {
                owner: "me".into(),
                html_url: "https://github.com/me/hello".into(),
                parent: Some(Box::new(base.clone())),
                ..base
            }
        } else {
            base
        }
    }

    #[test]
    fn encodes_branch_names() {
        assert_eq!(encode_component("feature/x y"), "feature%2Fx%20y");
        assert_eq!(encode_component("main"), "main");
    }

    #[test]
    fn pull_request_urls() {
        assert_eq!(
            pull_request_url(&gh(false), "feat/one"),
            "https://github.com/octocat/hello/pull/new/feat%2Fone"
        );
        assert_eq!(
            pull_request_url(&gh(true), "feat"),
            "https://github.com/octocat/hello/pull/new/me:feat"
        );
    }
}
