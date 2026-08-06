//! The dairy: spawns install/run commands inside a PTY, streams cleaned
//! output, and guarantees the whole process tree dies when a bottle is
//! capped. Orphaned dev servers are how milk spoils.

use once_cell::sync::Lazy;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use regex::Regex;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const PTY_ROWS: u16 = 40;
const PTY_COLS: u16 = 120;

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
///
/// CAUTION (Windows): the joined line is itself re-quoted by CommandBuilder
/// when it becomes cmd.exe's `/C` argument, and cmd does not understand
/// backslash-escaped quotes — so args containing spaces or quotes can get
/// shredded. Resolver-built commands are space-free per arg by
/// construction; prefer script files over inline `-e`/`-c` code snippets.
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

fn builder(cmd: &[String], cwd: &Path, env: &[(String, String)]) -> CommandBuilder {
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
    for (k, v) in env {
        b.env(k, v);
    }
    b
}

/// Incremental byte stream -> Chunk splitter, with just enough terminal
/// semantics to keep ConPTY output honest:
///
/// - `\r\n` is an ordinary line ending; only a *lone* `\r` (or a
///   cursor-to-column-1 CSI) means "progress-bar rewrite". The decision is
///   deferred one character, across feeds if needed. Treating every `\r`
///   as a rewrite collapses the whole log into one line on Windows and
///   hides lines from the URL sniffer.
/// - Backspace pops the previous character — conhost redraws single-char
///   spinners this way; stripping it instead concatenates every frame
///   into strings like `⠙⠹⠸⠼⠴`.
/// - Erase-line clears the pending line; clear-screen / cursor-home
///   finishes it — vite's screen clear otherwise glues its banner onto
///   the previous line.
/// - Bytes of a UTF-8 character split across reads are carried over, so
///   multibyte glyphs (braille spinners, box drawing) never shred.
enum Ansi {
    Normal,
    Esc,
    Csi(String),
}

pub struct Churn {
    buf: String,
    pending_cr: bool,
    ansi: Ansi,
    utf8_carry: Vec<u8>,
}

impl Churn {
    pub fn new() -> Self {
        Churn {
            buf: String::new(),
            pending_cr: false,
            ansi: Ansi::Normal,
            utf8_carry: Vec::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8], mut out: impl FnMut(Chunk)) {
        let mut data = std::mem::take(&mut self.utf8_carry);
        data.extend_from_slice(bytes);
        let text: String = match std::str::from_utf8(&data) {
            Ok(s) => s.to_string(),
            Err(e) if e.error_len().is_none() => {
                // clean prefix + an incomplete char at the end: hold the
                // tail bytes for the next read
                let valid = e.valid_up_to();
                self.utf8_carry = data[valid..].to_vec();
                // SAFETY-free path: valid_up_to guarantees this range parses
                std::str::from_utf8(&data[..valid]).unwrap_or_default().to_string()
            }
            Err(_) => String::from_utf8_lossy(&data).into_owned(),
        };

        for ch in text.chars() {
            match &mut self.ansi {
                Ansi::Normal => {
                    if self.pending_cr {
                        self.pending_cr = false;
                        if ch == '\n' {
                            // \r\n — an ordinary line ending
                            let line = clean(&std::mem::take(&mut self.buf));
                            out(Chunk::Line(line));
                            continue;
                        }
                        // lone \r — the current line is being rewritten
                        let line = clean(&std::mem::take(&mut self.buf));
                        if !line.is_empty() {
                            out(Chunk::Rewrite(line));
                        }
                    }
                    match ch {
                        '\n' => {
                            let line = clean(&std::mem::take(&mut self.buf));
                            out(Chunk::Line(line));
                        }
                        '\r' => self.pending_cr = true,
                        '\x08' => {
                            self.buf.pop();
                        }
                        '\x1b' => self.ansi = Ansi::Esc,
                        _ => self.buf.push(ch),
                    }
                }
                Ansi::Esc => {
                    if ch == '[' {
                        self.ansi = Ansi::Csi(String::new());
                    } else {
                        // not CSI (e.g. OSC) — put it back for clean()
                        self.buf.push('\x1b');
                        self.buf.push(ch);
                        self.ansi = Ansi::Normal;
                    }
                }
                Ansi::Csi(params) => {
                    if ('@'..='~').contains(&ch) {
                        let params = std::mem::take(params);
                        self.ansi = Ansi::Normal;
                        match ch {
                            // cursor to column 1: same meaning as a lone \r
                            'G' if matches!(params.as_str(), "" | "0" | "1") => {
                                let line = clean(&std::mem::take(&mut self.buf));
                                if !line.is_empty() {
                                    out(Chunk::Rewrite(line));
                                }
                            }
                            // erase entire line: drop the pending content
                            'K' if params == "2" => self.buf.clear(),
                            'K' => {} // erase-to-EOL: nothing tracked to the right
                            // clear screen / cursor home: the line is done
                            'J' | 'H' | 'f' => {
                                if !self.buf.is_empty() {
                                    let line = clean(&std::mem::take(&mut self.buf));
                                    out(Chunk::Line(line));
                                }
                            }
                            // anything else (colors, modes): back into the
                            // buffer for clean() to strip at line end
                            _ => {
                                self.buf.push('\x1b');
                                self.buf.push('[');
                                self.buf.push_str(&params);
                                self.buf.push(ch);
                            }
                        }
                    } else {
                        params.push(ch);
                    }
                }
            }
        }
    }

