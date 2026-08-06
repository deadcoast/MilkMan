//! MILKMAN core — commands, state, and THE ROUTE:
//! INSPECT -> BOTTLE -> CHILL -> DELIVER.

mod dairy;
mod ledger;
mod porter;
mod resolver;
mod scanner;
mod statics;

use dairy::Chunk;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{LineWriter, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

// ---------------------------------------------------------------- events

#[derive(Clone, Serialize)]
struct LinePayload {
    bottle: u32,
    kind: String,
    text: String,
    rewrite: bool,
}

#[derive(Clone, Serialize)]
struct ProfileView {
    stack: String,
    glyph: String,
    manager: String,
    framework: Option<String>,
    confidence: String,
    scripts: Vec<String>,
}

#[derive(Clone, Serialize)]
struct StatePayload {
    bottle: u32,
    state: String,
    url: Option<String>,
    port: Option<u16>,
    profile: Option<ProfileView>,
}

#[derive(Clone, Serialize)]
pub struct BottleInfo {
    id: u32,
    name: String,
    path: String,
    state: String,
    stack: Option<String>,
    glyph: Option<String>,
    manager: Option<String>,
    framework: Option<String>,
    last_url: Option<String>,
    restored: bool,
}

#[derive(Clone, Serialize)]
struct LogLine {
    kind: String,
    text: String,
}

fn emit_line(app: &AppHandle, bottle: u32, kind: &str, text: impl Into<String>) {
    let text = text.into();
    // mirror every finished line into the bottle's last-log file so a
    // restored bottle can replay its previous route
    if let Some(milk) = app.try_state::<Milk>() {
        if let Some(w) = milk.logs.lock().unwrap().get_mut(&bottle) {
            let _ = writeln!(w, "{kind}\t{text}");
        }
    }
    let _ = app.emit(
        "route:line",
        LinePayload {
            bottle,
            kind: kind.into(),
            text,
            rewrite: false,
        },
    );
}

fn emit_rewrite(app: &AppHandle, bottle: u32, text: impl Into<String>) {
    let _ = app.emit(
        "route:line",
        LinePayload {
            bottle,
            kind: "raw".into(),
            text: text.into(),
            rewrite: true,
        },
    );
}

fn emit_state(
    app: &AppHandle,
    bottle: u32,
    state: &str,
    url: Option<String>,
    port: Option<u16>,
    profile: Option<ProfileView>,
) {
    let _ = app.emit(
        "route:state",
        StatePayload {
            bottle,
            state: state.into(),
            url,
            port,
            profile,
        },
    );
}

fn profile_view(p: &resolver::StackProfile) -> ProfileView {
    ProfileView {
        stack: p.stack.label().into(),
        glyph: p.stack.glyph().into(),
        manager: p.manager.clone(),
        framework: p.framework.clone(),
        confidence: p.confidence.label().into(),
        scripts: p.scripts.clone(),
    }
}

/// Glyph for a stack label persisted in the ledger (restored bottles).
fn glyph_for(stack: &str) -> &'static str {
    match stack {
        "NODE" => "N",
        "PYTHON" => "PY",
        "RUST" => "RS",
        "GO" => "GO",
        "RUBY" => "RB",
        "STATIC" => "ST",
        _ => "?",
    }
}

// ---------------------------------------------------------------- state

/// Per-pour overrides set by the CLI (`pour --port N --script name`).
/// Sticky until the next explicit pour replaces them, so watch-triggered
/// re-pours keep the user's choices.
#[derive(Clone, Default)]
struct RouteOverrides {
    port: Option<u16>,
    script: Option<String>,
}

struct Ctl {
    path: PathBuf,
    name: String,
    cancel: Arc<AtomicBool>,
    brew: Arc<Mutex<Option<dairy::Brew>>>,
    icebox: Arc<Mutex<Option<statics::Icebox>>>,
    overrides: Arc<Mutex<RouteOverrides>>,
    watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
}

impl Ctl {
    fn new(path: PathBuf, name: String) -> Ctl {
        Ctl {
            path,
            name,
            cancel: Arc::new(AtomicBool::new(false)),
            brew: Arc::new(Mutex::new(None)),
            icebox: Arc::new(Mutex::new(None)),
            overrides: Arc::new(Mutex::new(RouteOverrides::default())),
            watcher: Arc::new(Mutex::new(None)),
        }
    }
}

