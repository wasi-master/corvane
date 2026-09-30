//! Push failures GitHub explains in its `remote:` lines - GHD
//! `ui/dispatcher/error-handlers.ts`: secret-scanning push protection
//! (`extractSecretScanningResults`), workflow files refused for a token
//! without the `workflow` scope (`refusedWorkflowUpdate`) and SAML SSO
//! re-authorization (`samlReauthRequired`).
//!
//! Corvane addition (`255-plain-language-remote-errors`): a pull from an
//! upstream branch that was deleted, and a clone into a folder the user may
//! not write to, get a plain-language sentence before git's own message
//! ([`plain_remote_error`], [`plain_clone_error`]); GHD shows git's text
//! (desktop#1325, desktop#13187).

use corvane_models::{BypassReason, SecretLocation, SecretScanResult};
use gpui_kit::App;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::Popup;

impl Dispatcher {
    /// `BypassPushProtectionDialog` submit: create the bypass through the
    /// API, then show the push-protection dialog again with the secret
    /// marked as bypassed (`openBypassPushProtection`).
    pub fn bypass_push_protection(
        id: u64,
        secret: SecretScanResult,
        reason: BypassReason,
        secrets: Vec<SecretScanResult>,
        mut bypassed: Vec<String>,
        cx: &mut App,
    ) {
        let Some(github) = Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.github.clone())
        else {
            Self::close_popup(cx);
            return;
        };
        let Some((endpoint, token, _)) = Self::api_for(&github, cx) else {
            Self::show_error(
                "Could not bypass push protection",
                "You are not signed in to GitHub.",
                cx,
            );
            return;
        };
        let (owner, name, placeholder, bypass_url) = (
            github.owner.clone(),
            github.name.clone(),
            secret.id.clone(),
            secret.bypass_url.clone(),
        );
        spawn_bg(
            cx,
            move || {
                corvane_github::Client::new(endpoint, token)
                    .create_push_protection_bypass(&owner, &name, reason, &placeholder)
                    .map(|_| ())
                    .map_err(|err| err.to_string())
            },
            move |result, cx| match result {
                Ok(()) => {
                    bypassed.push(secret.id.clone());
                    Self::show_popup(
                        Popup::PushProtectionError {
                            repo: id,
                            secrets,
                            bypassed,
                        },
                        cx,
                    );
                }
                Err(err) => Self::show_error(
                    "Could not bypass push protection",
                    format!(
                        "Unable to create push protection bypass.\n\n{err}\n\nTry again at: {bypass_url}"
                    ),
                    cx,
                ),
            },
        );
    }
}

/// The upstream branch a pull or fetch could not find on the remote: git's
/// "Your configuration specifies to merge with the ref 'refs/heads/<b>' …
/// but no such ref was fetched" or "couldn't find remote ref <b>".
pub fn missing_remote_branch(stderr: &str) -> Option<String> {
    let name = if let Some(rest) = stderr.split_once("to merge with the ref '") {
        rest.1.split('\'').next()?
    } else {
        stderr
            .split_once("couldn't find remote ref ")?
            .1
            .lines()
            .next()?
            .trim()
    };
    let name = name.strip_prefix("refs/heads/").unwrap_or(name);
    (!name.is_empty()).then(|| name.to_string())
}

/// A plain-language explanation for a failed pull or fetch, followed by
/// git's message; `None` when there is nothing better to say.
pub fn plain_remote_error(err: &corvane_git::GitError) -> Option<String> {
    let corvane_git::GitError::Failed { stderr, .. } = err else {
        return None;
    };
    let branch = missing_remote_branch(stderr)?;
    Some(format!(
        "The branch \"{branch}\" no longer exists on the remote, so there is nothing to pull. \
         It may have been deleted after a merge. Push to publish it again, or switch to \
         another branch.\n\n{err}"
    ))
}

/// A plain-language explanation for a clone into `path` that failed because
/// the folder (or its parent) is not writable, followed by the error.
pub fn plain_clone_error(err: &corvane_git::GitError, path: &std::path::Path) -> Option<String> {
    let denied = match err {
        corvane_git::GitError::Spawn(io) | corvane_git::GitError::Io(io) => {
            io.kind() == std::io::ErrorKind::PermissionDenied
        }
        corvane_git::GitError::Failed { stderr, .. } => {
            (stderr.contains("could not create work tree dir")
                || stderr.contains("could not create leading directories")
                || stderr.contains("could not create directory"))
                && stderr.contains("Permission denied")
        }
        _ => false,
    };
    denied.then(|| {
        let folder = path.parent().unwrap_or(path);
        format!(
            "You do not have permission to create a folder in \"{}\". Choose a location you \
             can write to, such as a folder in your home directory.\n\n{err}",
            folder.display()
        )
    })
}

