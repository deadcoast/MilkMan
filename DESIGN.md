# MILKMAN — DESIGN SPECIFICATION

> DELIVERING FRESH, COLD AND READY TO DRINK PACKAGES

A desktop runner for AI-packaged web projects. Drag a directory in; MILKMAN detects the stack, installs dependencies, and runs the app — every step rendered in a stylized DOS-terminal window.

---

## 1. ARCHITECTURE

**Stack: Tauri 2 (Rust core) + Svelte 5 frontend.**

Rationale: the detection/process engine belongs in Rust (filesystem scanning, PTY management, process supervision), while the DOS-terminal aesthetic and drag-drop UI are dramatically faster to build and iterate in web tech. Pure-Rust GUI (egui/iced) would fight you on typography and the retro styling you want.

```ps1
┌─────────────────────────────────────────────────┐
│                 SVELTE FRONTEND                 │
│   DropZone · Crate (project list) · Terminal    │
│   RouteTracker (stage UI) · DeliverySlip        │
└───────────────▲──────────────┬──────────────────┘
                │ Tauri events │ invoke()
┌───────────────┴──────────────▼──────────────────┐
│                   RUST CORE                     │
│  scanner    → walks dir, builds StackProfile    │
│  resolver   → picks package manager + commands  │
│  dairy      → PTY process supervisor            │
│  porter     → sniffs stdout for localhost URLs  │
│  ledger     → per-project state + history       │
└─────────────────────────────────────────────────┘
```

### Core crates

| Concern | Crate |
| --- | --- |
| PTY (live, colored output) | `portable-pty` |
| Dir walking | `walkdir` + `ignore` (respects .gitignore) |
| Manifest parsing | `serde_json`, `toml` |
| ANSI passthrough to frontend | raw bytes → `xterm.js`-free custom renderer (see §5) |
| Process cleanup (Windows) | Job Objects via `win32job`; Unix: process groups + `kill_on_drop` |

**Windows orphan prevention is non-negotiable.** `npm run dev` spawns children (node → vite → esbuild). Killing only the parent leaves zombie servers holding ports. Every spawned process is assigned to a Job Object (Windows) / process group (Unix) so closing a bottle kills the whole tree.

---

## 2. DETECTION ENGINE ("INSPECT")

Detection runs in a strict priority cascade. Lockfiles outrank manifests — they name the *exact* package manager the AI (or user) used.

### 2.1 Package manager resolution

| Evidence (highest priority first) | Manager | Install |
| --- | --- | --- |
| `bun.lockb` / `bun.lock` | bun | `bun install` |
| `pnpm-lock.yaml` | pnpm | `pnpm install` |
| `yarn.lock` | yarn | `yarn install` |
| `package-lock.json` or bare `package.json` | npm | `npm install` |
| `uv.lock` | uv | `uv sync` |
| `pyproject.toml` (no uv.lock) | uv | `uv sync` (fallback `pip install -e .`) |
| `requirements.txt` only | uv | `uv pip install -r requirements.txt` (fallback pip) |
| `Cargo.toml` | cargo | `cargo build` |
| `go.mod` | go | `go mod download` |
| `Gemfile` | bundler | `bundle install` |
| `index.html` only, no manifest | static | *(skip install)* |

Managers are verified against PATH at startup (`which`/`where`); unavailable managers fall down the cascade with a visible `[SUBSTITUTED]` line in the terminal — never a silent swap.

### 2.2 Run command resolution

**Node:** read `package.json` scripts in priority order `dev` → `start` → `serve` → `preview`. Framework badge derived from deps (`vite`, `next`, `@sveltejs/kit`, `astro`, `react-scripts`, `nuxt`).

**Python:** heuristic chain — `manage.py` → `python manage.py runserver`; FastAPI/uvicorn in deps → `uvicorn <module>:app --reload` (module found by scanning for `app = FastAPI()`); Flask → `flask run`; else `python main.py`/`app.py`.

**Static:** MILKMAN's built-in static server (axum, random free port) — this is the killer path for single-file HTML artifacts dragged straight out of claude.ai.

