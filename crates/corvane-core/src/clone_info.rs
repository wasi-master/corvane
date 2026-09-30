//! Clone URL resolution - GHD `ui/clone-repository/clone-repository.tsx`
//! (`resolveCloneInfo`), `lib/find-account.ts` (`findAccountForRemoteURL`),
//! `lib/remote-parsing.ts` (`parseRepositoryIdentifier`) and
//! `lib/api.ts` (`fetchRepositoryCloneInfo`).
//!
//! `owner/name` shorthand and GitHub URLs are looked up on the API so the
//! clone uses the repository's canonical URL (SSH when the user typed an SSH
//! URL) and default branch, and a missing repository is reported before any
//! git process starts. Accounts are tried in GHD's order, with an anonymous
//! GitHub.com account last so public repositories resolve without sign-in.
//!
//! Deviation: when every account answers 404 for an `owner/name` shorthand,
//! GHD hands the bare alias to git (which fails with "repository does not
//! exist"); Corvane shows GHD's "We couldn't find that repository" error
//! instead. When a lookup fails otherwise (offline, rate limit) the shorthand
//! is cloned as `https://github.com/owner/name.git`.
//!
//! Deviation (`360-clone-local-sources`): a local folder (`/path`, `~/path`)
//! or `file://` URL is cloned as typed once [`resolve_local`] finds a
//! repository there, and "no Git repository at that path" is reported
//! before git runs (GHD turns `/a/b` into `https://github.com/a/b.git` and
//! rejects longer paths).
//!
//! Deviation (`355-clone-prefers-ssh`): the SSH URL can be preferred for
//! every lookup, not only for a typed SSH URL (GHD has no protocol setting).

use std::path::PathBuf;

use corvane_github::{Client, Endpoint, RepositoryCloneInfo};
use corvane_models::split_remote;
use gpui_kit::App;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;

/// The `DialogError` GHD shows when the repository can't be found.
pub const REPOSITORY_NOT_FOUND: &str = "We couldn't find that repository. Check that you are logged in, the network is accessible, and the URL or repository alias are spelled correctly.";

/// `360-clone-local-sources`: the source is not a repository on disk.
pub const LOCAL_SOURCE_NOT_FOUND: &str =
    "There's no Git repository at that path. Check the path and try again.";

/// `360-clone-local-sources`: the folder a local clone source names: an
/// absolute path, `~/…`, or a `file://` URL (`file:///abs`,
/// `file://localhost/abs`, `%20` for spaces).
pub fn local_source(input: &str) -> Option<PathBuf> {
    let input = input.trim();
    if let Some(rest) = input.strip_prefix("file://") {
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        return rest
            .starts_with('/')
            .then(|| PathBuf::from(rest.replace("%20", " ")));
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rest));
    }
    input.starts_with('/').then(|| PathBuf::from(input))
}

/// `360-clone-local-sources`: `None` when `input` is not a local source,
/// else what to clone (a `file://` URL as typed, a path expanded) or
/// [`LOCAL_SOURCE_NOT_FOUND`].
pub fn resolve_local(input: &str) -> Option<Result<CloneInfo, &'static str>> {
    let path = local_source(input)?;
    let found = matches!(
        corvane_git::path_status(&path),
        corvane_git::PathStatus::Repository | corvane_git::PathStatus::Bare
    );
    Some(if !found {
        Err(LOCAL_SOURCE_NOT_FOUND)
    } else {
        let input = input.trim();
        Ok(CloneInfo {
            url: if input.starts_with("file://") {
                input.to_string()
            } else {
                path.display().to_string()
            },
            default_branch: None,
        })
    })
}

/// GHD `IRepositoryIdentifier`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryIdentifier {
    pub owner: String,
    pub name: String,
    /// `None` for the `owner/name` shorthand.
    pub hostname: Option<String>,
}

