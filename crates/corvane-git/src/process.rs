//! The single choke point for spawning `git` (never spawn it elsewhere).
//! Environment mirrors GitHub Desktop's `lib/git/core.ts`.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tracing::{debug, warn};

use crate::detect::GitBinary;
use crate::error::{GitError, Result};

#[derive(Debug)]
pub struct GitOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl GitOutput {
    pub fn stdout_string(&self) -> Result<String> {
        Ok(String::from_utf8(self.stdout.clone())?)
    }
}

/// Builder for one git invocation.
#[derive(Clone, Debug)]
pub struct GitCommand {
    bin: Arc<GitBinary>,
    args: Vec<OsString>,
    cwd: Option<PathBuf>,
    env: Vec<(OsString, OsString)>,
    /// Exit codes that are not failures (e.g. `diff --exit-code` → 1).
    ok_codes: Vec<i32>,
    /// Bytes written to git's stdin (`commit -F -`, `update-index --stdin`).
    stdin: Option<Vec<u8>>,
    /// Variables removed from the inherited environment (`GIT_SEQUENCE_EDITOR`).
    env_removed: Vec<OsString>,
}

/// Process-wide toggle for `-c credential.helper=manager` (set per network
/// operation by the dispatcher for non-GitHub remotes).
static CREDENTIAL_HELPER: AtomicBool = AtomicBool::new(false);

pub fn set_credential_helper(enabled: bool) {
    CREDENTIAL_HELPER.store(enabled, Ordering::Relaxed);
}

impl GitCommand {
    pub fn new(bin: Arc<GitBinary>) -> Self {
        Self {
            bin,
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            ok_codes: vec![0],
            stdin: None,
            env_removed: Vec::new(),
        }
    }

    /// Drop an inherited environment variable for this invocation.
    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Self {
        self.env_removed.push(key.as_ref().to_os_string());
        self
    }

    pub fn stdin(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(bytes.into());
        self
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
        self
    }

    pub fn current_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.cwd = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    pub fn allow_exit_code(mut self, code: i32) -> Self {
        self.ok_codes.push(code);
        self
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.bin.path);
        // Settings › Advanced › Use Git Credential Manager: `-c credential.helper=manager`
        // for the network commands (GHD `useExternalCredentialHelper`).
        if CREDENTIAL_HELPER.load(Ordering::Relaxed)
            && self.args.first().is_some_and(|a| {
                matches!(
                    a.to_str(),
                    Some("fetch" | "pull" | "push" | "clone" | "ls-remote")
                )
            })
        {
            cmd.args(["-c", "credential.helper=manager"]);
        }
        cmd.args(&self.args);
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        // Settings › Git › Hooks: the user's login-shell environment first,
        // so hooks see the same PATH as a terminal.
        if let Some(env) = crate::hook_env::hook_env() {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        // GHD: never let git prompt on a terminal; force stable English output.
        cmd.env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "en_US.UTF-8")
            .env("GIT_OPTIONAL_LOCKS", "0");
        for k in &self.env_removed {
            cmd.env_remove(k);
        }
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd.stdin(if self.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        cmd
    }

    fn describe(&self) -> String {
        self.args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Run to completion, capturing stdout/stderr. Blocking: call from a
    /// background thread (GPUI `background_spawn`).
    pub fn run(&self) -> Result<GitOutput> {
        let started = Instant::now();
        let args = self.describe();
        let output = match &self.stdin {
            None => self.command().output().map_err(GitError::Spawn)?,
            Some(bytes) => {
                let mut child = self
                    .command()
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(GitError::Spawn)?;
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    // git may exit early; a broken pipe is then reported via the exit code
                    let _ = stdin.write_all(bytes);
                }
                child.wait_with_output().map_err(GitError::Spawn)?
            }
        };
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let code = output.status.code();
        debug!(
            git = %args,
            cwd = ?self.cwd,
            code,
            ms = started.elapsed().as_millis(),
            "git finished"
        );
        if code.is_some_and(|c| self.ok_codes.contains(&c)) {
            Ok(GitOutput {
                status: output.status,
                stdout: output.stdout,
                stderr,
            })
        } else {
            warn!(git = %args, code, stderr = %stderr.trim(), "git failed");
            Err(GitError::Failed {
                args,
                code,
                stderr: stderr.trim().to_string(),
            })
        }
    }

