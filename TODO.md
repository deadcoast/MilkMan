# TODO

(a) The terminal is a black void. A capped/restored bottle has no lines (logs aren't persisted), and the "DROP A PACKAGE" art only renders when no bottle exists — so the restored bottle greets you with nothing.
(b) The crate glyph reads "NO". restore_crate doesn't send a glyph, so the frontend falls back to stack.slice(0, 2) → "NODE" becomes "NO" — which reads like a warning label, not a Node badge. (Live-detected bottles correctly show "N".)
(c) The timer is lying. CAPPED 00:06 is counting from when the bottle was restored, not anything meaningful. And the status strip shows only "NODE" — the ledger already stores manager and framework, but restore_crate doesn't pass them through.

------
[001]:

- Bottle "resting" screen — capped bottle with no log shows the bottle art + name, stack/manager badges, path, last URL if known, and click / press R to RE-POUR. Kills the black void.
- Fix the restored glyph + profile — map stack→glyph on the Rust side and pass the ledger's manager/framework through restore_crate; show — instead of a ticking timer for restored bottles.
- Persist each bottle's last log to app data, so a restored bottle can replay its previous route instead of being amnesiac.
- Explicit RE-POUR affordance for capped bottles — right now clicking a capped bottle in the crate silently re-runs it, which makes selection destructive. Selecting should select; a slip button (RE-POUR) and the R key should re-run.
- Show the bottle name + URL in the status strip — the full name currently app at 7 chars), and the URL lives only in the slip.

------
[002]:

- CLI Pseudo Input — Introduce a flag input system that accepts cli flags `--flags`. **THIS DOES NOT CHANGE OR REPLACE THE CURRENT METHODS; IT ADDS ONTO THEM AS AN ADDITIONAL INPUT FEATURE**. Add a small text input area fo further simulate the terminal. Add bash syntax highlighting to the usrtd input to increase their experience and the applications design.
- CLI Input `cmd_help` Button — A button that when clicked on provides a dropdown menu of the `commands` / `--flags`. Lets keep these somewhat opinionated; we dont need long chains of cli for users here. This feature is created to make users FEEL like they are working through the cli, to make them FEEL like they are more advanced users than they are. In reality we are catering these flags and commands to do exactly what we want them to. It is a layer of user experience while not losing quality of life by introducing traditional cli command prompt usage as they can get complex.
- "Pour on port…" — manual port override UI on top of the auto-retry you already have.
- One-click .env bootstrap — CHILL already warns about .env.example with no .env; add a button that copies the example into place and re-pours.
- Reveal in Explorer — the opener plugin is already a dependency; revealItemInDir on the bottle's path is nearly free.
- Copy full log button (and maybe Ctrl+F filter for long logs).
- Watch mode — re-pour automatically when files in the package change (notify crate). Great for the claude.ai artifact iterate-loop.
- Auto-open on FRESH (opt-in setting) — the zip's whole point is "ready to drink."
- Script picker — when a package.json has several scripts, let the user override which one pours (dropdown in the slip or right-click on the bottle). Pairs well with the existing INFERRED/GUESSED confidence labels.

------
[003]:

- Dropdown Menu `cmd_help` button that displays the relevant introduced `--flag` input function
- Hotkey hints in the status strip — dim chips like ^C cancel · L log · R re-poverable via the fold marker and ^C via the cancel button tooltip.
- Keyboard crate navigation — ↑/↓ to switch bottles.
- Give the M brand tile a job — About panel or settings (port list, auto-open, notifications); it's currently decorative.
- Terminal text zoom — Ctrl +/− adjusting --cell; 11px is crisp but small on high-DPI.
- Custom title bar — the native Windows bar breaks the CRT aesthetic; Tauri overlay decorations would let the chrome go full milk-noir. (Largest item here, purely cosmetic.)

------
