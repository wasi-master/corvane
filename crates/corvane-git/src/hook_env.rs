//! Git hook environment (Settings › Git › Hooks): GHD
//! `lib/shell.ts#updateEnvironmentForProcess` reads the user's login shell
//! environment so hooks find the same tools (nvm, rbenv, asdf…) as a
//! terminal would. Corvane applies it to every git subprocess.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Variables never copied from the shell (`ExcludedEnvironmentVars` + shell
/// bookkeeping that would confuse child processes).
const EXCLUDED: &[&str] = &[
    "LOCAL_GIT_DIRECTORY",
    "PWD",
    "OLDPWD",
    "SHLVL",
    "_",
    "TERM_SESSION_ID",
];

#[derive(Default)]
struct State {
    /// `None` = feature off.
    env: Option<HashMap<String, String>>,
    cache: bool,
}

static STATE: RwLock<State> = RwLock::new(State {
    env: None,
    cache: true,
});

/// Variables git subprocesses inherit on top of Corvane's own environment.
pub fn hook_env() -> Option<HashMap<String, String>> {
    STATE.read().ok().and_then(|s| s.env.clone())
}

/// Turn the feature off (`enableGitHookEnv = false`).
pub fn clear_hook_env() {
    if let Ok(mut s) = STATE.write() {
        s.env = None;
    }
}

/// Store a freshly loaded environment; `cache` mirrors `cacheGitHookEnv`.
pub fn set_hook_env(env: HashMap<String, String>, cache: bool) {
    if let Ok(mut s) = STATE.write() {
        s.env = Some(env);
        s.cache = cache;
    }
}

/// `cacheGitHookEnv` off: refresh before an operation that may run hooks.
/// Blocking (spawns a login shell); call from a background thread.
pub fn reload_if_uncached() {
    let (enabled, cache) = STATE
        .read()
        .map(|s| (s.env.is_some(), s.cache))
        .unwrap_or((false, true));
    if enabled
        && !cache
        && let Ok(env) = load_shell_env()
    {
        set_hook_env(env, false);
    }
}

/// Run `$SHELL -ilc` and collect its environment, NUL-delimited so values
/// with newlines survive (the same awk one-liner GHD uses). 5 s timeout.
pub fn load_shell_env() -> std::io::Result<HashMap<String, String>> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
    let cmd = r#"command awk 'BEGIN{for(k in ENVIRON) printf("%c%s=%s%c", 0, k, ENVIRON[k], 0)}'"#;
    let mut child = Command::new(&shell)
        .args(["-ilc", cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "shell did not print its environment within 5 s",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let bytes = reader.join().unwrap_or_default();
    Ok(parse_env(&String::from_utf8_lossy(&bytes)))
}

/// `\0KEY=VALUE\0` pairs → map, minus the excluded names.
pub fn parse_env(output: &str) -> HashMap<String, String> {
    output
        .split('\0')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| pair.split_once('='))
        .filter(|(k, _)| !EXCLUDED.contains(k))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nul_delimited_pairs() {
        let env = parse_env("\0PATH=/a:/b\0\0MULTI=line1\nline2\0\0PWD=/x\0");
        assert_eq!(env.get("PATH").map(String::as_str), Some("/a:/b"));
        assert_eq!(env.get("MULTI").map(String::as_str), Some("line1\nline2"));
        assert!(!env.contains_key("PWD"));
    }

    #[test]
    fn loads_the_login_shell_environment() {
        let env = load_shell_env().expect("shell env");
        assert!(env.contains_key("PATH"));
    }
}