struct Milk {
    bottles: Mutex<HashMap<u32, Ctl>>,
    next: AtomicU32,
    ledger: Arc<Mutex<ledger::Ledger>>,
    ledger_file: PathBuf,
    workspace: PathBuf,
    logs_dir: PathBuf,
    /// open last-log files, keyed by bottle id — emit_line mirrors into these
    logs: Mutex<HashMap<u32, LineWriter<std::fs::File>>>,
}

impl Milk {
    fn new(data_dir: PathBuf) -> Self {
        let ledger_file = data_dir.join("ledger.json");
        let workspace = data_dir.join("workspace");
        let logs_dir = data_dir.join("logs");
        let _ = std::fs::create_dir_all(&workspace);
        let _ = std::fs::create_dir_all(&logs_dir);
        Milk {
            bottles: Mutex::new(HashMap::new()),
            next: AtomicU32::new(1),
            ledger: Arc::new(Mutex::new(ledger::Ledger::load(&ledger_file))),
            ledger_file,
            workspace,
            logs_dir,
            logs: Mutex::new(HashMap::new()),
        }
    }

    fn log_path(&self, package: &Path) -> PathBuf {
        self.logs_dir
            .join(format!("{}.log", ledger::hash_str(&package.to_string_lossy())))
    }
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package".into())
}

// ---------------------------------------------------------------- ports

/// Ports worth trying when a package's own choice is taken.
const COMMON_PORTS: &[u16] = &[
    3000, 3001, 4000, 4173, 4321, 5000, 5173, 5174, 8000, 8080, 8081, 8888,
];

/// How many times DELIVER re-pours onto another port before giving up.
const MAX_REPOURS: usize = 4;

/// Pick the first common port that is currently free and untried, and
/// rebuild the run command pinned to it. None when the stack can't be
/// steered or every candidate is busy.
fn steer_to_free_port(
    profile: &resolver::StackProfile,
    tried: &[u16],
) -> Option<(Vec<String>, Vec<(String, String)>, u16)> {
    let port = COMMON_PORTS
        .iter()
        .copied()
        .filter(|p| !tried.contains(p))
        .find(|p| TcpListener::bind(("127.0.0.1", *p)).is_ok())?;
    let (cmd, env) = profile.run_with_port(port)?;
    Some((cmd, env, port))
}

// ---------------------------------------------------------------- zip intake

fn unpack_zip(zip_path: &Path, workspace: &Path) -> Result<PathBuf, String> {
    let stem = zip_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package".into());
    let dest = workspace.join(&stem);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&dest);
    }
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(&dest).map_err(|e| e.to_string())?;

    // if the zip wraps everything in a single folder, descend into it
    let entries: Vec<_> = std::fs::read_dir(&dest)
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    if entries.len() == 1 && entries[0].path().is_dir() {
        return Ok(entries[0].path());
    }
    Ok(dest)
}

// ---------------------------------------------------------------- the route

