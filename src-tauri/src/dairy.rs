//! The dairy: spawns install/run commands inside a PTY, streams cleaned
//! output, and guarantees the whole process tree dies when a bottle is
//! capped. Orphaned dev servers are how milk spoils.

use once_cell::sync::Lazy;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use regex::Regex;
use std::io::Read;
use std::path::Path;

/// One emitted unit of terminal output.
pub enum Chunk {
    /// a completed line (ended with \n)
    Line(String),
    /// a carriage-return rewrite of the current line (progress bars)
    Rewrite(String),
}

static ANSI: Lazy<Regex> = Lazy::new(|| {
    // CSI sequences, OSC sequences, and stray ESC codes
    Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)?|\x1b[@-_]")
        .expect("ansi regex")
});

fn clean(s: &str) -> String {
    let s = ANSI.replace_all(s, "");
    s.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .collect()
}

/// Join argv into a single shell line, quoting args that need it.
fn shell_join(cmd: &[String]) -> String {
    cmd.iter()
        .map(|a| {
            if a == "&&" || (!a.is_empty() && !a.contains(' ') && !a.contains('"')) {
                a.clone()
            } else {
                format!("\"{}\"", a.replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn builder(cmd: &[String], cwd: &Path) -> CommandBuilder {
    let line = shell_join(cmd);
    let mut b = if cfg!(windows) {
        let mut b = CommandBuilder::new("cmd");
        b.args(["/C", &line]);
        b
    } else {
        let mut b = CommandBuilder::new("/bin/sh");
        b.args(["-c", &line]);
        b
    };
    b.cwd(cwd);
    b.env("FORCE_COLOR", "0");
    b.env("NO_COLOR", "1");
    b.env("CI", "false");
    b
}

/// Incremental byte stream -> Chunk splitter.
pub struct Churn {
    buf: String,
}

impl Churn {
    pub fn new() -> Self {
        Churn { buf: String::new() }
    }

    pub fn feed(&mut self, bytes: &[u8], mut out: impl FnMut(Chunk)) {
        let text = String::from_utf8_lossy(bytes);
        for ch in text.chars() {
            match ch {
                '\n' => {
                    let line = clean(&std::mem::take(&mut self.buf));
                    out(Chunk::Line(line));
                }
                '\r' => {
                    let line = clean(&std::mem::take(&mut self.buf));
                    if !line.is_empty() {
                        out(Chunk::Rewrite(line));
                    }
                }
                _ => self.buf.push(ch),
            }
        }
    }

    pub fn flush(&mut self, mut out: impl FnMut(Chunk)) {
        if !self.buf.is_empty() {
            let line = clean(&std::mem::take(&mut self.buf));
            if !line.is_empty() {
                out(Chunk::Line(line));
            }
        }
    }
}

/// A running long-lived process (the DELIVER stage).
pub struct Brew {
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    pub pid: Option<u32>,
}

impl Brew {
    /// Kill the entire process tree, not just the shell wrapper.
    pub fn cap(&mut self) {
        if let Some(pid) = self.pid {
            kill_tree(pid);
        }
        let _ = self.child.kill();
    }

    /// Blocking wait for exit after the reader has hit EOF.
    pub fn wait(&mut self) -> u32 {
        self.child
            .wait()
            .map(|s| s.exit_code())
            .unwrap_or(1)
    }

    pub fn try_wait(&mut self) -> Option<u32> {
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|s| s.exit_code())
    }
}

#[cfg(windows)]
fn kill_tree(pid: u32) {
    // taskkill /T walks the child tree; blunt but reliable.
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output();
}

#[cfg(unix)]
fn kill_tree(pid: u32) {
    unsafe {
        // portable-pty children are session leaders: negative pid signals
        // the whole process group.
        libc::kill(-(pid as i32), libc::SIGTERM);
        std::thread::sleep(std::time::Duration::from_millis(300));
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

fn open_pty(
    cmd: &[String],
    cwd: &Path,
) -> Result<(Brew, Box<dyn Read + Send>), String> {
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    let child = pair
        .slave
        .spawn_command(builder(cmd, cwd))
        .map_err(|e| e.to_string())?;
    drop(pair.slave);
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| e.to_string())?;
    let pid = child.process_id();
    Ok((
        Brew {
            child,
            _master: pair.master,
            pid,
        },
        reader,
    ))
}

/// Run a command to completion (the BOTTLE stage), streaming chunks.
/// Returns the exit code.
pub fn pour(
    cmd: &[String],
    cwd: &Path,
    mut on_chunk: impl FnMut(Chunk),
) -> Result<u32, String> {
    let (mut brew, mut reader) = open_pty(cmd, cwd)?;
    let mut churn = Churn::new();
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => churn.feed(&buf[..n], &mut on_chunk),
            Err(_) => break,
        }
    }
    churn.flush(&mut on_chunk);
    let status = brew.child.wait().map_err(|e| e.to_string())?;
    Ok(status.exit_code())
}

/// Start a long-lived command (the DELIVER stage). Caller owns the reader
/// loop and the Brew handle.
pub fn tap(cmd: &[String], cwd: &Path) -> Result<(Brew, Box<dyn Read + Send>), String> {
    open_pty(cmd, cwd)
}
