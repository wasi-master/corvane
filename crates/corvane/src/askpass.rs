//! `GIT_ASKPASS` mode: git runs this binary with the prompt as its only
//! argument (and `CORVANE_ASKPASS=1` in the environment) and reads the answer
//! from stdout. Usernames come from `CORVANE_ASKPASS_LOGINS`
//! (`host=login;host2=login2`), passwords from the macOS Keychain entry the
//! sign-in flow stored (`corvane_platform::keychain`). Anything unknown gets
//! an empty answer, which makes git fail with an authentication error that
//! the app turns into the "Authentication failed" dialog (GHD's
//! `askpass-trampoline` does the same over a local socket).

use std::collections::HashMap;

/// `Username for 'https://github.com': ` → `github.com`
/// `Password for 'https://octocat@github.com': ` → (`github.com`, `octocat`)
pub fn parse_prompt(prompt: &str) -> Option<(&'static str, String, Option<String>)> {
    let prompt = prompt.trim();
    let (kind, rest) = if let Some(rest) = prompt.strip_prefix("Username for '") {
        ("username", rest)
    } else {
        ("password", prompt.strip_prefix("Password for '")?)
    };
    let url = rest.split('\'').next()?;
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    let (user, host) = match without_scheme.split_once('@') {
        Some((user, host)) => (Some(user.to_string()), host),
        None => (None, without_scheme),
    };
    let host = host.split('/').next().unwrap_or(host).to_lowercase();
    Some((kind, host, user))
}

fn logins() -> HashMap<String, String> {
    std::env::var("CORVANE_ASKPASS_LOGINS")
        .unwrap_or_default()
        .split(';')
        .filter_map(|pair| {
            let (host, login) = pair.split_once('=')?;
            Some((host.to_lowercase(), login.to_string()))
        })
        .collect()
}

/// Answer one prompt, or `None` when Corvane knows nothing about the host.
pub fn answer(prompt: &str) -> Option<String> {
    let (kind, host, user) = parse_prompt(prompt)?;
    let logins = logins();
    match kind {
        "username" => logins.get(&host).cloned(),
        _ => {
            let login = user.or_else(|| logins.get(&host).cloned())?;
            corvane_platform::keychain::token(&host, &login)
                .ok()
                .flatten()
                .or_else(|| {
                    corvane_platform::keychain::generic_password(&host, &login)
                        .ok()
                        .flatten()
                })
        }
    }
}

/// Entry point for `CORVANE_ASKPASS=1 corvane "<prompt>"`.
pub fn run() -> ! {
    let prompt = std::env::args().nth(1).unwrap_or_default();
    match answer(&prompt) {
        Some(value) => {
            println!("{value}");
            std::process::exit(0);
        }
        None => {
            println!();
            std::process::exit(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_prompts() {
        assert_eq!(
            parse_prompt("Username for 'https://github.com': "),
            Some(("username", "github.com".into(), None))
        );
        assert_eq!(
            parse_prompt("Password for 'https://octocat@GitHub.com': "),
            Some(("password", "github.com".into(), Some("octocat".into())))
        );
        assert_eq!(parse_prompt("Enter passphrase for key '/x': "), None);
    }
}