#[allow(clippy::too_many_arguments)]
fn run_route(
    app: AppHandle,
    id: u32,
    root: PathBuf,
    cancel: Arc<AtomicBool>,
    brew_slot: Arc<Mutex<Option<dairy::Brew>>>,
    icebox_slot: Arc<Mutex<Option<statics::Icebox>>>,
    ledger: Arc<Mutex<ledger::Ledger>>,
    ledger_file: PathBuf,
    overrides: RouteOverrides,
) {
    let canceled = || cancel.load(Ordering::SeqCst);

    // ---- [1/4] INSPECT ------------------------------------------------
    emit_state(&app, id, "inspecting", None, None, None);
    emit_line(&app, id, "stage", "[1/4] INSPECT \u{2500} reading the label");

    let ev = scanner::scan(&root);
    let count = ev.found.len();
    for (i, name) in ev.found.iter().enumerate() {
        let branch = if i + 1 == count { "\u{2514}\u{2500}" } else { "\u{251c}\u{2500}" };
        emit_line(&app, id, "tree", format!(" {branch} {name:<22} FOUND"));
    }
    if count == 0 {
        emit_line(&app, id, "tree", " \u{2514}\u{2500} nothing recognizable");
    }

    let Some(mut profile) = resolver::resolve(&ev) else {
        emit_line(&app, id, "err", "SPOILED \u{2500} no runnable stack detected in this package");
        emit_line(&app, id, "note", "expected one of: package.json, pyproject.toml, requirements.txt, Cargo.toml, go.mod, Gemfile, index.html");
        emit_state(&app, id, "spoiled", None, None, None);
        return;
    };

    // CLI `pour --script <name>`: run a chosen manager script instead of
    // the resolver's pick (node-family packages only)
    if let Some(script) = &overrides.script {
        let node_family = profile.scripts.iter().any(|s| s == script)
            || matches!(profile.stack, resolver::Stack::Node);
        if node_family && profile.manager != "builtin" {
            profile.run_cmd = vec![profile.manager.clone(), "run".into(), script.clone()];
            profile.stack = resolver::Stack::Node;
            if !profile.scripts.iter().any(|s| s == script) {
                emit_line(&app, id, "warn", format!(" ! \"{script}\" is not in package.json scripts \u{2500} pouring anyway"));
            }
            emit_line(&app, id, "note", format!(" script override \u{2500} \"{script}\""));
        } else {
            emit_line(&app, id, "warn", " ! --script only applies to node packages \u{2500} ignored");
        }
    }

    let fw = profile
        .framework
        .clone()
        .map(|f| format!(" \u{00b7} {f}"))
        .unwrap_or_default();
    emit_line(
        &app,
        id,
        "ok",
        format!(
            " stack: {}/{}{}  [{}]",
            profile.stack.label(),
            profile.manager,
            fw,
            profile.confidence.label()
        ),
    );
    for w in &profile.warnings {
        emit_line(&app, id, "warn", format!(" ! {w}"));
    }
    emit_state(&app, id, "inspecting", None, None, Some(profile_view(&profile)));
    if canceled() {
        return;
    }

    // ---- [2/4] BOTTLE -------------------------------------------------
    emit_state(&app, id, "bottling", None, None, Some(profile_view(&profile)));
    emit_line(&app, id, "stage", "[2/4] BOTTLE \u{2500} filling dependencies");

    let dep_hash = profile.hash_source.as_deref().and_then(ledger::hash_file);
    let unchanged = {
        let led = ledger.lock().unwrap();
        match (led.get(&root), &dep_hash) {
            (Some(entry), Some(h)) => entry.dep_hash.as_deref() == Some(h.as_str()),
            _ => false,
        }
    };
    let artifacts_present = profile
        .artifact_dir
        .map(|d| root.join(d).exists())
        .unwrap_or(false);

    match &profile.install_cmd {
        None => emit_line(&app, id, "note", " nothing to install \u{2500} package is ready to drink"),
        Some(_) if unchanged && artifacts_present => {
            emit_line(&app, id, "ok", " no change since last delivery \u{2500} skipping install");
        }
        Some(cmd) => {
            emit_line(&app, id, "note", format!(" $ {}", cmd.join(" ")));
            let poured = dairy::serve(cmd, &root, &[], &brew_slot, &cancel, |c| match c {
                Chunk::Line(t) => emit_line(&app, id, "raw", t),
                Chunk::Rewrite(t) => emit_rewrite(&app, id, t),
            });
            match poured {
                Err(e) => {
                    emit_line(&app, id, "err", format!("SPOILED \u{2500} could not start install: {e}"));
                    emit_state(&app, id, "spoiled", None, None, Some(profile_view(&profile)));
                    return;
                }
                Ok(code) => {
                    if canceled() {
                        return;
                    }
                    if code != 0 {
                        emit_line(&app, id, "err", format!("SPOILED \u{2500} install exited with code {code}"));
                        emit_state(&app, id, "spoiled", None, None, Some(profile_view(&profile)));
                        return;
                    }
                    emit_line(&app, id, "ok", " dependencies bottled");
                }
            }
        }
    }

    // record in the ledger now that deps are known-good
    {
        let mut led = ledger.lock().unwrap();
        let last_url = led.get(&root).and_then(|e| e.last_url.clone());
        led.upsert(ledger::Entry {
            path: root.clone(),
            name: dir_name(&root),
            dep_hash,
            stack: profile.stack.label().into(),
            manager: profile.manager.clone(),
            framework: profile.framework.clone(),
            last_url,
        });
        led.save(&ledger_file);
    }
    if canceled() {
        return;
    }

    // ---- [3/4] CHILL --------------------------------------------------
    emit_state(&app, id, "chilling", None, None, Some(profile_view(&profile)));
    emit_line(&app, id, "stage", "[3/4] CHILL \u{2500} checking the icebox");

    if ev.env_example && !ev.env_file {
        emit_line(&app, id, "warn", " ! .env.example present but no .env \u{2500} the app may need secrets");
    }
    if let Some(port) = profile.default_port {
        let free = TcpListener::bind(("127.0.0.1", port)).is_ok();
        if free {
            emit_line(&app, id, "ok", format!(" port {port} free"));
        } else {
            emit_line(&app, id, "warn", format!(" ! port {port} in use \u{2500} the server may pick another"));
        }
    } else {
        emit_line(&app, id, "note", " no default port for this stack \u{2500} watching output instead");
    }
    if canceled() {
        return;
    }

    // ---- [4/4] DELIVER ------------------------------------------------
    emit_state(&app, id, "delivering", None, None, Some(profile_view(&profile)));
    emit_line(&app, id, "stage", "[4/4] DELIVER \u{2500} on the doorstep");

    // static packages ride MILKMAN's own truck
    if profile.run_cmd.is_empty() {
        match statics::chill(root.clone()) {
            Ok(icebox) => {
                let url = format!("http://localhost:{}", icebox.port);
                let port = icebox.port;
                *icebox_slot.lock().unwrap() = Some(icebox);
                emit_line(&app, id, "url", format!(" \u{25cf} FRESH \u{2500} {url}"));
                emit_state(&app, id, "fresh", Some(url), Some(port), Some(profile_view(&profile)));
            }
            Err(e) => {
                emit_line(&app, id, "err", format!("SPOILED \u{2500} static server failed: {e}"));
                emit_state(&app, id, "spoiled", None, None, Some(profile_view(&profile)));
            }
        }
        return;
    }

    let mut run_cmd = profile.run_cmd.clone();
    let mut run_env: Vec<(String, String)> = Vec::new();
    let mut tried_ports: Vec<u16> = Vec::new();
    let mut repours = 0usize;

    // CLI `pour --port <n>` pins the doorstep explicitly; otherwise, if
    // the stack's own port is already visibly taken, steer to a free one
    // up front instead of crashing into it
    if let Some(p) = overrides.port {
        match profile.run_with_port(p) {
            Some((cmd, env)) => {
                emit_line(&app, id, "note", format!(" port override \u{2500} {p}"));
                tried_ports.push(p);
                run_cmd = cmd;
                run_env = env;
            }
            None => {
                emit_line(&app, id, "warn", " ! this stack's port can't be steered \u{2500} --port ignored");
            }
        }
    } else if let Some(dp) = profile.default_port {
        if TcpListener::bind(("127.0.0.1", dp)).is_err() {
            tried_ports.push(dp);
            if let Some((cmd, env, port)) = steer_to_free_port(&profile, &tried_ports) {
                emit_line(
                    &app,
                    id,
                    "warn",
                    format!(" ! port {dp} taken \u{2500} pouring on {port} instead"),
                );
                tried_ports.push(port);
                run_cmd = cmd;
                run_env = env;
            }
        }
    }

    loop {
        emit_line(&app, id, "note", format!(" $ {}", run_cmd.join(" ")));
        let mut delivered = false;
        // Some(named_port) once the child complains its port is occupied
        let mut clash: Option<Option<u16>> = None;
        let served = dairy::serve(&run_cmd, &root, &run_env, &brew_slot, &cancel, |c| {
            // the doorstep address can arrive on a finished line or inside
            // a progress-rewritten one — sniff both
            let text = match &c {
                Chunk::Line(t) | Chunk::Rewrite(t) => t,
            };
            let found = if delivered { None } else { porter::sniff(text) };
            if !delivered && clash.is_none() && porter::is_port_clash(text) {
                clash = Some(porter::clash_port(text));
            }
            match c {
                Chunk::Line(t) => emit_line(&app, id, "raw", t),
                Chunk::Rewrite(t) => emit_rewrite(&app, id, t),
            }
            if let Some(url) = found {
                delivered = true;
                let port = url.rsplit(':').next().and_then(|p| {
                    p.chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<u16>()
                        .ok()
                });
                // remember the doorstep for the next restore
                {
                    let mut led = ledger.lock().unwrap();
                    if led.set_url(&root, &url) {
                        led.save(&ledger_file);
                    }
                }
                emit_line(&app, id, "url", format!(" \u{25cf} FRESH \u{2500} {url}"));
                emit_state(&app, id, "fresh", Some(url), port, Some(profile_view(&profile)));
            }
        });
        let code = match served {
            Ok(code) => code,
            Err(e) => {
                emit_line(&app, id, "err", format!("SPOILED \u{2500} could not start: {e}"));
                emit_state(&app, id, "spoiled", None, None, Some(profile_view(&profile)));
                return;
            }
        };

        if canceled() {
            emit_line(&app, id, "note", " \u{2500}\u{2500} capped \u{2500}\u{2500}");
            return; // stop_bottle already emitted the capped state
        }
        if delivered {
            emit_line(&app, id, "note", format!(" process ended (exit {code})"));
            emit_state(&app, id, "capped", None, None, Some(profile_view(&profile)));
            return;
        }
        if code != 0 {
            if let Some(named_port) = clash {
                // the doorstep was occupied — try the next free common port
                if let Some(p) = named_port {
                    if !tried_ports.contains(&p) {
                        tried_ports.push(p);
                    }
                }
                if repours < MAX_REPOURS {
                    if let Some((cmd, env, port)) = steer_to_free_port(&profile, &tried_ports) {
                        repours += 1;
                        emit_line(
                            &app,
                            id,
                            "warn",
                            format!(
                                " ! port taken \u{2500} re-pouring on {port} ({repours}/{MAX_REPOURS})"
                            ),
                        );
                        tried_ports.push(port);
                        run_cmd = cmd;
                        run_env = env;
                        continue;
                    }
                }
                emit_line(
                    &app,
                    id,
                    "warn",
                    " ! every port MILKMAN tried is taken (or this stack's port can't be moved) \u{2500} free one up, then RE-POUR",
                );
            }
            emit_line(&app, id, "err", format!("SPOILED \u{2500} exited with code {code} before serving"));
            emit_state(&app, id, "spoiled", None, None, Some(profile_view(&profile)));
            return;
        }
        emit_line(&app, id, "note", " process finished without serving a URL");
        emit_state(&app, id, "capped", None, None, Some(profile_view(&profile)));
        return;
    }
}