    /// Run with `--progress`-style stderr streamed line by line (clone, fetch, push).
    /// Lines are split on `\n` and `\r` so percentage updates arrive as they happen.
    pub fn run_streaming(&self, on_stderr_line: impl FnMut(&str)) -> Result<GitOutput> {
        self.run_streaming_on(StreamedPipe::Stderr, on_stderr_line)
    }

    /// Like `run_streaming` but streams stdout (cherry-pick prints one
    /// `[branch sha] summary` line per applied commit there).
    pub fn run_streaming_stdout(&self, on_stdout_line: impl FnMut(&str)) -> Result<GitOutput> {
        self.run_streaming_on(StreamedPipe::Stdout, on_stdout_line)
    }

    fn run_streaming_on(
        &self,
        pipe: StreamedPipe,
        mut on_line: impl FnMut(&str),
    ) -> Result<GitOutput> {
        let started = Instant::now();
        let args = self.describe();
        let mut child = self
            .command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(GitError::Spawn)?;
        if let (Some(bytes), Some(mut stdin)) = (&self.stdin, child.stdin.take()) {
            use std::io::Write;
            let _ = stdin.write_all(bytes);
        }

        // the pipe that is not streamed is drained on a thread so git never blocks
        let (streamed, drained): (Box<dyn std::io::Read + Send>, Box<dyn std::io::Read + Send>) =
            match pipe {
                StreamedPipe::Stderr => (
                    Box::new(child.stderr.take().expect("piped")),
                    Box::new(child.stdout.take().expect("piped")),
                ),
                StreamedPipe::Stdout => (
                    Box::new(child.stdout.take().expect("piped")),
                    Box::new(child.stderr.take().expect("piped")),
                ),
            };
        let drain_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut drained = drained;
            let _ = drained.read_to_end(&mut buf);
            buf
        });

        let mut streamed_all = String::new();
        let mut reader = BufReader::new(streamed);
        let mut chunk = Vec::new();
        loop {
            chunk.clear();
            // read until \r or \n
            let n = read_until_any(&mut reader, b"\r\n", &mut chunk);
            if n == 0 {
                break;
            }
            let line = String::from_utf8_lossy(&chunk);
            let line = line.trim_end_matches(['\r', '\n']);
            if !line.is_empty() {
                on_line(line);
            }
            streamed_all.push_str(line);
            streamed_all.push('\n');
        }

        let status = child.wait().map_err(GitError::Spawn)?;
        let drained = drain_thread.join().unwrap_or_default();
        let (stdout, stderr) = match pipe {
            StreamedPipe::Stderr => (drained, streamed_all),
            StreamedPipe::Stdout => (
                streamed_all.into_bytes(),
                String::from_utf8_lossy(&drained).into_owned(),
            ),
        };
        let code = status.code();
        debug!(git = %args, code, ms = started.elapsed().as_millis(), "git finished (streamed)");
        if code.is_some_and(|c| self.ok_codes.contains(&c)) {
            Ok(GitOutput {
                status,
                stdout,
                stderr,
            })
        } else {
            warn!(git = %args, code, stderr = %stderr.trim(), "git failed");
            Err(GitError::Failed {
                args,
                code,
                stderr: stderr.trim().to_string(),
            })
        }
    }
}

#[derive(Clone, Copy)]
enum StreamedPipe {
    Stdout,
    Stderr,
}

fn read_until_any(reader: &mut impl BufRead, delims: &[u8], out: &mut Vec<u8>) -> usize {
    let mut total = 0;
    loop {
        let available = match reader.fill_buf() {
            Ok(buf) => buf,
            Err(_) => return total,
        };
        if available.is_empty() {
            return total;
        }
        match available.iter().position(|b| delims.contains(b)) {
            Some(i) => {
                out.extend_from_slice(&available[..=i]);
                reader.consume(i + 1);
                return total + i + 1;
            }
            None => {
                let n = available.len();
                out.extend_from_slice(available);
                reader.consume(n);
                total += n;
            }
        }
    }
}
