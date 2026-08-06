# MILKMAN v0.0.1

*Delivering fresh, cold and ready to drink packages.*

Drag a project folder — or a `.zip` straight out of claude.ai — onto the
window. MILKMAN inspects it, installs its dependencies, runs it, and hands
you the URL. Four stages, one route:

```ps1
[1/4] INSPECT  — reading the label
[2/4] BOTTLE   — filling dependencies
[3/4] CHILL    — checking the icebox
[4/4] DELIVER  — on the doorstep
```

## Prerequisites

- **Rust** (stable) — <https://rustup.rs>
- **Node 18+** with npm
- **Tauri 2 system deps** — on Windows: WebView2 (preinstalled on Win 11).
  Linux/macOS: see <https://tauri.app/start/prerequisites/>

Whatever stacks you want MILKMAN to pour must themselves be on PATH
(`npm`/`pnpm`/`bun`, `uv`/`python`, `cargo`, `go`…). Missing managers are
substituted with a visible `[SUBSTITUTED]` warning, never silently.

## Run

```ps1
npm install
npm run tauri dev
```

Build an installer:

```ps1
npm run tauri build
```

## What it detects

| Evidence                                                           | Verdict                          |
|--------------------------------------------------------------------|----------------------------------|
| `bun.lockb` / `pnpm-lock.yaml` / `yarn.lock` / `package-lock.json` | Node, exact manager              |
| `package.json` only                                                | Node, npm inferred               |
| `uv.lock` / `pyproject.toml` / `requirements.txt`                  | Python (uv preferred)            |
| `manage.py`                                                        | Django                           |
| `FastAPI(` / `Flask(` in source                                    | uvicorn / flask run              |
| `Cargo.toml`                                                       | `cargo run`                      |
| `go.mod`                                                           | `go run .`                       |
| `index.html` alone                                                 | MILKMAN's built-in static server |

Lockfiles outrank manifests. Confidence is shown as `[EXACT]`,
`[INFERRED]`, or `[GUESSED]` — a guess is honest about being one.

## Controls

- **Drop** a folder or `.zip` anywhere — new bottle in the crate
- **Re-drop** the same folder — re-runs it (unchanged lockfile skips install)
- **L** — toggle the full raw log (the terminal folds output past 200 lines)
- **Click a capped bottle** — pours it again
- **× on a bottle** — removes it and forgets it
- **STOP / OPEN / RE-POUR** — on the delivery slip

## Design notes

The terminal is a custom renderer, not xterm.js: MILKMAN's output is a
narrated route, with raw process output demoted to a dimmed, left-ruled
inset. Palette is COLD DARK (`#0c0c10` void / cream text / cold blue),
type is Departure Mono over Space Grotesk. Fonts are bundled; Departure
Mono's OFL license sits next to it in `public/fonts/`.

## Known limits (v0.0.1)

- **Process cleanup** uses `taskkill /T /F` on Windows and process-group
  signals on Unix. Win32 Job Objects are planned for v0.0.2.
- **PEEK** (embedded preview webview) is deferred to v0.0.2 — OPEN uses
  your default browser.
- On macOS, apps launched from Finder inherit a minimal PATH; if managers
  aren't found, launch from a terminal (`npm run tauri dev`) or install
  managers system-wide.
- Stopping a bottle mid-install kills the installer; a re-pour resumes
  from a clean slate.
- The `uv venv && uv pip install` fallback for bare `requirements.txt`
  relies on the shell joining `&&` — cosmetic in the log, functional in
  practice.

---

MIVIM · com.mivim.milkman · v0.0.1