// ---------------------------------------------------------------- commands

fn spawn_route(app: &AppHandle, milk: &Milk, id: u32) {
    let bottles = milk.bottles.lock().unwrap();
    let Some(ctl) = bottles.get(&id) else { return };
    ctl.cancel.store(false, Ordering::SeqCst);
    // (re)open this bottle's last-log file — one route per file, truncated
    // at the start of every pour
    if let Ok(f) = std::fs::File::create(milk.log_path(&ctl.path)) {
        milk.logs.lock().unwrap().insert(id, LineWriter::new(f));
    }
    let overrides = ctl.overrides.lock().unwrap().clone();
    let app = app.clone();
    let path = ctl.path.clone();
    let cancel = ctl.cancel.clone();
    let brew = ctl.brew.clone();
    let icebox = ctl.icebox.clone();
    let ledger = milk.ledger.clone();
    let ledger_file = milk.ledger_file.clone();
    std::thread::spawn(move || {
        run_route(app, id, path, cancel, brew, icebox, ledger, ledger_file, overrides);
    });
}

fn cap_bottle(ctl: &Ctl) {
    ctl.cancel.store(true, Ordering::SeqCst);
    if let Some(brew) = ctl.brew.lock().unwrap().as_mut() {
        brew.cap();
    }
    if let Some(icebox) = ctl.icebox.lock().unwrap().take() {
        icebox.stop();
    }
}