/// GHD `parseRepositoryIdentifier`: a remote URL with an owner and a name,
/// else `owner/name`.
pub fn parse_repository_identifier(url: &str) -> Option<RepositoryIdentifier> {
    let url = url.trim();
    if let Some((host, path)) = split_remote(url) {
        let path = path.trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let mut parts = path.rsplit('/');
        let name = parts.next().unwrap_or_default();
        let owner = parts.next().unwrap_or_default();
        if !owner.is_empty() && !name.is_empty() {
            return Some(RepositoryIdentifier {
                owner: owner.to_string(),
                name: name.to_string(),
                hostname: Some(host),
            });
        }
    }
    let pieces: Vec<&str> = url.split('/').collect();
    match pieces.as_slice() {
        [owner, name] if !owner.is_empty() && !name.is_empty() => Some(RepositoryIdentifier {
            owner: owner.to_string(),
            name: name.to_string(),
            hostname: None,
        }),
        _ => None,
    }
}

/// An account `findAccountForRemoteURL` may pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Web host (`github.com`, `ghe.corp`).
    pub host: String,
    pub is_dotcom: bool,
    /// `false` for `Account.anonymous()`.
    pub authenticated: bool,
}

/// What to clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneInfo {
    pub url: String,
    pub default_branch: Option<String>,
}

/// `lookup(candidate index, owner, name, ssh)` answers
/// `fetchRepositoryCloneInfo` for one account: `Ok(None)` is a 404.
pub type Lookup<'a> =
    dyn FnMut(usize, &str, &str, bool) -> Result<Option<RepositoryCloneInfo>, String> + 'a;

/// GHD `resolveCloneInfo` + `findAccountForRemoteURL`. `Ok` with the typed
/// URL when there is nothing to look up (or the lookup failed for another
/// reason than "not found"); `Err(REPOSITORY_NOT_FOUND)` otherwise.
pub fn resolve(
    input: &str,
    candidates: &[Candidate],
    lookup: &mut Lookup<'_>,
) -> Result<CloneInfo, &'static str> {
    resolve_with(input, candidates, lookup, true, false)
}

/// `resolve`; with `strict_shorthand` off (the GHD value of
/// `204-clone-shorthand-not-found`) an `owner/name` every account answers
/// 404 for is handed to git as typed instead of failing here. `prefer_ssh`
/// (`355-clone-prefers-ssh`) asks the API for the SSH URL even when the
/// input is not an SSH URL.
pub fn resolve_with(
    input: &str,
    candidates: &[Candidate],
    lookup: &mut Lookup<'_>,
    strict_shorthand: bool,
    prefer_ssh: bool,
) -> Result<CloneInfo, &'static str> {
    let input = input.trim();
    let as_is = || CloneInfo {
        url: input.to_string(),
        default_branch: None,
    };
    if input.ends_with(".wiki.git") {
        return Ok(as_is());
    }
    let identifier = parse_repository_identifier(input);
    // "Respect the user's preference if they provided an SSH URL"
    let ssh = prefer_ssh || input.starts_with("git@") || input.starts_with("ssh://");

    // 1. an account for the URL's host
    if let Some((host, _)) = split_remote(input)
        && let Some(ix) = candidates
            .iter()
            .position(|c| c.host.eq_ignore_ascii_case(&host))
    {
        let Some(id) = identifier else {
            return Ok(as_is());
        };
        return match lookup(ix, &id.owner, &id.name, ssh) {
            Ok(Some(info)) => Ok(CloneInfo {
                url: info.url,
                default_branch: info.default_branch,
            }),
            Ok(None) => Err(REPOSITORY_NOT_FOUND),
            // `.catch(err => ({ url }))`
            Err(_) => Ok(as_is()),
        };
    }

    // 2. the first account that can see `owner/name`: signed-in GitHub.com,
    // then Enterprise, then anonymous GitHub.com
    let Some(id) = identifier else {
        return Ok(as_is());
    };
    let rank = |c: &Candidate| match (c.is_dotcom, c.authenticated) {
        (true, true) => 0,
        (false, _) => 1,
        (true, false) => 2,
    };
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by_key(|&ix| rank(&candidates[ix]));
    let mut lookup_failed = false;
    for ix in order {
        if let Some(host) = &id.hostname
            && !candidates[ix].host.eq_ignore_ascii_case(host)
        {
            continue;
        }
        // `canAccessRepository`; its answer doubles as the clone info GHD
        // fetches right after
        match lookup(ix, &id.owner, &id.name, ssh) {
            Ok(Some(info)) => {
                return Ok(CloneInfo {
                    url: info.url,
                    default_branch: info.default_branch,
                });
            }
            Ok(None) => {}
            Err(_) => lookup_failed = true,
        }
    }
    // every account answered 404: not found (see the module doc); a lookup
    // that failed otherwise (offline, rate limit) leaves it to git
    if id.hostname.is_none() && !lookup_failed && strict_shorthand {
        return Err(REPOSITORY_NOT_FOUND);
    }
    Ok(as_is())
}