**Rust:** `cargo run`.

**Ambiguity rule:** if two stacks coexist (e.g. `package.json` + `pyproject.toml`), MILKMAN does **not** guess. It presents both as separate runnable "bottles" from one crate slot and runs whichever the user taps — or both.

### 2.3 StackProfile (the scan result)

```rust
struct StackProfile {
    stack: Stack,            // Node, Python, Rust, Static, ...
    manager: Manager,        // npm, pnpm, uv, cargo, builtin
    framework: Option<String>,
    install_cmd: Vec<String>,
    run_cmd: Vec<String>,
    confidence: Confidence,  // Exact (lockfile) | Inferred | Guessed
    warnings: Vec<String>,   // engine mismatches, missing .env, etc.
}
```

Confidence is shown to the user (§5) — `EXACT`, `INFERRED`, `GUESSED` — so a wrong guess is never a surprise.

---

## 3. THE ROUTE (pipeline stages)

Every project moves through four themed stages. Plain-English subtitles keep the metaphor legible.

| # | Stage | Meaning | Terminal header |
| --- | --- | --- | --- |
| 1 | **INSPECT** | scan + detect | `[1/4] INSPECT ─ reading the label` |
| 2 | **BOTTLE** | install deps | `[2/4] BOTTLE ─ filling dependencies` |
| 3 | **CHILL** | pre-run checks: free-port probe, env warnings | `[3/4] CHILL ─ checking the icebox` |
| 4 | **DELIVER** | run + serve URL | `[4/4] DELIVER ─ on the doorstep` |

Failure states use the theme once, plainly: a failed stage stamps **SPOILED** with the real error directly beneath it. No cutesy error messages beyond the stamp — errors must stay actionable.

Re-drop of an already-known directory skips straight to a diff-aware BOTTLE (lockfile hash comparison → "no change, skipping install") — this is the AI edit-loop payoff: re-drag, instantly re-DELIVERed.

---

## 4. UI LAYOUT

Minimal chrome. Two-region layout, native UI framing the terminal.

```txt
┌─ MILKMAN ─────────────────────────────── - o x ┐
│ ┌───────┐                                      │
│ │ CRATE │  ╔═[ MILKMAN v1.0 ]═══════════════╗  │
│ │       │  ║                                ║  │
│ │ [B1]  │  ║  [1/4] INSPECT ─ reading...    ║  │
│ │ [B2]  │  ║   ├─ package.json      FOUND   ║  │
│ │ [B3]  │  ║   ├─ pnpm-lock.yaml    FOUND   ║  │
│ │  +    │  ║   └─ stack: NODE/pnpm  EXACT   ║  │
│ │       │  ║                                ║  │
│ └───────┘  ╚════════════════════════════════╝  │
│  STATUS ▸ NODE·pnpm·vite  PORT 5173   00:04    │
└────────────────────────────────────────────────┘
```

- **DropZone / Crate (left rail, 72px):** empty state is a full-window drop target with a dashed bottle outline and `DROP A PACKAGE`. Loaded projects become **bottles** in the crate — vertical stack of square slots, each with a stack glyph and a state ring (running = cold-blue pulse, spoiled = red, delivered = green). Multiple projects run concurrently; clicking a bottle switches the terminal view.
- **Terminal (main region):** the DOS window, §5.
- **Status strip (bottom, 28px):** detected stack badges, active port, elapsed time, confidence tag. Native-UI shade.
- **Delivery slip:** on successful DELIVER, a slim card slides over the terminal's bottom edge: `● FRESH — http://localhost:5173` with `OPEN` (system browser) and `PEEK` (embedded Tauri webview panel, splitting the window 50/50 with the terminal). PEEK is what makes MILKMAN a true artifact-runner rather than a launcher.

Drag-in of a `.zip` (how claude.ai exports) is first-class: auto-extract to a managed workspace dir, then treat as a folder drop.

---

## 5. VISUAL DESIGN SYSTEM

### 5.1 Palette — "COLD DARK"

One dark family; the terminal sits one step *deeper* than the native chrome so the two read as materials, not clashing colors.