    pub fn flush(&mut self, mut out: impl FnMut(Chunk)) {
        self.pending_cr = false;
        self.ansi = Ansi::Normal;
        self.utf8_carry.clear();
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
    master: Option<Box<dyn MasterPty + Send>>,
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

    /// Drop the PTY master. ConPTY keeps its output pipe open for as long
    /// as the pseudo console exists — a blocked reader only unblocks once
    /// the master is gone, so this is how the route escapes the read loop
    /// after the child has exited (or been capped).
    pub fn release_pty(&mut self) {
        self.master = None;
    }
}

// ---------------------------------------------------------------- doorman

/// Terminal-side answers to ConPTY's VT queries. conhost forwards these to
/// the hosting terminal and BLOCKS the child's console calls until they are
/// answered — a PTY host that never replies hangs the child before it even
/// starts its real work (the "stuck at npm install" stall).
const VT_QUERIES: &[(&[u8], &[u8])] = &[
    (b"\x1b[6n", b"\x1b[1;1R"), // DSR — report cursor at home
    (b"\x1b[0c", b"\x1b[?1;0c"), // DA1 (explicit param) — plain VT100
    (b"\x1b[c", b"\x1b[?1;0c"), // DA1 — plain VT100
    (b"\x1b[18t", b"\x1b[8;40;120t"), // text-area size — matches PtySize
];

struct Doorman {
    carry: Vec<u8>,
}

impl Doorman {
    fn new() -> Self {
        Doorman { carry: Vec::new() }
    }

    /// Scan a chunk of PTY output for queries and answer each one. Keeps a
    /// small carry so a query split across two reads is still recognized.
    fn answer(&mut self, bytes: &[u8], writer: &mut (dyn Write + Send)) {
        let mut hay = std::mem::take(&mut self.carry);
        hay.extend_from_slice(bytes);

        let mut replied = false;
        let mut i = 0;
        while i < hay.len() {
            let hit = VT_QUERIES.iter().find(|(q, _)| hay[i..].starts_with(q));
            match hit {
                Some((q, reply)) => {
                    let _ = writer.write_all(reply);
                    replied = true;
                    i += q.len();
                }
                None => i += 1,
            }
        }
        if replied {
            let _ = writer.flush();
        }

        // keep the longest tail that is a strict prefix of some query, so a
        // straddling query is completed next read without double-answering
        let max_carry = 4.min(hay.len());
        let keep = (1..=max_carry)
            .rev()
            .find(|&k| {
                let tail = &hay[hay.len() - k..];
                VT_QUERIES.iter().any(|(q, _)| k < q.len() && q.starts_with(tail))
            })
            .unwrap_or(0);
        self.carry = hay[hay.len() - keep..].to_vec();
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
    env: &[(String, String)],
) -> Result<(Brew, Box<dyn Read + Send>, Box<dyn Write + Send>), String> {
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: PTY_ROWS,
            cols: PTY_COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    let child = pair
        .slave
        .spawn_command(builder(cmd, cwd, env))
        .map_err(|e| e.to_string())?;
    drop(pair.slave);
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| e.to_string())?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| e.to_string())?;
    let pid = child.process_id();
    Ok((
        Brew {
            child,
            master: Some(pair.master),
            pid,
        },
        reader,
        writer,
    ))
}