/// `getRemoteMessage`: the `remote: ` lines of a push's stderr, unprefixed.
pub fn remote_message(stderr: &str) -> String {
    stderr
        .lines()
        .filter_map(|line| line.strip_prefix("remote: "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `rejectedPathRe`: the workflow file a push was rejected for.
pub fn rejected_workflow_path(stderr: &str) -> Option<String> {
    const MARKER: &str = "refusing to allow an OAuth App to create or update workflow `";
    stderr
        .lines()
        .filter(|line| line.trim_start().starts_with("! [remote rejected]"))
        .find_map(|line| {
            let rest = line.split_once(MARKER)?.1;
            let path = rest.split('`').next()?;
            (!path.is_empty()).then(|| path.to_string())
        })
}

/// `samlReauthErrorMessageRe`: the organization that enforces SAML SSO.
pub fn saml_organization(remote_message: &str) -> Option<String> {
    let needle = "' organization has enabled or enforced SAML SSO";
    if !remote_message.contains("you must re-authorize") {
        return None;
    }
    let end = remote_message.find(needle)?;
    let start = remote_message[..end].rfind('`')?;
    let name = &remote_message[start + 1..end];
    (!name.is_empty()).then(|| name.to_string())
}

/// `extractSecretScanningResults`: every "-- <description> --" block with
/// its `- commit: <sha>` / `path: <file>:<line>` locations and the bypass URL.
pub fn secret_scan_results(remote_message: &str) -> Vec<SecretScanResult> {
    let mut results = Vec::new();
    let mut rest = remote_message;
    while let Some(start) = rest.find("—— ") {
        let block_start = start + "—— ".len();
        let after = &rest[block_start..];
        let Some(desc_end) = after.find(" —") else {
            break;
        };
        let description = after[..desc_end].trim().to_string();
        let body = &after[desc_end..];
        // the block ends where the next one begins
        let block_end = body.find("—— ").unwrap_or(body.len());
        let block = &body[..block_end];
        rest = &body[block_end..];
        let Some(locations_start) = block.find("locations:") else {
            continue;
        };
        let mut locations = Vec::new();
        let mut lines = block[locations_start + "locations:".len()..]
            .lines()
            .peekable();
        let mut bypass_url = None;
        let mut requires_approval = false;
        while let Some(line) = lines.next() {
            let line = line.trim();
            if let Some(sha) = line.strip_prefix("- commit: ") {
                let sha = sha.trim().to_string();
                let Some(path_line) = lines.next() else {
                    break;
                };
                let Some(path) = path_line.trim().strip_prefix("path: ") else {
                    continue;
                };
                let (path, line_number) = path
                    .rsplit_once(':')
                    .and_then(|(p, n)| n.trim().parse::<u64>().ok().map(|n| (p.to_string(), n)))
                    .unwrap_or_else(|| (path.to_string(), 0));
                if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
                    locations.push(SecretLocation {
                        commit_sha: sha,
                        path,
                        line_number,
                    });
                }
            } else if let Some(pos) = line.find("https://") {
                let url = line[pos..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                if bypass_url.is_none() && !url.is_empty() {
                    bypass_url = Some(url);
                }
            }
            if line.contains("request an exemption") {
                requires_approval = true;
            }
        }
        let Some(bypass_url) = bypass_url else {
            continue;
        };
        let id = bypass_url.rsplit('/').next().unwrap_or("").to_string();
        results.push(SecretScanResult {
            id,
            description,
            locations,
            bypass_url,
            requires_approval,
        });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUSH_PROTECTION: &str = "remote: error: GH013: Repository rule violations found for refs/heads/main.
remote: 
remote: - GITHUB PUSH PROTECTION
remote:   —————————————————————————————————————————
remote:     Resolve the following violations before pushing again
remote: 
remote:     - Push cannot contain secrets
remote: 
remote:      (?) Learn how to resolve a blocked push
remote:      https://docs.github.com/code-security/secret-scanning/pushing-a-branch-blocked-by-push-protection
remote: 
remote: 
remote:       —— GitHub Personal Access Token ——————————————————————
remote:        locations:
remote:          - commit: 0123456789abcdef0123456789abcdef01234567
remote:            path: config/app.yml:12
remote:          - commit: 0123456789abcdef0123456789abcdef01234567
remote:            path: src/main.rs:3
remote: 
remote:        (?) To push, remove secret from commit(s) or follow this URL to allow the secret.
remote:        https://github.com/octocat/hello/security/secret-scanning/unblock-secret/2abc
remote: 
remote:       —— Slack Token ————————————————————————————————————
remote:        locations:
remote:          - commit: 89abcdef0123456789abcdef0123456789abcdef
remote:            path: .env:1
remote: 
remote:        (?) To push, request an exemption by following this URL.
remote:        https://github.com/octocat/hello/security/secret-scanning/unblock-secret/3def
remote: 
To https://github.com/octocat/hello.git
 ! [remote rejected] main -> main (push declined due to repository rule violations)
error: failed to push some refs to 'https://github.com/octocat/hello.git'
";

    #[test]
    fn parses_secret_scanning_results() {
        let secrets = secret_scan_results(&remote_message(PUSH_PROTECTION));
        assert_eq!(secrets.len(), 2);
        assert_eq!(secrets[0].description, "GitHub Personal Access Token");
        assert_eq!(secrets[0].id, "2abc");
        assert_eq!(secrets[0].locations.len(), 2);
        assert_eq!(secrets[0].locations[1].path, "src/main.rs");
        assert_eq!(secrets[0].locations[1].line_number, 3);
        assert!(!secrets[0].requires_approval);
        assert_eq!(secrets[1].description, "Slack Token");
        assert!(secrets[1].requires_approval);
        assert_eq!(
            secrets[1].bypass_url,
            "https://github.com/octocat/hello/security/secret-scanning/unblock-secret/3def"
        );
    }

    #[test]
    fn explains_missing_upstream_and_clone_permission() {
        let stderr = "Your configuration specifies to merge with the ref 'refs/heads/patch-1'\nfrom the remote, but no such ref was fetched.\n";
        assert_eq!(missing_remote_branch(stderr).as_deref(), Some("patch-1"));
        assert_eq!(
            missing_remote_branch("fatal: couldn't find remote ref feature/x\n").as_deref(),
            Some("feature/x")
        );
        assert!(missing_remote_branch("fatal: unable to access").is_none());
        let err = corvane_git::GitError::Failed {
            args: "pull".into(),
            code: Some(1),
            stderr: stderr.into(),
        };
        let text = plain_remote_error(&err).unwrap();
        assert!(
            text.starts_with("The branch \"patch-1\" no longer exists"),
            "{text}"
        );
        assert!(text.contains("no such ref was fetched"));

        let path = std::path::Path::new("/Applications/repo");
        let denied = corvane_git::GitError::Spawn(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        ));
        assert!(
            plain_clone_error(&denied, path)
                .unwrap()
                .contains("\"/Applications\"")
        );
        let git_denied = corvane_git::GitError::Failed {
            args: "clone".into(),
            code: Some(128),
            stderr: "fatal: could not create work tree dir 'repo': Permission denied\n".into(),
        };
        assert!(plain_clone_error(&git_denied, path).is_some());
        let other = corvane_git::GitError::Failed {
            args: "clone".into(),
            code: Some(128),
            stderr: "fatal: repository not found\n".into(),
        };
        assert!(plain_clone_error(&other, path).is_none());
    }

    #[test]
    fn parses_workflow_and_saml_rejections() {
        let stderr = "To github.com:o/r.git\n ! [remote rejected] feat -> feat (refusing to allow an OAuth App to create or update workflow `.github/workflows/ci.yml` without `workflow` scope)\nerror: failed to push some refs\n";
        assert_eq!(
            rejected_workflow_path(stderr).as_deref(),
            Some(".github/workflows/ci.yml")
        );
        let saml = "remote: The `acme' organization has enabled or enforced SAML SSO. To access\nremote: this repository, you must re-authorize the OAuth Application `GitHub Desktop`.\n";
        assert_eq!(
            saml_organization(&remote_message(saml)).as_deref(),
            Some("acme")
        );
        assert!(saml_organization("nothing").is_none());
    }
}
