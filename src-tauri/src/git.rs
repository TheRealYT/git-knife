//! The one and only place that spawns the `git` CLI. Everything else in the
//! backend goes through `run`, which sets a clean environment, optionally feeds
//! stdin, captures output, and maps a non-zero exit to a typed `Err(String)`.

use std::io::Write;
use std::process::{Command, Stdio};

/// Like [`run`] but returns raw stdout bytes — needed when output is framed by
/// byte counts (e.g. `git cat-file --batch`) or may not be valid UTF-8.
pub fn run_bytes(repo: &str, args: &[&str], stdin: Option<&str>) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to launch git: {e}"))?;

    // Drain stdout/stderr while the input is being written. A large
    // `cat-file --batch` response can otherwise fill the child's stdout pipe
    // before the parent has finished writing stdin, leaving both processes
    // blocked on opposite pipes.
    let writer = stdin
        .map(|data| {
            let mut sink = child
                .stdin
                .take()
                .ok_or_else(|| "failed to open git stdin".to_string())?;
            let input = data.as_bytes().to_vec();
            Ok::<_, String>(std::thread::spawn(move || {
                sink.write_all(&input)
                    .map_err(|e| format!("failed to write to git stdin: {e}"))
            }))
        })
        .transpose()?;

    let out = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for git: {e}"))?;

    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| "git stdin writer thread panicked".to_string())??;
    }

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out.stdout)
}

/// Run `git -C <repo> <args...>`.
///
/// * `env`   — extra environment variables (used to drive `commit-tree`).
/// * `stdin` — data piped to git's stdin (the commit message for `commit-tree`).
pub fn run(
    repo: &str,
    args: &[&str],
    env: &[(&str, &str)],
    stdin: Option<&str>,
) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo);
    cmd.args(args);

    // Keep git non-interactive and locale-stable.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("LC_ALL", "C");
    for (k, v) in env {
        cmd.env(k, v);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to launch git (is it installed?): {e}"))?;

    if let Some(data) = stdin {
        // Take + drop the handle so git sees EOF once the message is written.
        let mut sink = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open git stdin".to_string())?;
        sink.write_all(data.as_bytes())
            .map_err(|e| format!("failed to write to git stdin: {e}"))?;
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for git: {e}"))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if stderr.is_empty() {
            format!("git {} exited with {}", args.join(" "), out.status)
        } else {
            stderr
        };
        return Err(msg);
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