/// Run a command to completion, streaming chunks. Returns the exit code.
/// Retained convenience wrapper over `serve` for callers that don't need a
/// shared Brew slot or cancellation.
#[allow(dead_code)]
pub fn pour(
    cmd: &[String],
    cwd: &Path,
    on_chunk: impl FnMut(Chunk),
) -> Result<u32, String> {
    let slot = Arc::new(Mutex::new(None));
    let cancel = Arc::new(AtomicBool::new(false));
    serve(cmd, cwd, &[], &slot, &cancel, on_chunk)
}

/// Start a long-lived command. Caller owns the reader loop, the Brew
/// handle, and answering VT queries on the writer.
#[allow(dead_code)]
pub fn tap(
    cmd: &[String],
    cwd: &Path,
) -> Result<(Brew, Box<dyn Read + Send>, Box<dyn Write + Send>), String> {
    open_pty(cmd, cwd, &[])
}

/// How long output may keep trickling after the child exits (or the run is
/// canceled) before the PTY master is dropped to force the reader loose.
const DRAIN_GRACE: Duration = Duration::from_millis(400);

/// Spawn `cmd` in a PTY, park the Brew in `slot`, and stream output chunks
/// until the process ends or `cancel` trips. Returns the exit code.
///
/// This is the one place that deals with the two ConPTY facts of life that
/// otherwise hang THE ROUTE on Windows:
///   1. conhost blocks the child's console calls until the terminal answers
///      its VT queries — the Doorman replies from the reader thread;
///   2. the reader never sees EOF while the PTY master is alive — a
///      watchdog polls the child and releases the master to unblock it.
pub fn serve(
    cmd: &[String],
    cwd: &Path,
    env: &[(String, String)],
    slot: &Arc<Mutex<Option<Brew>>>,
    cancel: &Arc<AtomicBool>,
    mut on_chunk: impl FnMut(Chunk),
) -> Result<u32, String> {
    let (brew, mut reader, mut writer) = open_pty(cmd, cwd, env)?;
    *slot.lock().unwrap() = Some(brew);

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let pump = std::thread::spawn(move || {
        let mut doorman = Doorman::new();
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    doorman.answer(&buf[..n], &mut *writer);
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut churn = Churn::new();
    let mut drain_deadline: Option<Instant> = None;
    loop {
        if drain_deadline.is_none() {
            let over = cancel.load(Ordering::SeqCst)
                || slot
                    .lock()
                    .unwrap()
                    .as_mut()
                    .map_or(true, |b| b.try_wait().is_some());
            if over {
                drain_deadline = Some(Instant::now() + DRAIN_GRACE);
            }
        }
        if let Some(deadline) = drain_deadline {
            if Instant::now() >= deadline {
                if let Some(b) = slot.lock().unwrap().as_mut() {
                    b.release_pty();
                }
                while let Ok(bytes) = rx.recv_timeout(Duration::from_millis(500)) {
                    churn.feed(&bytes, &mut on_chunk);
                }
                break;
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => churn.feed(&bytes, &mut on_chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    churn.flush(&mut on_chunk);

    let code = {
        let mut s = slot.lock().unwrap();
        let code = match s.as_mut() {
            Some(b) => {
                // a cancel that raced in before the Brew reached the slot
                // still has to kill the process, or wait() blocks forever
                if cancel.load(Ordering::SeqCst) && b.try_wait().is_none() {
                    b.cap();
                }
                b.release_pty();
                b.wait()
            }
            None => 1,
        };
        *s = None;
        code
    };
    let _ = pump.join();
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn churn_all(feeds: &[&[u8]]) -> (Vec<String>, Vec<String>) {
        let mut churn = Churn::new();
        let (mut lines, mut rewrites) = (Vec::new(), Vec::new());
        for feed in feeds {
            churn.feed(feed, |c| match c {
                Chunk::Line(t) => lines.push(t),
                Chunk::Rewrite(t) => rewrites.push(t),
            });
        }
        churn.flush(|c| {
            if let Chunk::Line(t) = c {
                lines.push(t)
            }
        });
        (lines, rewrites)
    }

    /// ConPTY ends every line with \r\n — those are ordinary lines, not
    /// progress rewrites. Getting this wrong hides every line from the
    /// URL sniffer and collapses the log into one line on Windows.
    #[test]
    fn churn_crlf_is_a_line_not_a_rewrite() {
        let (lines, rewrites) =
            churn_all(&[b"Available on:\r\n  http://127.0.0.1:8080\r\n"]);
        assert_eq!(lines, vec!["Available on:", "  http://127.0.0.1:8080"]);
        assert!(rewrites.is_empty(), "CRLF must not produce rewrites: {rewrites:?}");
    }

    #[test]
    fn churn_lone_cr_is_still_a_rewrite() {
        let (lines, rewrites) = churn_all(&[b"progress 10%\rprogress 50%\r\ndone\r\n"]);
        assert_eq!(rewrites, vec!["progress 10%"]);
        assert_eq!(lines, vec!["progress 50%", "done"]);
    }

    #[test]
    fn churn_crlf_split_across_feeds() {
        let (lines, rewrites) = churn_all(&[b"alpha\r", b"\nbeta\r\n"]);
        assert_eq!(lines, vec!["alpha", "beta"]);
        assert!(rewrites.is_empty(), "split CRLF must not produce rewrites: {rewrites:?}");
    }

    /// conhost redraws single-char spinners with backspaces; frames must
    /// replace each other, not concatenate into `⠙⠹⠸⠼`.
    #[test]
    fn churn_backspace_replaces_spinner_frames() {
        let (lines, _) =
            churn_all(&["⠙\u{8}⠹\u{8}⠸ done\r\n".as_bytes()]);
        assert_eq!(lines, vec!["⠸ done"]);
    }

    /// erase-line drops the transient frame; cursor-to-column-1 acts like
    /// a lone \r (rewrite).
    #[test]
    fn churn_erase_line_and_column_one() {
        let (lines, rewrites) =
            churn_all(&[b"frame1\x1b[2Kframe2\r\n10%\x1b[G20%\r\n"]);
        assert_eq!(lines, vec!["frame2", "20%"]);
        assert_eq!(rewrites, vec!["10%"]);
    }

    /// vite clears the screen before its banner — that must finish the
    /// pending line instead of gluing the banner onto it.
    #[test]
    fn churn_clear_screen_finishes_line() {
        let (lines, _) =
            churn_all(&[b"> vite\x1b[2J\x1b[1;1H[vite] ready\r\n"]);
        assert_eq!(lines, vec!["> vite", "[vite] ready"]);
    }

    /// a multibyte glyph split across two reads must survive intact
    #[test]
    fn churn_utf8_split_across_feeds() {
        let bytes = "⠼ tick\r\n".as_bytes();
        let (lines, _) = churn_all(&[&bytes[..2], &bytes[2..]]);
        assert_eq!(lines, vec!["⠼ tick"]);
    }

    /// color codes still reach clean() and get stripped
    #[test]
    fn churn_sgr_codes_still_stripped() {
        let (lines, _) = churn_all(&[b"\x1b[32mgreen\x1b[0m ok\r\n"]);
        assert_eq!(lines, vec!["green ok"]);
    }

    fn run_serve(
        cmd: Vec<String>,
        cwd: std::path::PathBuf,
        slot: Arc<Mutex<Option<Brew>>>,
        cancel: Arc<AtomicBool>,
    ) -> mpsc::Receiver<Result<u32, String>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut lines = 0u32;
            let res = serve(&cmd, &cwd, &[], &slot, &cancel, |c| {
                if let Chunk::Line(t) = c {
                    lines += 1;
                    eprintln!("  | {t}");
                }
            });
            eprintln!("serve done: {res:?} ({lines} lines)");
            let _ = tx.send(res);
        });
        rx
    }

    /// The reported BOTTLE hang: `npm install` must complete and return —
    /// requires answering ConPTY's VT queries AND forcing reader EOF after
    /// exit. Point MILKMAN_TEST_PKG at a duplicated copy of
    /// test_directory/AC7_FACTION_PACKAGE.zip's contents.
    #[test]
    fn bottle_install_completes() {
        let Some(root) = std::env::var_os("MILKMAN_TEST_PKG") else {
            eprintln!("MILKMAN_TEST_PKG not set — skipping");
            return;
        };
        let cmd = vec!["npm".to_string(), "install".to_string()];
        let slot = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let rx = run_serve(cmd, root.into(), slot, cancel);
        match rx.recv_timeout(Duration::from_secs(180)) {
            Ok(Ok(code)) => assert_eq!(code, 0, "npm install failed"),
            Ok(Err(e)) => panic!("could not spawn npm install: {e}"),
            Err(_) => panic!("serve() never returned — BOTTLE hang regression"),
        }
    }

    /// Full DELIVER pass over the designated test package: `npm run dev`
    /// must produce a sniffable local URL, and capping must end the serve.
    #[test]
    fn deliver_sniffs_url_and_caps() {
        let Some(root) = std::env::var_os("MILKMAN_TEST_PKG") else {
            eprintln!("MILKMAN_TEST_PKG not set — skipping");
            return;
        };
        let root = std::path::PathBuf::from(root);

        // BOTTLE first so http-server is present (idempotent re-install)
        let slot = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let rx = run_serve(
            vec!["npm".into(), "install".into()],
            root.clone(),
            slot.clone(),
            cancel.clone(),
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(180))
                .expect("install hang")
                .expect("install spawn"),
            0
        );

        // DELIVER: watch the stream for the doorstep address. Runs the
        // installed http-server directly, skipping the dev script's `-o`
        // so automated test runs don't pop a browser tab.
        let slot = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let (url_tx, url_rx) = mpsc::channel::<String>();
        let (done_tx, done_rx) = mpsc::channel();
        {
            let (slot, cancel) = (slot.clone(), cancel.clone());
            let cmd = vec![
                "node".to_string(),
                "node_modules/http-server/bin/http-server".to_string(),
                ".".to_string(),
                "-p".to_string(),
                "8123".to_string(),
            ];
            std::thread::spawn(move || {
                let res = serve(&cmd, &root, &[], &slot, &cancel, |c| {
                    let (Chunk::Line(t) | Chunk::Rewrite(t)) = c;
                    if let Some(url) = crate::porter::sniff(&t) {
                        let _ = url_tx.send(url);
                    }
                });
                let _ = done_tx.send(res);
            });
        }
        let url = url_rx
            .recv_timeout(Duration::from_secs(60))
            .expect("no local URL sniffed from dev server — DELIVER broken");
        eprintln!("sniffed: {url}");
        assert!(url.starts_with("http://localhost"), "unexpected url: {url}");

        cancel.store(true, Ordering::SeqCst);
        if let Some(b) = slot.lock().unwrap().as_mut() {
            b.cap();
        }
        match done_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(_) => {}
            Err(_) => panic!("serve() still blocked 10s after cap"),
        }
    }

    /// Extra env pairs (the PORT= steering path) must reach the child.
    #[test]
    fn serve_passes_extra_env() {
        let dir = std::env::temp_dir().join("milkman-env-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(
            dir.join("env.js"),
            "console.log('milk-port=' + process.env.PORT);\n",
        )
        .expect("write env.js");
        std::fs::write(dir.join("package.json"), "{\"type\":\"commonjs\"}\n")
            .expect("write package.json");

        let slot = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let mut lines: Vec<String> = Vec::new();
        let code = serve(
            &["node".to_string(), "env.js".to_string()],
            &dir,
            &[("PORT".to_string(), "4567".to_string())],
            &slot,
            &cancel,
            |c| {
                if let Chunk::Line(t) = c {
                    lines.push(t)
                }
            },
        )
        .expect("spawn node env.js");
        assert_eq!(code, 0);
        assert!(
            lines.iter().any(|l| l == "milk-port=4567"),
            "PORT env did not reach the child: {lines:?}"
        );
    }

    /// Force-cancel: capping a long-lived process must unblock serve()
    /// promptly (the cancel button depends on this).
    #[test]
    fn cancel_unblocks_serve() {
        // a script file sidesteps cmd.exe's nested-quote mangling of
        // inline `-e` code (see shell_join's caution note)
        let dir = std::env::temp_dir().join("milkman-cancel-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(
            dir.join("tick.js"),
            "setInterval(() => console.log('tick'), 200);\n",
        )
        .expect("write tick.js");
        // stop node's package.json walk here — stray/broken manifests in
        // the surrounding temp dir must not break the test
        std::fs::write(dir.join("package.json"), "{\"type\":\"commonjs\"}\n")
            .expect("write package.json");
        let cmd = vec!["node".to_string(), "tick.js".to_string()];
        let slot = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let rx = run_serve(cmd, dir, slot.clone(), cancel.clone());

        // let it start, then cap it the way stop_bottle does
        std::thread::sleep(Duration::from_secs(3));
        cancel.store(true, Ordering::SeqCst);
        if let Some(b) = slot.lock().unwrap().as_mut() {
            b.cap();
        }
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => panic!("could not spawn node: {e}"),
            Err(_) => panic!("serve() still blocked 10s after cap — cancel would hang"),
        }
    }
}