#[tauri::command]
fn load_project(app: AppHandle, milk: State<Milk>, path: String) -> Result<BottleInfo, String> {
    let mut root = PathBuf::from(&path);
    if root.is_file() {
        if root.extension().map(|e| e == "zip").unwrap_or(false) {
            root = unpack_zip(&root, &milk.workspace)?;
        } else {
            return Err("drop a folder or a .zip".into());
        }
    }
    if !root.is_dir() {
        return Err("that path is not a directory".into());
    }

    // re-drop of a known path re-runs the existing bottle
    {
        let bottles = milk.bottles.lock().unwrap();
        if let Some((&id, ctl)) = bottles.iter().find(|(_, c)| c.path == root) {
            let info = BottleInfo {
                id,
                name: ctl.name.clone(),
                path: root.to_string_lossy().into_owned(),
                state: "inspecting".into(),
                stack: None,
                glyph: None,
                manager: None,
                framework: None,
                last_url: None,
                restored: false,
            };
            cap_bottle(ctl);
            drop(bottles);
            spawn_route(&app, &milk, id);
            return Ok(info);
        }
    }

    let id = milk.next.fetch_add(1, Ordering::SeqCst);
    let name = dir_name(&root);
    let ctl = Ctl::new(root.clone(), name.clone());
    milk.bottles.lock().unwrap().insert(id, ctl);
    spawn_route(&app, &milk, id);
    Ok(BottleInfo {
        id,
        name,
        path: root.to_string_lossy().into_owned(),
        state: "inspecting".into(),
        stack: None,
        glyph: None,
        manager: None,
        framework: None,
        last_url: None,
        restored: false,
    })
}

