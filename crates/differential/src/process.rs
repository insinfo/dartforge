//! Running external processes, capturing stdout/stderr, with a time limit.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Output of a process (or of a whole oracle): stdout, stderr and exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl Output {
    /// Synthetic output for a failure of the harness itself (missing tool, timeout…).
    pub fn error(message: impl Into<String>) -> Output {
        Output { stdout: String::new(), stderr: message.into(), code: -1 }
    }

    /// First non-empty line of stderr — the report's grouping key.
    pub fn first_stderr_line(&self) -> &str {
        self.stderr.lines().map(str::trim_end).find(|l| !l.trim().is_empty()).unwrap_or("")
    }
}

/// Code returned when the process exceeds the time limit.
pub const TIMEOUT_CODE: i32 = -2;

/// Cap on what the harness keeps of the stdout/stderr of ONE process.
///
/// A program looping on output fills the harness's memory, not only its own:
/// `read_to_end` grows without bound. What the report cares about is the first
/// divergence and the first stderr line, and both fit here with room to spare.
/// The rest is still read and discarded, otherwise the pipe fills up and the
/// child hangs before the time limit can kill it.
const CAPTURE_CAP: usize = 4 * 1024 * 1024;

/// Reads everything to the end, keeping at most [`CAPTURE_CAP`] bytes.
fn read_capped(mut r: impl Read) -> Vec<u8> {
    let mut kept: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    let mut discarded: usize = 0;
    loop {
        match r.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if kept.len() < CAPTURE_CAP {
                    let fits = (CAPTURE_CAP - kept.len()).min(n);
                    kept.extend_from_slice(&chunk[..fits]);
                    discarded += n - fits;
                } else {
                    discarded += n;
                }
            }
        }
    }
    if discarded > 0 {
        // The marker is part of the compared (and cached) output: kept
        // verbatim so cached oracle outputs still match.
        kept.extend_from_slice(
            format!("\n[saída truncada pelo harness: mais {discarded} bytes]\n").as_bytes(),
        );
    }
    kept
}

/// Runs `program args…` in `cwd` and returns its output. Kills the process after `limit`.
pub fn run(program: &str, args: &[String], cwd: &Path, limit: Duration) -> Output {
    run_with_path(program, args, cwd, limit, &[])
}

/// Like `run`, prepending `extra_path` to the child's `PATH` (LLVM DLLs for `dartforge`).
pub fn run_with_path(program: &str, args: &[String], cwd: &Path, limit: Duration, extra_path: &[std::path::PathBuf]) -> Output {
    run_with_env(program, args, cwd, limit, extra_path, &[])
}

/// Like `run_with_path`, with extra environment variables for the child only
/// (the native mode's `--gc-stress`). `set_var` in the harness itself would be
/// `unsafe` and would apply to the other threads too.
pub fn run_with_env(
    program: &str,
    args: &[String],
    cwd: &Path,
    limit: Duration,
    extra_path: &[std::path::PathBuf],
    env: &[(&str, &str)],
) -> Output {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    if !extra_path.is_empty() {
        let current = std::env::var_os("PATH").unwrap_or_default();
        let mut new: Vec<std::path::PathBuf> = extra_path.to_vec();
        new.extend(std::env::split_paths(&current));
        if let Ok(j) = std::env::join_paths(new) {
            cmd.env("PATH", j);
        }
    }
    let mut child = match cmd.spawn() {
        Ok(f) => f,
        Err(e) => return Output::error(format!("could not run `{program}`: {e}")),
    };
    let mut out = child.stdout.take().expect("stdout piped");
    let mut err = child.stderr.take().expect("stderr piped");
    let out_reader = std::thread::spawn(move || read_capped(&mut out));
    let err_reader = std::thread::spawn(move || read_capped(&mut err));
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if start.elapsed() > limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break None,
        }
    };
    let stdout = String::from_utf8_lossy(&out_reader.join().unwrap_or_default()).into_owned();
    let mut stderr = String::from_utf8_lossy(&err_reader.join().unwrap_or_default()).into_owned();
    let code = match status {
        Some(s) => s.code().unwrap_or(-3),
        None if timed_out => {
            stderr = format!("timed out ({} s): {program} {}\n{stderr}", limit.as_secs(), args.join(" "));
            TIMEOUT_CODE
        }
        None => -3,
    };
    Output { stdout, stderr, code }
}