impl Dispatcher {
    /// Resolve what the Clone dialog should clone (GHD `resolveCloneInfo`)
    /// on a background thread. `prefer_ssh` asks for the SSH clone URL
    /// (`355-clone-prefers-ssh`, decided by the dialog).
    pub fn resolve_clone_info(
        input: String,
        prefer_ssh: bool,
        then: impl FnOnce(Result<CloneInfo, &'static str>, &mut App) + 'static,
        cx: &mut App,
    ) {
        let mut clients: Vec<(Candidate, Client)> = Vec::new();
        for account in &Self::state(cx).read(cx).accounts {
            let Some(token) = corvane_platform::keychain::token(&account.host(), &account.login)
                .ok()
                .flatten()
            else {
                continue;
            };
            let endpoint = Endpoint::from_api_base(&account.endpoint);
            clients.push((
                Candidate {
                    host: endpoint.host().to_string(),
                    is_dotcom: endpoint.is_dotcom(),
                    authenticated: true,
                },
                Client::new(endpoint, token),
            ));
        }
        // `Account.anonymous()`
        let anonymous = Endpoint::github_com();
        clients.push((
            Candidate {
                host: anonymous.host().to_string(),
                is_dotcom: true,
                authenticated: false,
            },
            Client::new(anonymous, ""),
        ));
        let strict_shorthand = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::CLONE_SHORTHAND_NOT_FOUND);
        let local_sources = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::CLONE_LOCAL_SOURCES);
        spawn_bg(
            cx,
            move || {
                if local_sources && let Some(local) = resolve_local(&input) {
                    return local;
                }
                let candidates: Vec<Candidate> = clients.iter().map(|(c, _)| c.clone()).collect();
                let mut lookup = |ix: usize, owner: &str, name: &str, ssh: bool| {
                    clients[ix]
                        .1
                        .repository_clone_info(owner, name, ssh)
                        .map_err(|err| err.to_string())
                };
                // `owner/name` passed through as typed becomes a GitHub.com URL
                resolve_with(
                    &input,
                    &candidates,
                    &mut lookup,
                    strict_shorthand,
                    prefer_ssh,
                )
                .map(|mut info| {
                    info.url = corvane_git::normalize_clone_url(&info.url).unwrap_or(info.url);
                    info
                })
            },
            then,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dotcom(authenticated: bool) -> Candidate {
        Candidate {
            host: "github.com".into(),
            is_dotcom: true,
            authenticated,
        }
    }

    fn ghe() -> Candidate {
        Candidate {
            host: "ghe.corp".into(),
            is_dotcom: false,
            authenticated: true,
        }
    }

    fn info(url: &str) -> Option<RepositoryCloneInfo> {
        Some(RepositoryCloneInfo {
            url: url.into(),
            default_branch: Some("trunk".into()),
        })
    }

    #[test]
    fn parses_identifiers() {
        let id = parse_repository_identifier("https://github.com/desktop/desktop.git").unwrap();
        assert_eq!(
            (id.owner.as_str(), id.name.as_str()),
            ("desktop", "desktop")
        );
        assert_eq!(id.hostname.as_deref(), Some("github.com"));
        let id = parse_repository_identifier("git@ghe.corp:team/app.git").unwrap();
        assert_eq!((id.owner.as_str(), id.name.as_str()), ("team", "app"));
        let id = parse_repository_identifier("hubot/cool-repo").unwrap();
        assert_eq!(id.hostname, None);
        assert_eq!(parse_repository_identifier("nope"), None);
        assert_eq!(parse_repository_identifier("a/b/c"), None);
    }

    #[test]
    fn shorthand_resolves_through_the_first_account_that_sees_it() {
        let candidates = [dotcom(false), ghe(), dotcom(true)];
        let mut calls = Vec::new();
        let mut lookup = |ix: usize, _: &str, _: &str, _: bool| {
            calls.push(ix);
            // only the anonymous account can see it (public repo)
            Ok(if ix == 0 {
                info("https://github.com/o/n.git")
            } else {
                None
            })
        };
        let got = resolve("o/n", &candidates, &mut lookup).unwrap();
        assert_eq!(got.url, "https://github.com/o/n.git");
        assert_eq!(got.default_branch.as_deref(), Some("trunk"));
        // signed-in GitHub.com first, Enterprise next, anonymous last
        assert_eq!(calls, vec![2, 1, 0]);
    }

    #[test]
    fn missing_shorthand_is_not_found() {
        let mut lookup = |_: usize, _: &str, _: &str, _: bool| Ok(None);
        assert_eq!(
            resolve("o/missing", &[dotcom(false)], &mut lookup),
            Err(REPOSITORY_NOT_FOUND)
        );
    }

    #[test]
    fn url_uses_the_host_account_and_keeps_ssh() {
        let candidates = [dotcom(false), ghe()];
        let mut seen = None;
        let mut lookup = |ix: usize, owner: &str, name: &str, ssh: bool| {
            seen = Some((ix, owner.to_string(), name.to_string(), ssh));
            Ok(info("git@ghe.corp:team/app.git"))
        };
        let got = resolve("git@ghe.corp:team/app.git", &candidates, &mut lookup).unwrap();
        assert_eq!(got.url, "git@ghe.corp:team/app.git");
        assert_eq!(seen, Some((1, "team".into(), "app".into(), true)));

        let mut not_found = |_: usize, _: &str, _: &str, _: bool| Ok(None);
        assert_eq!(
            resolve("https://github.com/o/private", &candidates, &mut not_found),
            Err(REPOSITORY_NOT_FOUND)
        );
    }

    #[test]
    fn local_sources_are_checked_and_cloned_as_typed() {
        assert_eq!(local_source("hubot/cool"), None);
        assert_eq!(local_source("https://github.com/a/b"), None);
        assert_eq!(local_source(" /a/b/c "), Some(PathBuf::from("/a/b/c")));
        assert_eq!(
            local_source("file://localhost/a/my%20repo"),
            Some(PathBuf::from("/a/my repo"))
        );
        assert!(resolve_local("owner/name").is_none());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string();
        assert_eq!(resolve_local(&path), Some(Err(LOCAL_SOURCE_NOT_FOUND)));
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert_eq!(resolve_local(&path).unwrap().unwrap().url, path);
        let url = format!("file://{path}");
        assert_eq!(resolve_local(&url).unwrap().unwrap().url, url);
    }

    #[test]
    fn prefer_ssh_asks_for_the_ssh_url() {
        let mut seen = None;
        let mut lookup = |_: usize, _: &str, _: &str, ssh: bool| {
            seen = Some(ssh);
            Ok(info("git@github.com:o/n.git"))
        };
        let got = resolve_with("o/n", &[dotcom(true)], &mut lookup, true, true).unwrap();
        assert_eq!(got.url, "git@github.com:o/n.git");
        assert_eq!(seen, Some(true));
    }

    #[test]
    fn lookup_errors_and_unknown_hosts_clone_as_typed() {
        let mut failing = |_: usize, _: &str, _: &str, _: bool| Err("rate limited".to_string());
        let got = resolve("https://github.com/o/n", &[dotcom(false)], &mut failing).unwrap();
        assert_eq!(got.url, "https://github.com/o/n");
        let mut never = |_: usize, _: &str, _: &str, _: bool| -> Result<_, String> {
            panic!("no lookup for other hosts")
        };
        let got = resolve("https://gitlab.com/o/n.git", &[dotcom(false)], &mut never).unwrap();
        assert_eq!(got.url, "https://gitlab.com/o/n.git");
        // anonymous API rate-limited: the shorthand is cloned as typed
        let mut limited = |_: usize, _: &str, _: &str, _: bool| Err("403".to_string());
        let got = resolve("o/n", &[dotcom(false)], &mut limited).unwrap();
        assert_eq!(got.url, "o/n");
        let got = resolve(
            "https://github.com/o/n.wiki.git",
            &[dotcom(false)],
            &mut never,
        )
        .unwrap();
        assert_eq!(got.default_branch, None);
    }
}