#[tauri::command]
fn stop_bottle(app: AppHandle, milk: State<Milk>, id: u32) {
    let bottles = milk.bottles.lock().unwrap();
    if let Some(ctl) = bottles.get(&id) {
        // acknowledge the interrupt in the terminal, terminal-style, so
        // users learn ctrl+c is the cancel hotkey
        emit_line(&app, id, "sig", "^C");
        cap_bottle(ctl);
        emit_state(&app, id, "capped", None, None, None);
    }
}

#[tauri::command]
fn rerun_bottle(
    app: AppHandle,
    milk: State<Milk>,
    id: u32,
    port: Option<u16>,
    script: Option<String>,
) {
    {
        let bottles = milk.bottles.lock().unwrap();
        if let Some(ctl) = bottles.get(&id) {
            // every explicit pour replaces the whole override set — a plain
            // `pour` returns to defaults
            *ctl.overrides.lock().unwrap() = RouteOverrides { port, script };
            cap_bottle(ctl);
        }
    }
    spawn_route(&app, &milk, id);
}

#[tauri::command]
fn remove_bottle(milk: State<Milk>, id: u32) {
    let mut bottles = milk.bottles.lock().unwrap();
    if let Some(ctl) = bottles.remove(&id) {
        cap_bottle(&ctl);
        milk.logs.lock().unwrap().remove(&id);
        let _ = std::fs::remove_file(milk.log_path(&ctl.path));
        let mut led = milk.ledger.lock().unwrap();
        led.remove(&ctl.path);
        led.save(&milk.ledger_file);
    }
}

#[tauri::command]
fn restore_crate(milk: State<Milk>) -> Vec<BottleInfo> {
    let entries: Vec<ledger::Entry> = milk.ledger.lock().unwrap().entries.clone();
    let mut out = Vec::new();
    let mut bottles = milk.bottles.lock().unwrap();
    for entry in entries {
        if !entry.path.is_dir() {
            continue;
        }
        if bottles.values().any(|c| c.path == entry.path) {
            continue;
        }
        let id = milk.next.fetch_add(1, Ordering::SeqCst);
        bottles.insert(id, Ctl::new(entry.path.clone(), entry.name.clone()));
        out.push(BottleInfo {
            id,
            name: entry.name,
            path: entry.path.to_string_lossy().into_owned(),
            state: "capped".into(),
            glyph: Some(glyph_for(&entry.stack).into()),
            stack: Some(entry.stack),
            manager: Some(entry.manager),
            framework: entry.framework,
            last_url: entry.last_url,
            restored: true,
        });
    }
    out
}

// ------------------------------------------------------------ cli commands

/// Replay a bottle's persisted last log (lines only, capped).
#[tauri::command]
fn bottle_log(milk: State<Milk>, id: u32) -> Vec<LogLine> {
    let path = {
        let bottles = milk.bottles.lock().unwrap();
        let Some(ctl) = bottles.get(&id) else { return vec![] };
        milk.log_path(&ctl.path)
    };
    let Ok(text) = std::fs::read_to_string(&path) else { return vec![] };
    let mut lines: Vec<LogLine> = text
        .lines()
        .map(|l| {
            let (kind, text) = l.split_once('\t').unwrap_or(("raw", l));
            LogLine { kind: kind.into(), text: text.into() }
        })
        .collect();
    let cap = 2000;
    if lines.len() > cap {
        lines.drain(..lines.len() - cap);
    }
    lines
}

