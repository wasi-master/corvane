//! Editors, shells, Finder, GitHub URLs, the Settings and Repository Settings
//! dialogs' load/save, and repository removal - GHD `app-store.ts`
//! (`_openInExternalEditor`, `_openShell`, `_openInBrowser`, `_setRemoteURL`,
//! `_saveGitIgnore`, `_removeRepository`) plus `preferences.tsx#onSave` and
//! `repository-settings.tsx#onSubmit`.
//!
//! Deviation: View on GitHub also opens a non-GitHub repository's default
//! remote as a web page (`remote_web_url`, `262-view-on-remote`); GHD
//! disables it.

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
    /// `239-line-endings-setting`: the `--local` `core.autocrlf` to store
    /// (`None`: remove it, so the global value applies).
    pub autocrlf: Option<Option<String>>,
}

/// `openIssueCreationPage`: GitHub's issue template chooser.
pub fn issue_creation_url(html_url: &str) -> String {
    format!("{html_url}/issues/new/choose")
}

/// The web page of a remote that is not a GitHub repository
/// (`262-view-on-remote`): an `http(s)` URL without its credentials and
/// `.git`, and SSH / scp-style / `git://` URLs as `https://host/path`.
/// `None` for local paths and anything else without a host.
pub fn remote_web_url(url: &str) -> Option<String> {
    let url = url.trim();
    let (scheme, host, path) = match ["https://", "http://"]
        .into_iter()
        .find_map(|scheme| url.strip_prefix(scheme).map(|rest| (scheme, rest)))
    {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            (scheme, host.to_string(), path.to_string())
        }
        None => {
            let (host, path) = corvane_models::split_remote(url)?;
            ("https://", host, path)
        }
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{scheme}{host}/{path}"))
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

/// GHD `_openCreatePullRequestInBrowser`: `${htmlURL}/pull/new/[base...]compare`;
/// a fork contributing to its parent prefixes both refs with `owner:name:`.
pub fn pull_request_url(
    gh: &GitHubRepository,
    compare: &str,
    base: Option<&str>,
    contributing_to_parent: bool,
) -> String {
    let base_prefix = match (&gh.parent, contributing_to_parent) {
        (Some(parent), true) => format!("{}:{}:", parent.owner, parent.name),
        _ => String::new(),
    };
    let encoded_base = base
        .map(|b| format!("{base_prefix}{}...", encode_component(b)))
        .unwrap_or_default();
    let compare_prefix = if contributing_to_parent {
        format!("{}:{}:", gh.owner, gh.name)
    } else {
        String::new()
    };
    format!(
        "{}/pull/new/{encoded_base}{compare_prefix}{}",
        gh.html_url,
        encode_component(compare)
    )
}

impl Dispatcher {
    // ---- integrations ----

    /// Probe LaunchServices for every known editor and shell (background),
    /// then remember them for the menus and Settings › Integrations.
    pub fn detect_integrations(cx: &mut App) {
        let extras = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::EXTRA_EDITORS);
        spawn_bg(
            cx,
            move || {
                (
                    editors::available_editors(extras),
                    shells::available_shells(),
                )
            },
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

    /// Flag `811`: History › Open with Default Program opens `path` as it
    /// is at `sha`, written to a read-only file under the temporary
    /// directory, rather than today's working copy.
    pub fn open_commit_file_with_default_program(id: u64, sha: String, path: String, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || -> Result<PathBuf, String> {
                let bytes = corvane_git::blob_bytes(git, &workdir, &sha, &path)
                    .map_err(|e| e.to_string())?;
                let short = &sha[..sha.len().min(12)];
                let file = historical_file_path(&std::env::temp_dir(), short, &path)
                    .ok_or_else(|| format!("Invalid path {path}"))?;
                write_read_only(&file, &bytes).map_err(|e| e.to_string())?;
                Ok(file)
            },
            |result, cx| match result {
                Ok(file) => cx.open_with_system(&file),
                Err(err) => Self::show_error("Could not open file", err, cx),
            },
        );
    }

    /// Repository › Open in <Editor> (`_openInExternalEditor`).
    pub fn open_in_editor(path: PathBuf, cx: &mut App) {
        Self::open_in_editor_at(path, None, cx);
    }

    /// `open_in_editor` at a 1-based line where the editor supports it
    /// (the diff's "Open in <Editor> at Line N", flag
    /// `diff-open-in-editor-at-line`); a custom editor opens the file.
    pub fn open_in_editor_at(path: PathBuf, line: Option<u32>, cx: &mut App) {
        let (editors, selected, custom, workspace_file) = {
            let s = Self::state(cx).read(cx);
            (
                s.editors.clone(),
                s.settings.external_editor.clone(),
                s.settings
                    .use_custom_editor
                    .then(|| s.settings.custom_editor.clone())
                    .flatten(),
                s.flags.bool(crate::flags::ids::VSCODE_WORKSPACE_FILE),
            )
        };
        if let Some(custom) = custom {
            // `launchCustomExternalEditor`
            spawn_bg(
                cx,
                move || {
                    corvane_platform::custom_integration::launch(
                        &custom.path,
                        &custom.arguments,
                        &path,
                    )
                },
                |result, cx| {
                    if let Err(message) = result {
                        Self::show_editor_error(
                            editors::EditorError {
                                message: format!(
                                    "{message} Please open {} and check your custom editor.",
                                    corvane_platform::editors::SETTINGS_LABEL
                                ),
                                suggest_default_editor: false,
                                open_preferences: true,
                            },
                            cx,
                        );
                    }
                },
            );
            return;
        }
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
            move || match line {
                Some(line) => editors::launch_at_line(&editor, &path, line),
                None => {
                    // `509-vscode-workspace-file`: a repository opens its only
                    // workspace file instead of the folder
                    let target = workspace_file
                        .then(|| editors::code_workspace_file(&editor, &path))
                        .flatten()
                        .unwrap_or(path);
                    editors::launch(&editor, &target)
                }
            },
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
        let (shells, selected, custom) = {
            let s = Self::state(cx).read(cx);
            (
                s.shells.clone(),
                s.shell_label(),
                s.settings
                    .use_custom_shell
                    .then(|| s.settings.custom_shell.clone())
                    .flatten(),
            )
        };
        if let Some(custom) = custom {
            // `launchCustomShell`
            let path = path.to_path_buf();
            spawn_bg(
                cx,
                move || {
                    corvane_platform::custom_integration::launch(
                        &custom.path,
                        &custom.arguments,
                        &path,
                    )
                },
                |result, cx| {
                    if let Err(message) = result {
                        Self::show_popup(
                            Popup::ShellError {
                                message: format!(
                                    "{message} Please open {} and check your custom shell.",
                                    corvane_platform::editors::SETTINGS_LABEL
                                ),
                            },
                            cx,
                        );
                    }
                },
            );
            return;
        }
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
                        "Could not find shell '{selected}'. Please open {} and choose an installed shell.",
                        corvane_platform::editors::SETTINGS_LABEL
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

    /// Repository › Show in Finder (`revealInFileManager`), and every Reveal
    /// in Finder item. With a `510-file-manager` application set, it opens
    /// the folder (a file's parent folder) with `open -a <app>` instead.
    pub fn show_in_finder(path: &Path, cx: &mut App) {
        let app = Self::state(cx)
            .read(cx)
            .flags
            .text(crate::flags::ids::FILE_MANAGER)
            .trim()
            .to_string();
        if app.is_empty() {
            // Linux: Electron's route (FileManager1, then xdg-open), which
            // works without a desktop portal too
            #[cfg(not(target_os = "macos"))]
            {
                let path = path.to_path_buf();
                cx.background_executor()
                    .spawn(async move {
                        if let Err(err) = corvane_platform::apps::show_item_in_folder(&path) {
                            warn!(%err, path = %path.display(), "could not show the item");
                        }
                    })
                    .detach();
            }
            #[cfg(target_os = "macos")]
            cx.reveal_path(path);
            return;
        }
        let dir = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent()
                .map_or_else(|| path.to_path_buf(), Path::to_path_buf)
        };
        if let Err(err) = corvane_platform::apps::open_with_app(Path::new(&app), &dir) {
            Self::show_error(
                "Unable to Open File Manager",
                format!("Could not open {} with {app}: {err}", dir.display()),
                cx,
            );
        }
    }

    /// Repository › Open With… (`_openWithSystemDialog`): pick an application,
    /// then `open -a <app> <repository>`. Also a changed file's "Open With…"
    /// (Corvane `713-open-file-with`), whose error names the file.
    pub fn open_with(path: PathBuf, cx: &mut App) {
        let (title, what) = if path.is_dir() {
            ("Unable to Open Repository", "the repository")
        } else {
            ("Unable to Open File", "the file")
        };
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
                        title,
                        format!("Could not open {what} with {}: {err}", app.display()),
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

    /// Repository › View on GitHub; with `262-view-on-remote` a repository
    /// that is not on GitHub opens its default remote's web page instead.
    pub fn view_on_github(id: u64, cx: &mut App) {
        if let Some((gh, _)) = Self::github_and_branch(id, cx) {
            Self::open_url(&gh.html_url, cx);
        } else if let Some(url) = Self::non_github_remote_web_url(id, cx) {
            Self::open_url(&url, cx);
        }
    }

    /// Repository › View Upstream on GitHub (Corvane addition, flag
    /// `321-view-upstream-on-github`): the parent of a fork. Nothing happens
    /// for a repository that is not a fork.
    pub fn view_upstream_on_github(id: u64, cx: &mut App) {
        let url = Self::github_and_branch(id, cx).and_then(|(gh, _)| gh.parent.map(|p| p.html_url));
        if let Some(url) = url {
            Self::open_url(&url, cx);
        }
    }

    /// The default remote's web page (`remote_web_url`) of a loaded
    /// repository that is not on GitHub, when `262-view-on-remote` is on.
    pub fn non_github_remote_web_url(id: u64, cx: &App) -> Option<String> {
        let s = Self::state(cx).read(cx);
        if !s.flags.bool(crate::flags::ids::VIEW_ON_REMOTE) || s.repository(id)?.github.is_some() {
            return None;
        }
        let info = s.repo_states.get(&id)?.info.as_ref()?;
        remote_web_url(&corvane_git::find_default_remote(&info.remotes)?.url)
    }

    /// Repository › Create Issue on GitHub (`openIssueCreationPage`): the
    /// template chooser of the repository contributions go to (the parent of
    /// a fork unless the fork is set up for its own work,
    /// `getNonForkGitHubRepository`). GitHub answers `/issues/new/choose`
    /// with the plain new-issue form when the repository has no templates,
    /// so, as in GHD, nothing is checked locally.
    pub fn create_issue(id: u64, cx: &mut App) {
        let url = Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.non_fork_github())
            .map(|gh| issue_creation_url(&gh.html_url));
        if let Some(url) = url {
            Self::open_url(&url, cx);
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
    /// (GHD `_createPullRequest` → `_publishBranch`), then the compare page
    /// opens; with an open pull request the menu shows it instead.
    pub fn create_pull_request(id: u64, cx: &mut App) {
        if Self::state(cx).read(cx).current_pull_request(id).is_some() {
            Self::show_pull_request(id, cx);
            return;
        }
        Self::create_pull_request_with_base(id, None, cx);
    }

    /// `_createPullRequest(repository, baseBranch)`: an unpublished branch
    /// or unpushed commits ask `PushBranchCommits` first.
    pub fn create_pull_request_with_base(id: u64, base: Option<String>, cx: &mut App) {
        let Some((_, Some(branch))) = Self::github_and_branch(id, cx) else {
            return;
        };
        let ahead_behind = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.ahead_behind);
        match ahead_behind {
            None => Self::show_popup(
                Popup::PushBranchCommits {
                    repo: id,
                    branch,
                    unpushed: None,
                    base,
                },
                cx,
            ),
            Some(ab) if ab.ahead > 0 => Self::show_popup(
                Popup::PushBranchCommits {
                    repo: id,
                    branch,
                    unpushed: Some(ab.ahead),
                    base,
                },
                cx,
            ),
            Some(_) => Self::open_create_pull_request_in_browser(id, base, cx),
        }
    }

    /// `PushBranchCommits.onSubmit`: push (or publish) the current branch,
    /// then open the compare page.
    pub fn push_branch_commits_and_create_pull_request(
        id: u64,
        base: Option<String>,
        cx: &mut App,
    ) {
        use crate::flags::ids;
        use crate::remote::PushOutcome;
        let (keeps_base, error_stops) = {
            let flags = &Self::state(cx).read(cx).flags;
            (
                flags.bool(ids::PUSH_BRANCH_COMMITS_KEEPS_BASE),
                flags.bool(ids::PUSH_BRANCH_COMMITS_ERROR_STOPS),
            )
        };
        // `304`: GHD's `onConfirm` drops the base picked in Preview Pull Request
        let base = base.filter(|_| keeps_base);
        Self::push_then(
            id,
            false,
            None,
            move |outcome, cx| {
                // an error dialog may have replaced the prompt
                if matches!(
                    Self::state(cx).read(cx).popup,
                    Some(Popup::PushBranchCommits { .. })
                ) {
                    Self::close_popup(cx);
                }
                // `305`: GHD opens the compare page even after a failed push
                match outcome {
                    PushOutcome::Pushed => Self::open_create_pull_request_in_browser(id, base, cx),
                    PushOutcome::Failed if !error_stops => {
                        Self::open_create_pull_request_in_browser(id, base, cx)
                    }
                    PushOutcome::Failed | PushOutcome::NotAttempted => {}
                }
            },
            cx,
        );
    }

    /// `_openCreatePullRequestInBrowser`
    pub fn open_create_pull_request_in_browser(id: u64, base: Option<String>, cx: &mut App) {
        let Some((gh, Some(branch))) = Self::github_and_branch(id, cx) else {
            return;
        };
        let contributing_to_parent = Self::state(cx)
            .read(cx)
            .repository(id)
            .is_some_and(|r| r.is_fork_contributing_to_parent());
        // the base is a remote branch name in the dialog; GitHub wants it bare
        let base = base.map(|b| {
            b.split_once('/')
                .map(|(_, name)| name.to_string())
                .unwrap_or(b)
        });
        Self::open_url(
            &pull_request_url(&gh, &branch, base.as_deref(), contributing_to_parent),
            cx,
        );
    }

    /// Settings › Git › Hooks: (re)load the login-shell environment for git
    /// subprocesses, or drop it when the option is off.
    pub fn refresh_hook_env(cx: &mut App) {
        let (enabled, cache) = {
            let s = Self::state(cx).read(cx).settings.clone();
            (s.enable_git_hook_env, s.cache_git_hook_env)
        };
        if !enabled {
            corvane_git::hook_env::clear_hook_env();
            return;
        }
        spawn_bg(
            cx,
            corvane_git::hook_env::load_shell_env,
            move |result, _| match result {
                Ok(env) => {
                    info!(
                        vars = env.len(),
                        "loaded git hook environment from the shell"
                    );
                    corvane_git::hook_env::set_hook_env(env, cache);
                }
                Err(err) => warn!(%err, "could not load the shell environment for git hooks"),
            },
        );
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
        // Settings › Advanced (and Appearance › Syntax highlighting) list the
        // on-demand packs from the manifest (`502-optional-components`,
        // `105-tree-sitter-highlighting`)
        let wants_manifest = {
            let s = Self::state(cx).read(cx);
            s.packs.manifest.is_none()
                && crate::packs::offered_packs(&s.flags)
                    .iter()
                    .any(|kind| !s.packs.bundled(*kind))
        };
        if wants_manifest {
            Self::refresh_packs_manifest(cx);
        }
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
        Self::refresh_hook_env(cx);
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
                    local_email: corvane_git::local_config_value(
                        git.clone(),
                        &workdir,
                        "user.email",
                    ),
                    global,
                    autocrlf,
                    local_autocrlf: corvane_git::local_config_value(git, &workdir, "core.autocrlf"),
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
                if let Some(value) = save.autocrlf {
                    let result = match value {
                        Some(value) => corvane_git::set_local_config_value(
                            git.clone(),
                            &workdir,
                            "core.autocrlf",
                            &value,
                        ),
                        None => corvane_git::remove_local_config_value(
                            git.clone(),
                            &workdir,
                            "core.autocrlf",
                        ),
                    };
                    if let Err(err) = result {
                        errors.push(format!("Failed saving the line ending setting: {err}"));
                    }
                }
                errors
            },
            move |errors, cx| {
                if !errors.is_empty() {
                    let title = if cfg!(target_os = "macos") {
                        "Repository Settings"
                    } else {
                        "Repository settings"
                    };
                    Self::show_error(title, errors.join("\n"), cx);
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

    /// Repository › Remove…: confirm first unless the prompt is turned off
    /// or the repository is missing (GHD `App.removeRepository`).
    pub fn request_remove_repository(id: u64, cx: &mut App) {
        let s = Self::state(cx).read(cx);
        let missing = s.repository(id).is_some_and(|r| r.missing);
        let confirm = s.settings.confirm_repository_removal && !missing;
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

/// Where a file as of commit `short_sha` is written (flag `811`):
/// `<tmp>/corvane-history/<short sha>/<repository-relative path>`. `None` for
/// a path that would leave that directory.
fn historical_file_path(tmp: &Path, short_sha: &str, path: &str) -> Option<PathBuf> {
    use std::path::Component;
    let relative = Path::new(path);
    if path.is_empty()
        || !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(tmp.join("corvane-history").join(short_sha).join(relative))
}

/// Write `bytes` to `file` (replacing an earlier copy) and make it read-only.
fn write_read_only(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if let Ok(meta) = std::fs::metadata(file) {
        let mut perms = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(file, perms)?;
    }
    std::fs::write(file, bytes)?;
    let mut perms = std::fs::metadata(file)?.permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(file, perms)
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
            permissions: None,
            allow_forking: None,
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
    fn remote_web_urls() {
        for (remote, web) in [
            (
                "https://gitlab.com/group/sub/proj.git",
                Some("https://gitlab.com/group/sub/proj"),
            ),
            (
                "https://user:tok@git.corp:8443/team/repo/",
                Some("https://git.corp:8443/team/repo"),
            ),
            (
                "http://gitea.local/me/repo",
                Some("http://gitea.local/me/repo"),
            ),
            (
                "git@bitbucket.org:team/repo.git",
                Some("https://bitbucket.org/team/repo"),
            ),
            (
                "ssh://git@git.corp:2222/team/repo.git",
                Some("https://git.corp/team/repo"),
            ),
            (
                "git://example.org/repo.git",
                Some("https://example.org/repo"),
            ),
            ("/srv/git/repo.git", None),
            ("https://host.example/", None),
        ] {
            assert_eq!(remote_web_url(remote).as_deref(), web, "{remote}");
        }
    }

    #[test]
    fn issues_are_created_on_the_contribution_target() {
        let mut repo = corvane_models::Repository::new(1, std::path::PathBuf::from("/tmp/hello"));
        repo.github = Some(gh(false));
        let url = repo
            .non_fork_github()
            .map(|g| issue_creation_url(&g.html_url));
        assert_eq!(
            url.as_deref(),
            Some("https://github.com/octocat/hello/issues/new/choose")
        );
        // a fork files issues on its parent by default (GHD #9232)
        repo.github = Some(gh(true));
        let url = repo
            .non_fork_github()
            .map(|g| issue_creation_url(&g.html_url));
        assert_eq!(
            url.as_deref(),
            Some("https://github.com/octocat/hello/issues/new/choose")
        );
    }

    #[test]
    fn encodes_branch_names() {
        assert_eq!(encode_component("feature/x y"), "feature%2Fx%20y");
        assert_eq!(encode_component("main"), "main");
    }

    #[test]
    fn pull_request_urls() {
        assert_eq!(
            pull_request_url(&gh(false), "feat/one", None, false),
            "https://github.com/octocat/hello/pull/new/feat%2Fone"
        );
        assert_eq!(
            pull_request_url(&gh(false), "feat", Some("develop"), false),
            "https://github.com/octocat/hello/pull/new/develop...feat"
        );
        assert_eq!(
            pull_request_url(&gh(true), "feat", None, true),
            "https://github.com/me/hello/pull/new/me:hello:feat"
        );
        assert_eq!(
            pull_request_url(&gh(true), "feat", Some("main"), true),
            "https://github.com/me/hello/pull/new/octocat:hello:main...me:hello:feat"
        );
        assert_eq!(
            pull_request_url(&gh(true), "feat", None, false),
            "https://github.com/me/hello/pull/new/feat"
        );
    }
}

#[cfg(test)]
mod historical_file_tests {
    use super::*;

    #[test]
    fn stays_inside_the_temporary_directory() {
        let tmp = Path::new("/tmp");
        assert_eq!(
            historical_file_path(tmp, "abc", "src/a.png"),
            Some(PathBuf::from("/tmp/corvane-history/abc/src/a.png"))
        );
        assert_eq!(historical_file_path(tmp, "abc", "../x"), None);
        assert_eq!(historical_file_path(tmp, "abc", "/etc/x"), None);
        assert_eq!(historical_file_path(tmp, "abc", ""), None);
    }

    #[test]
    fn rewrites_a_read_only_copy() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a/b.txt");
        write_read_only(&file, b"one").unwrap();
        assert!(std::fs::metadata(&file).unwrap().permissions().readonly());
        write_read_only(&file, b"two").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"two");
        assert!(std::fs::metadata(&file).unwrap().permissions().readonly());
    }
}
