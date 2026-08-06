//! INSPECT — walk the dropped directory and collect evidence of what it is.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PyKind {
    FastApi,
    Flask,
    Plain,
}

#[derive(Debug, Clone, Serialize)]
pub struct PyEntry {
    /// file path relative to root, forward slashes
    pub file: String,
    /// python module path, e.g. `src.main`
    pub module: String,
    pub kind: PyKind,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PackageJson {
    pub scripts: Vec<String>,
    pub deps: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Evidence {
    pub root: PathBuf,
    pub package_json: Option<PackageJson>,
    pub lock_bun: bool,
    pub lock_pnpm: bool,
    pub lock_yarn: bool,
    pub lock_npm: bool,
    pub uv_lock: bool,
    pub pyproject: bool,
    pub requirements: bool,
    pub cargo_toml: bool,
    pub go_mod: bool,
    pub gemfile: bool,
    pub index_html: bool,
    pub manage_py: bool,
    pub py_entry: Option<PyEntry>,
    pub env_example: bool,
    pub env_file: bool,
    /// display list of notable files found, in discovery order
    pub found: Vec<String>,
}

fn exists(root: &Path, name: &str, found: &mut Vec<String>) -> bool {
    let hit = root.join(name).exists();
    if hit {
        found.push(name.to_string());
    }
    hit
}

pub fn scan(root: &Path) -> Evidence {
    let mut ev = Evidence {
        root: root.to_path_buf(),
        ..Default::default()
    };
    let f = &mut ev.found;

    // Node family
    let has_pkg = exists(root, "package.json", f);
    ev.lock_bun = exists(root, "bun.lockb", f) || exists(root, "bun.lock", f);
    ev.lock_pnpm = exists(root, "pnpm-lock.yaml", f);
    ev.lock_yarn = exists(root, "yarn.lock", f);
    ev.lock_npm = exists(root, "package-lock.json", f);

    // Python family
    ev.uv_lock = exists(root, "uv.lock", f);
    ev.pyproject = exists(root, "pyproject.toml", f);
    ev.requirements = exists(root, "requirements.txt", f);
    ev.manage_py = exists(root, "manage.py", f);

    // Others
    ev.cargo_toml = exists(root, "Cargo.toml", f);
    ev.go_mod = exists(root, "go.mod", f);
    ev.gemfile = exists(root, "Gemfile", f);
    ev.index_html = exists(root, "index.html", f);

    // env hygiene
    ev.env_example = root.join(".env.example").exists() || root.join(".env.sample").exists();
    ev.env_file = root.join(".env").exists();

    if has_pkg {
        ev.package_json = read_package_json(&root.join("package.json"));
    }

    if ev.pyproject || ev.requirements || ev.uv_lock || ev.manage_py {
        ev.py_entry = find_py_entry(root);
    }

    ev
}

fn read_package_json(path: &Path) -> Option<PackageJson> {
    let text = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut pj = PackageJson::default();
    if let Some(obj) = v.get("scripts").and_then(|s| s.as_object()) {
        pj.scripts = obj.keys().cloned().collect();
    }
    for key in ["dependencies", "devDependencies"] {
        if let Some(obj) = v.get(key).and_then(|s| s.as_object()) {
            pj.deps.extend(obj.keys().cloned());
        }
    }
    Some(pj)
}

/// Look for a runnable python entrypoint at root and in src/, preferring
/// main.py / app.py, and classify it by framework markers in the source.
fn find_py_entry(root: &Path) -> Option<PyEntry> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in [root.to_path_buf(), root.join("src"), root.join("app")] {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "py").unwrap_or(false) {
                candidates.push(p);
            }
        }
    }
    // preference order: main.py, app.py, server.py, then anything else
    let rank = |p: &Path| -> u8 {
        match p.file_name().and_then(|n| n.to_str()).unwrap_or("") {
            "main.py" => 0,
            "app.py" => 1,
            "server.py" => 2,
            _ => 9,
        }
    };
    candidates.sort_by_key(|p| rank(p));

    // first framework-marked file wins; otherwise first ranked candidate
    let mut fallback: Option<PathBuf> = None;
    for p in &candidates {
        let Ok(src) = fs::read_to_string(p) else { continue };
        let kind = if src.contains("FastAPI(") {
            Some(PyKind::FastApi)
        } else if src.contains("Flask(") {
            Some(PyKind::Flask)
        } else {
            None
        };
        if let Some(kind) = kind {
            return Some(make_entry(root, p, kind));
        }
        if fallback.is_none() {
            fallback = Some(p.clone());
        }
    }
    fallback.map(|p| make_entry(root, &p, PyKind::Plain))
}

fn make_entry(root: &Path, p: &Path, kind: PyKind) -> PyEntry {
    let rel = p.strip_prefix(root).unwrap_or(p);
    let file = rel.to_string_lossy().replace('\\', "/");
    let module = file.trim_end_matches(".py").replace('/', ".");
    PyEntry { file, module, kind }
}