/// CLI `env` — create .env from .env.example so CHILL stops warning.
#[tauri::command]
fn bootstrap_env(milk: State<Milk>, id: u32) -> Result<String, String> {
    let root = {
        let bottles = milk.bottles.lock().unwrap();
        let Some(ctl) = bottles.get(&id) else { return Err("no such bottle".into()) };
        ctl.path.clone()
    };
    if root.join(".env").exists() {
        return Err("a .env already exists \u{2500} nothing to do".into());
    }
    for cand in [".env.example", ".env.sample"] {
        let src = root.join(cand);
        if src.exists() {
            std::fs::copy(&src, root.join(".env")).map_err(|e| e.to_string())?;
            return Ok(format!(".env created from {cand}"));
        }
    }
    Err("no .env.example or .env.sample in this package".into())
}

/// CLI `watch` — re-pour the bottle whenever files in the package change.
#[tauri::command]
fn watch_bottle(app: AppHandle, milk: State<Milk>, id: u32, on: bool) -> Result<(), String> {
    use notify::Watcher;

    let bottles = milk.bottles.lock().unwrap();
    let Some(ctl) = bottles.get(&id) else { return Err("no such bottle".into()) };

    if !on {
        let had = ctl.watcher.lock().unwrap().take().is_some();
        drop(bottles);
        if had {
            emit_line(&app, id, "note", " watch off");
        }
        return Ok(());
    }

    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(ev) = res {
            // ignore churn from installs, builds, and VCS internals
            const NOISE: [&str; 6] = ["node_modules", ".git", "dist", "build", "target", ".venv"];
            let relevant = ev.paths.iter().any(|p| {
                let s = p.to_string_lossy().replace('\\', "/");
                !NOISE
                    .iter()
                    .any(|n| s.contains(&format!("/{n}/")) || s.ends_with(&format!("/{n}")))
            });
            if relevant {
                let _ = tx.send(());
            }
        }
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(&ctl.path, notify::RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    *ctl.watcher.lock().unwrap() = Some(watcher);
    drop(bottles);
    emit_line(&app, id, "note", " watch on \u{2500} the route re-pours when package files change");

    let app2 = app.clone();
    std::thread::spawn(move || {
        // the sender lives inside the watcher; when the watcher is dropped
        // (watch off / bottle removed) recv fails and this thread ends
        while rx.recv().is_ok() {
            // debounce: wait for 700ms of quiet
            while rx.recv_timeout(Duration::from_millis(700)).is_ok() {}
            let milk = app2.state::<Milk>();
            {
                let bottles = milk.bottles.lock().unwrap();
                let Some(ctl) = bottles.get(&id) else { break };
                cap_bottle(ctl);
            }
            emit_line(&app2, id, "note", " \u{21ba} change detected \u{2500} re-pouring");
            spawn_route(&app2, &milk, id);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steering_skips_tried_common_ports() {
        let profile = resolver::StackProfile {
            stack: resolver::Stack::Node,
            manager: "npm".into(),
            framework: Some("vite".into()),
            install_cmd: None,
            run_cmd: vec!["npm".into(), "run".into(), "dev".into()],
            default_port: Some(5173),
            port_steer: resolver::PortSteer::ScriptFlag("--port"),
            confidence: resolver::Confidence::Exact,
            warnings: vec![],
            scripts: vec![],
            hash_source: None,
            artifact_dir: None,
        };
        let tried = vec![COMMON_PORTS[0], COMMON_PORTS[1]];
        let (cmd, env, port) = steer_to_free_port(&profile, &tried).expect("a free common port");
        assert!(!tried.contains(&port), "picked an already-tried port {port}");
        assert!(
            cmd.ends_with(&["--".into(), "--port".into(), port.to_string()]),
            "unexpected steered cmd: {cmd:?}"
        );
        assert!(env.is_empty());
    }
}

// ---------------------------------------------------------------- entry

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("milkman"));
            app.manage(Milk::new(data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_project,
            stop_bottle,
            rerun_bottle,
            remove_bottle,
            restore_crate,
            bottle_log,
            bootstrap_env,
            watch_bottle
        ])
        .run(tauri::generate_context!())
        .expect("error while running MILKMAN");
}
