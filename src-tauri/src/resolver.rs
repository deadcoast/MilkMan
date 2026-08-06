//! The cascade: evidence in, StackProfile out.
//! Lockfiles outrank manifests. Substitutions are visible, never silent.

use crate::scanner::{Evidence, PyKind};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Stack {
    Node,
    Python,
    Rust,
    Go,
    Ruby,
    Static,
}

impl Stack {
    pub fn label(&self) -> &'static str {
        match self {
            Stack::Node => "NODE",
            Stack::Python => "PYTHON",
            Stack::Rust => "RUST",
            Stack::Go => "GO",
            Stack::Ruby => "RUBY",
            Stack::Static => "STATIC",
        }
    }
    pub fn glyph(&self) -> &'static str {
        match self {
            Stack::Node => "N",
            Stack::Python => "PY",
            Stack::Rust => "RS",
            Stack::Go => "GO",
            Stack::Ruby => "RB",
            Stack::Static => "ST",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Confidence {
    Exact,
    Inferred,
    Guessed,
}

impl Confidence {
    pub fn label(&self) -> &'static str {
        match self {
            Confidence::Exact => "EXACT",
            Confidence::Inferred => "INFERRED",
            Confidence::Guessed => "GUESSED",
        }
    }
}

/// How a stack's dev server can be moved to a different port when its
/// preferred one is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PortSteer {
    /// no reliable way to choose the port — don't retry
    Fixed,
    /// append `-- <flag> <port>` to the manager script invocation
    /// (vite/astro/sveltekit/nuxt `--port`, next `-p`)
    ScriptFlag(&'static str),
    /// append `<flag> <port>` directly to the run command
    /// (uvicorn/flask `--port`, sinatra `-p`)
    ArgFlag(&'static str),
    /// append `127.0.0.1:<port>` (django runserver)
    BindAddr,
    /// set `PORT=<port>` in the environment (CRA, generic node, go)
    PortEnv,
}

#[derive(Debug, Clone, Serialize)]
pub struct StackProfile {
    pub stack: Stack,
    pub manager: String,
    pub framework: Option<String>,
    /// None = nothing to install (static sites)
    pub install_cmd: Option<Vec<String>>,
    /// Empty = MILKMAN's built-in static server
    pub run_cmd: Vec<String>,
    pub default_port: Option<u16>,
    pub port_steer: PortSteer,
    pub confidence: Confidence,
    pub warnings: Vec<String>,
    /// file whose hash decides whether BOTTLE can be skipped on re-runs
    pub hash_source: Option<PathBuf>,
    /// directory whose presence means deps are already installed
    pub artifact_dir: Option<&'static str>,
}

impl StackProfile {
    /// Rebuild the run command (and any extra env) pinned to `port`.
    /// None when this stack cannot be steered to another port.
    pub fn run_with_port(&self, port: u16) -> Option<(Vec<String>, Vec<(String, String)>)> {
        if self.run_cmd.is_empty() {
            return None; // built-in static server picks its own free port
        }
        let p = port.to_string();
        match self.port_steer {
            PortSteer::Fixed => None,
            PortSteer::ScriptFlag(flag) => {
                let mut cmd = self.run_cmd.clone();
                cmd.extend(["--".into(), flag.into(), p]);
                Some((cmd, vec![]))
            }
            PortSteer::ArgFlag(flag) => {
                let mut cmd = self.run_cmd.clone();
                cmd.extend([flag.into(), p]);
                Some((cmd, vec![]))
            }
            PortSteer::BindAddr => {
                let mut cmd = self.run_cmd.clone();
                cmd.push(format!("127.0.0.1:{p}"));
                Some((cmd, vec![]))
            }
            PortSteer::PortEnv => Some((self.run_cmd.clone(), vec![("PORT".into(), p)])),
        }
    }
}

fn available(bin: &str) -> bool {
    which::which(bin).is_ok()
}

/// Resolve the whole cascade. Returns None only for a directory with no
/// recognizable evidence at all.
pub fn resolve(ev: &Evidence) -> Option<StackProfile> {
    // ---- NODE ----------------------------------------------------------
    if ev.package_json.is_some() || ev.lock_bun || ev.lock_pnpm || ev.lock_yarn || ev.lock_npm {
        return Some(resolve_node(ev));
    }
    // ---- PYTHON --------------------------------------------------------
    if ev.uv_lock || ev.pyproject || ev.requirements || ev.manage_py {
        return Some(resolve_python(ev));
    }
    // ---- RUST ----------------------------------------------------------
    if ev.cargo_toml {
        let mut warnings = vec![];
        if !available("cargo") {
            warnings.push("cargo not found on PATH".into());
        }
        return Some(StackProfile {
            stack: Stack::Rust,
            manager: "cargo".into(),
            framework: None,
            install_cmd: Some(vec!["cargo".into(), "build".into()]),
            run_cmd: vec!["cargo".into(), "run".into()],
            default_port: None,
            port_steer: PortSteer::PortEnv,
            confidence: Confidence::Exact,
            warnings,
            hash_source: Some(ev.root.join("Cargo.lock")).filter(|p| p.exists())
                .or(Some(ev.root.join("Cargo.toml"))),
            artifact_dir: Some("target"),
        });
    }
    // ---- GO ------------------------------------------------------------
    if ev.go_mod {
        return Some(StackProfile {
            stack: Stack::Go,
            manager: "go".into(),
            framework: None,
            install_cmd: Some(vec!["go".into(), "mod".into(), "download".into()]),
            run_cmd: vec!["go".into(), "run".into(), ".".into()],
            default_port: None,
            port_steer: PortSteer::PortEnv,
            confidence: Confidence::Inferred,
            warnings: if available("go") { vec![] } else { vec!["go not found on PATH".into()] },
            hash_source: Some(ev.root.join("go.sum")).filter(|p| p.exists())
                .or(Some(ev.root.join("go.mod"))),
            artifact_dir: None,
        });
    }
    // ---- RUBY ----------------------------------------------------------
    if ev.gemfile {
        return Some(StackProfile {
            stack: Stack::Ruby,
            manager: "bundler".into(),
            framework: None,
            install_cmd: Some(vec!["bundle".into(), "install".into()]),
            run_cmd: vec!["bundle".into(), "exec".into(), "ruby".into(), "app.rb".into()],
            default_port: Some(4567),
            port_steer: PortSteer::ArgFlag("-p"),
            confidence: Confidence::Guessed,
            warnings: vec!["ruby entrypoint guessed as app.rb".into()],
            hash_source: Some(ev.root.join("Gemfile.lock")).filter(|p| p.exists())
                .or(Some(ev.root.join("Gemfile"))),
            artifact_dir: None,
        });
    }
    // ---- STATIC --------------------------------------------------------
    if ev.index_html {
        return Some(StackProfile {
            stack: Stack::Static,
            manager: "builtin".into(),
            framework: None,
            install_cmd: None,
            run_cmd: vec![], // MILKMAN's own static server
            default_port: None,
            port_steer: PortSteer::Fixed, // the icebox picks a free port itself
            confidence: Confidence::Exact,
            warnings: vec![],
            hash_source: None,
            artifact_dir: None,
        });
    }
    None
}

fn resolve_node(ev: &Evidence) -> StackProfile {
    let mut warnings: Vec<String> = Vec::new();

    // lockfile priority, with availability fall-through
    let wanted: &str = if ev.lock_bun {
        "bun"
    } else if ev.lock_pnpm {
        "pnpm"
    } else if ev.lock_yarn {
        "yarn"
    } else {
        "npm"
    };
    let confidence = if ev.lock_bun || ev.lock_pnpm || ev.lock_yarn || ev.lock_npm {
        Confidence::Exact
    } else {
        Confidence::Inferred
    };
    let fallbacks = ["bun", "pnpm", "yarn", "npm"];
    let manager = if available(wanted) {
        wanted.to_string()
    } else {
        let sub = fallbacks.iter().find(|m| available(m)).unwrap_or(&"npm");
        warnings.push(format!("[SUBSTITUTED] {wanted} not on PATH — using {sub}"));
        sub.to_string()
    };

    // framework + script
    let pj = ev.package_json.clone().unwrap_or_default();
    let dep = |name: &str| pj.deps.iter().any(|d| d == name);
    let (framework, default_port, port_steer) = if dep("next") {
        (Some("next".into()), Some(3000), PortSteer::ScriptFlag("-p"))
    } else if dep("@sveltejs/kit") {
        (Some("sveltekit".into()), Some(5173), PortSteer::ScriptFlag("--port"))
    } else if dep("astro") {
        (Some("astro".into()), Some(4321), PortSteer::ScriptFlag("--port"))
    } else if dep("nuxt") {
        (Some("nuxt".into()), Some(3000), PortSteer::ScriptFlag("--port"))
    } else if dep("react-scripts") {
        (Some("cra".into()), Some(3000), PortSteer::PortEnv)
    } else if dep("vite") {
        (Some("vite".into()), Some(5173), PortSteer::ScriptFlag("--port"))
    } else {
        (None, None, PortSteer::PortEnv)
    };

    let script = ["dev", "start", "serve", "preview"]
        .iter()
        .find(|s| pj.scripts.iter().any(|have| have == *s))
        .map(|s| s.to_string());

    let run_cmd: Vec<String> = match &script {
        Some(s) => vec![manager.clone(), "run".into(), s.clone()],
        None => {
            warnings.push("no dev/start script in package.json".into());
            if ev.index_html {
                warnings.push("falling back to built-in static server".into());
                vec![]
            } else {
                vec![manager.clone(), "start".into()]
            }
        }
    };

    let hash_source = [
        "bun.lockb", "bun.lock", "pnpm-lock.yaml", "yarn.lock", "package-lock.json",
    ]
    .iter()
    .map(|n| ev.root.join(n))
    .find(|p| p.exists())
    .or(Some(ev.root.join("package.json")));

    StackProfile {
        stack: if run_cmd.is_empty() { Stack::Static } else { Stack::Node },
        manager: manager.clone(),
        framework,
        install_cmd: Some(vec![manager, "install".into()]),
        run_cmd,
        default_port,
        port_steer,
        confidence,
        warnings,
        hash_source,
        artifact_dir: Some("node_modules"),
    }
}

fn resolve_python(ev: &Evidence) -> StackProfile {
    let mut warnings: Vec<String> = Vec::new();
    let use_uv = available("uv");
    if !use_uv {
        warnings.push("[SUBSTITUTED] uv not on PATH — using pip/python".into());
    }
    let manager = if use_uv { "uv" } else { "pip" }.to_string();

    // install
    let install_cmd = if use_uv {
        if ev.uv_lock || ev.pyproject {
            Some(vec!["uv".into(), "sync".into()])
        } else {
            Some(vec![
                "uv".into(), "venv".into(), "&&".into(),
                "uv".into(), "pip".into(), "install".into(), "-r".into(), "requirements.txt".into(),
            ])
        }
    } else if ev.requirements {
        Some(vec![
            "python".into(), "-m".into(), "pip".into(), "install".into(),
            "-r".into(), "requirements.txt".into(),
        ])
    } else if ev.pyproject {
        Some(vec![
            "python".into(), "-m".into(), "pip".into(), "install".into(), "-e".into(), ".".into(),
        ])
    } else {
        None
    };

    // run — prefix every command with the manager's runner so the venv is used
    let prefix: Vec<String> = if use_uv {
        vec!["uv".into(), "run".into()]
    } else {
        vec![]
    };
    let with_prefix = |tail: Vec<&str>| -> Vec<String> {
        prefix
            .iter()
            .cloned()
            .chain(tail.into_iter().map(String::from))
            .collect()
    };

    let mut confidence = Confidence::Inferred;
    let mut framework: Option<String> = None;
    let mut default_port: Option<u16> = None;
    let mut port_steer = PortSteer::Fixed;

    let run_cmd: Vec<String> = if ev.manage_py {
        framework = Some("django".into());
        default_port = Some(8000);
        port_steer = PortSteer::BindAddr;
        with_prefix(vec!["python", "manage.py", "runserver"])
    } else if let Some(entry) = &ev.py_entry {
        match entry.kind {
            PyKind::FastApi => {
                framework = Some("fastapi".into());
                default_port = Some(8000);
                port_steer = PortSteer::ArgFlag("--port");
                let target = format!("{}:app", entry.module);
                with_prefix(vec!["uvicorn", &target, "--reload"])
            }
            PyKind::Flask => {
                framework = Some("flask".into());
                default_port = Some(5000);
                port_steer = PortSteer::ArgFlag("--port");
                with_prefix(vec!["python", "-m", "flask", "--app", &entry.file, "run"])
            }
            PyKind::Plain => {
                confidence = Confidence::Guessed;
                warnings.push(format!("entrypoint guessed as {}", entry.file));
                with_prefix(vec!["python", &entry.file])
            }
        }
    } else {
        confidence = Confidence::Guessed;
        warnings.push("no python entrypoint found — guessing main.py".into());
        with_prefix(vec!["python", "main.py"])
    };

    if ev.uv_lock {
        confidence = Confidence::Exact.min_with(confidence);
    }

    StackProfile {
        stack: Stack::Python,
        manager,
        framework,
        install_cmd,
        run_cmd,
        default_port,
        port_steer,
        confidence,
        warnings,
        hash_source: [
            "uv.lock", "requirements.txt", "pyproject.toml",
        ]
        .iter()
        .map(|n| ev.root.join(n))
        .find(|p| p.exists()),
        artifact_dir: Some(".venv"),
    }
}

impl Confidence {
    /// keep the weaker of two signals (Exact lockfile + Guessed entrypoint
    /// is still a guess where it counts)
    fn min_with(self, other: Confidence) -> Confidence {
        use Confidence::*;
        match (self, other) {
            (Guessed, _) | (_, Guessed) => Guessed,
            (Inferred, _) | (_, Inferred) => Inferred,
            _ => Exact,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::PackageJson;

    fn node_ev(deps: &[&str], scripts: &[&str]) -> Evidence {
        Evidence {
            root: std::env::temp_dir(),
            package_json: Some(PackageJson {
                scripts: scripts.iter().map(|s| s.to_string()).collect(),
                deps: deps.iter().map(|s| s.to_string()).collect(),
            }),
            lock_npm: true,
            ..Default::default()
        }
    }

    #[test]
    fn vite_steers_with_script_flag() {
        let p = resolve(&node_ev(&["vite"], &["dev"])).expect("node profile");
        let (cmd, env) = p.run_with_port(4001).expect("steerable");
        assert!(
            cmd.ends_with(&["--".into(), "--port".into(), "4001".into()]),
            "unexpected retry cmd: {cmd:?}"
        );
        assert!(env.is_empty());
    }

    #[test]
    fn cra_steers_with_port_env() {
        let p = resolve(&node_ev(&["react-scripts"], &["start"])).expect("node profile");
        let (cmd, env) = p.run_with_port(4001).expect("steerable");
        assert_eq!(cmd, p.run_cmd, "env steering must not alter the command");
        assert_eq!(env, vec![("PORT".to_string(), "4001".to_string())]);
    }

    #[test]
    fn django_steers_with_bind_addr() {
        let ev = Evidence {
            root: std::env::temp_dir(),
            manage_py: true,
            requirements: true,
            ..Default::default()
        };
        let p = resolve(&ev).expect("python profile");
        let (cmd, _) = p.run_with_port(4001).expect("steerable");
        assert_eq!(cmd.last().map(String::as_str), Some("127.0.0.1:4001"));
    }

    #[test]
    fn static_and_plain_python_are_fixed() {
        let ev = Evidence {
            root: std::env::temp_dir(),
            index_html: true,
            ..Default::default()
        };
        let p = resolve(&ev).expect("static profile");
        assert!(p.run_with_port(4001).is_none(), "icebox needs no steering");
    }
}