| Token | Hex | Use |
| --- | --- | --- |
| `--void` | `#0C0C10` | terminal background (deepest) |
| `--chrome` | `#17171D` | native UI background |
| `--raised` | `#212129` | crate slots, status strip, cards |
| `--line` | `#2E2E38` | borders, box-drawing dividers in native UI |
| `--cream` | `#F2EAD9` | primary terminal text — *milk* |
| `--skim` | `#8D8A80` | secondary/dim terminal text |
| `--cold` | `#6FB7FF` | info, URLs, running state — *cold* |
| `--fresh` | `#7CE38B` | success — *fresh* |
| `--spoiled` | `#FF6B6B` | errors |
| `--butter` | `#FFC66D` | warnings, in-progress spinners |

Rules: `--cream` on `--void` is the terminal's default pair (warm-on-black = the retro read). Native UI text is `--cream` at 90% on `--chrome`. Accents never appear as large fills — only text, rings, and 1px rules. That restraint is what keeps "minimal modern" over "neon retro."

### 5.2 Typography

| Region | Face | Notes |
| --- | --- | --- |
| Terminal | **Departure Mono** | pixel-grid mono, the modern-retro voice; ships bundled |
| Native UI | **Space Grotesk** | headers/labels, tracking +4% uppercase for stage names |
| Fallbacks | IBM Plex Mono / system sans | |

Terminal renders at integer pixel multiples only (11px/22px) — no fractional scaling, so the pixel font stays crisp.

### 5.3 The DOS terminal window

Not xterm.js. A custom Svelte renderer over a line buffer — MILKMAN controls every character, which is the whole point.

- **Frame:** double-line box drawing (`╔ ═ ╗ ║ ╚ ╝`) in `--line`, title inset `═[ MILKMAN v1.0 ]═` in `--cream`.
- **Stage headers:** bracketed counters `[2/4] BOTTLE` in `--butter` while active, `--fresh` when passed; tree-style detail lines (`├─`, `└─`) beneath.
- **Live process output** (npm/uv/etc.) streams into an inset region, dimmed to `--skim`, left-ruled with `│` — *designed containment of raw output*, condensed: collapse runs of progress-bar rewrites into a single updating line; hard-cap the inset to the last 200 lines with `▲ 412 lines folded` above it. Full raw log always one keystroke away (`L`).
- **Cursor:** blinking `▓` block, 530ms.
- **Texture:** 3% scanline overlay + very subtle center vignette on the terminal region only. No curvature, no chromatic aberration — one texture cue is retro; three is cosplay.
- **Spinner:** braille-free, DOS-safe frames `─ \ │ /`. No modern emoji anywhere.

### 5.4 Motion

- Bottle drop-in: 180ms settle with 1-frame squash (milk slosh).
- Stage transitions: header stamps in with a 2-frame typewriter reveal, no fades.
- Delivery slip: 240ms slide-up, ease-out.
- Everything else is instant. Retro machines didn't tween.

---

## 6. STATE MODEL (per bottle)

```ps1
EMPTY → INSPECTING → BOTTLING → CHILLING → DELIVERING → FRESH
                │         │         │           │
                └────► SPOILED ◄────┴───────────┘
FRESH ──(file re-drop / watcher)──► BOTTLING (diff-aware) → FRESH
FRESH ──(stop)──► CAPPED (stopped, restartable)
```

`ledger` persists crate contents, ports, and lockfile hashes across app restarts (JSON in app data dir), so reopening MILKMAN restores the crate cold.

---

## 7. BUILD ORDER

1. Core: scanner + resolver with the §2 cascade, unit-tested against fixture dirs (one per row of the manager table).
2. `dairy` PTY supervisor + Job Object cleanup — prove kill-tree on Windows first.
3. Terminal renderer + palette + Departure Mono, static content.
4. Wire events end-to-end for the Node/npm path only → first full DELIVER.
5. Crate multi-project, delivery slip, PEEK webview.
6. Remaining stacks, zip ingestion, diff-aware re-bottle, ledger persistence.

---
