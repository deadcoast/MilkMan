# TODO

> Tests on the application utilize the file `/test_directory/AC7_FACTION_PACKAGE.zip`.
>> A backup that should never be modified; only duplicated for use is in `/test_directory/backups/AC7_FACTION_PACKAGE-backup.zip`.

## ERRORS

### TERMINAL TITLE WINDOW

THE TERMINAL WINDOW TITLE IS CUT OFF BY ITS OWN UI WINDOW. IT NEEDS TO BE LOWERED. THE DYNAMIC FUNCTION IS WORKING, BECAUSE WHEN I RESIZE THE WINDOW, THE TITLE RESIZES WITH IT; HALWAY INSIDE ITS OWN UI TERMINAL WINDOW. THE ENTIRE TERMINAL WINDOWS TEXT IS CUT OFF AS THE WINDOW TRAILS DURING COMMANDS WHEN THE APPLICATION RUNS.

### HANGS AFTER APPLICATION RAN `npm install`

> app output

```milkman
 stack: NODE/npm  [INFERRED]
[2/4] BOTTLE ─ filling dependencies \
 $ npm install
▓
```

> [!NOTE]
>> `\` is the spinner; its working and animated.
>> `▓` is the blinking cursor; its working and animated.

The application stalls here.

---

## NEEDS A FORCE CANCEL BUTTON

When the application stalls or hangs up there is no way to stop the run or refresh the window. Also accept ctrl+c. BUT WHEN THE BUTTON IS PUSHED, ctrl+c MUST BE PRINTED IN THE WINDOW AS IF IT IS AN ACNOWLEDGEMENT TO THE COMMAND BEING PRESSED AND INTERRUPTING THE RUN. THIS WAY USERS WILL UNDERSTAND THAT `ctrl+c` IS THE HOTKEY FOR THE CANCEL/INTERRUPT BUTTON.

---

## The [+] Button is currently still placeholder

The button on the left of the application below the active terminal button does not open a new milkman terminal window. It should

---

## MIVIM DELIVERY NOTES — 2026-08-06

All four items above are resolved. Verified by 6 backend tests (`cargo test` in `src-tauri`, set `MILKMAN_TEST_PKG` to an extracted duplicate of the AC7 zip) plus `svelte-check`, `vite build`, and a live app screenshot.

- **TERMINAL TITLE**: `.title` straddled the frame's top border while being a child of `.frame { overflow: hidden }`, so its top half was clipped. It now lives in a non-clipping `.terminal` wrapper (`Terminal.svelte`). The title also shows the active bottle's name.
- **HANG AT `npm install`**: two Windows/ConPTY root causes, both fixed in `dairy.rs`:
  1. ConPTY sends VT queries (e.g. cursor-position `ESC[6n`) to the hosting terminal and **blocks the child's console calls until answered** — cmd.exe froze before even launching npm. A `Doorman` now answers these queries.
  2. The PTY reader **never receives EOF while the master handle is alive**, so the route thread stayed blocked even after a process exited. `dairy::serve()` now watches the child (`try_wait`) and drops the master to force the reader loose.
  3. Bonus find: ConPTY ends every line with `\r\n`; `Churn` treated every `\r` as a progress-bar rewrite, collapsing the whole log into one line and hiding the FRESH URL from the sniffer. `\r\n` is now a plain line ending; only a lone `\r` rewrites.
- **FORCE CANCEL**: a red `═[ ^C CANCEL ]═` tab appears on the terminal frame while a route runs; `ctrl+c` (when no text is selected — selection still copies) does the same. Both print `^C` in the terminal as acknowledgement, then cap the run. Re-run stays as before: click the bottle (capped) or RE-POUR (spoiled).
- **[+] BUTTON**: now live — opens a FOLDER… / .ZIP… native picker (via `tauri-plugin-dialog`) and bottles the selection as a new terminal session. Drag-and-drop still works everywhere.
- Known edge (documented in `shell_join`): command args containing quotes/spaces can be mangled by `cmd /C` nested quoting on Windows; resolver-built commands are space-free per arg, so routes are unaffected.

### Round 2 — after live testing with a non-test package (2026-08-06)

- `Churn` now applies minimal terminal semantics instead of stripping control codes: backspace replaces spinner frames (no more `⠙⠹⠸⠼⠴` concatenation), erase-line drops transient frames, cursor-to-column-1 rewrites like a lone `\r`, and clear-screen/cursor-home finish the pending line (no more `> vite1:54:01 a.m.` gluing). UTF-8 characters split across PTY reads are carried over intact.
- DELIVER now emits a plain-language hint when the child's output shows a port conflict (`EADDRINUSE` / "already in use"): close the holder, then RE-POUR.
- Note: CHILL's "port N free" line reflects the framework's *default* port, not project-level overrides in vite config.

### Round 3 — port-clash auto-retry (2026-08-06)

DELIVER now re-pours on a different common port instead of spoiling when the doorstep is occupied:

- Proactive: if the stack's default port is already taken before launch, the run starts steered to a free common port (`! port 5173 taken ─ pouring on 3000 instead`).
- Reactive: when the child dies complaining `EADDRINUSE` / "already in use" (`porter::is_port_clash`, contested port parsed from the message), MILKMAN picks the next *locally-free* untried port from `COMMON_PORTS` and relaunches, up to 4 re-pours.
- Steering is per-stack (`resolver::PortSteer`): `-- --port N` for vite/sveltekit/astro/nuxt, `-- -p N` for next, `PORT=N` env for CRA/generic node/go/rust, `--port N` for uvicorn/flask, `127.0.0.1:N` for django, `-p N` for the sinatra guess. Plain python scripts and the built-in static server (which always picks a free port) are `Fixed` — no retry, clear hint instead.
- So the earlier MILKMAN-in-MILKMAN drop now lands on e.g. port 3000 instead of spoiling against the dev instance's 1420.
