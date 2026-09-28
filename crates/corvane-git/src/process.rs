//! The single choke point for spawning `git` (never spawn it elsewhere).
//! Environment mirrors GitHub Desktop's `lib/git/core.ts`.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
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
}

impl GitCommand {
    pub fn new(bin: Arc<GitBinary>) -> Self {
        Self {
            bin,
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            ok_codes: vec![0],
        }
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
        cmd.args(&self.args);
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        // GHD: never let git prompt on a terminal; force stable English output.
        cmd.env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "en_US.UTF-8")
            .env("GIT_OPTIONAL_LOCKS", "0");
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::null());
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
        let output = self.command().output().map_err(GitError::Spawn)?;
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
    pub fn run_streaming(&self, mut on_stderr_line: impl FnMut(&str)) -> Result<GitOutput> {
        let started = Instant::now();
        let args = self.describe();
        let mut child = self
            .command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(GitError::Spawn)?;

        let stdout_pipe = child.stdout.take();
        let stdout_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut pipe) = stdout_pipe {
                let _ = std::io::Read::read_to_end(&mut pipe, &mut buf);
            }
            buf
        });

        let mut stderr_all = String::new();
        if let Some(stderr) = child.stderr.take() {
            let mut reader = BufReader::new(stderr);
            let mut chunk = Vec::new();
            loop {
                chunk.clear();
                // read until \r or \n
                let n = read_until_any(&mut reader, &[b'\r', b'\n'], &mut chunk);
                if n == 0 {
                    break;
                }
                let line = String::from_utf8_lossy(&chunk);
                let line = line.trim_end_matches(['\r', '\n']);
                if !line.is_empty() {
                    on_stderr_line(line);
                }
                stderr_all.push_str(line);
                stderr_all.push('\n');
            }
        }

        let status = child.wait().map_err(GitError::Spawn)?;
        let stdout = stdout_thread.join().unwrap_or_default();
        let code = status.code();
        debug!(git = %args, code, ms = started.elapsed().as_millis(), "git finished (streamed)");
        if code.is_some_and(|c| self.ok_codes.contains(&c)) {
            Ok(GitOutput {
                status,
                stdout,
                stderr: stderr_all,
            })
        } else {
            Err(GitError::Failed {
                args,
                code,
                stderr: stderr_all.trim().to_string(),
            })
        }
    }
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
