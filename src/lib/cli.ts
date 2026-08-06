// The pseudo-CLI: a small, opinionated command set that drives MILKMAN's
// existing machinery. It exists to make the terminal feel like a terminal —
// every flag maps to something the app already does well, and nothing more.

import { crate } from "./bottles.svelte";
import {
  bootstrapEnv,
  open,
  rerunBottle,
  reveal,
  stopBottle,
  watchBottle,
  type PourOverrides,
} from "./ipc";
import type { BottleView } from "./types";

export interface HelpEntry {
  /** text inserted into the input when picked from the cmd_help menu */
  insert: string;
  usage: string;
  desc: string;
}

const HELP: HelpEntry[] = [
  { insert: "pour", usage: "pour", desc: "run the route again, fresh defaults" },
  { insert: "pour --port ", usage: "pour --port <n>", desc: "pin the dev server to a port" },
  { insert: "pour --script ", usage: "pour --script <name>", desc: "run a different package.json script" },
  { insert: "pour --watch", usage: "pour --watch", desc: "re-pour whenever package files change" },
  { insert: "pour --open", usage: "pour --open", desc: "open the browser when FRESH" },
  { insert: "cap", usage: "cap", desc: "stop the run (same as ctrl+c)" },
  { insert: "open", usage: "open", desc: "open the fresh URL in the browser" },
  { insert: "reveal", usage: "reveal", desc: "show the package folder in Explorer" },
  { insert: "env", usage: "env", desc: "create .env from .env.example, then re-pour" },
  { insert: "copy", usage: "copy", desc: "copy this route's log to the clipboard" },
  { insert: "watch off", usage: "watch [off]", desc: "toggle re-pour on file changes" },
  { insert: "clear", usage: "clear", desc: "wipe the screen (scrollback survives on L)" },
  { insert: "help", usage: "help", desc: "you are here" },
];

export const COMMAND_NAMES = ["pour", "cap", "open", "reveal", "env", "copy", "watch", "clear", "help"];

/** Menu entries, extended with the active bottle's real script names. */
export function helpEntries(): HelpEntry[] {
  const scripts = crate.active?.profile?.scripts ?? [];
  const dynamic: HelpEntry[] = scripts.slice(0, 6).map((s) => ({
    insert: `pour --script ${s}`,
    usage: `pour --script ${s}`,
    desc: "from this package.json",
  }));
  return [...HELP, ...dynamic];
}

// ---------------------------------------------------------------- lexer

/** Whitespace-split honoring double quotes. */
function tokenize(input: string): string[] {
  const out: string[] = [];
  const re = /"([^"]*)"|(\S+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(input))) out.push(m[1] ?? m[2]);
  return out;
}

// ---------------------------------------------------------------- runner

export async function runCli(input: string): Promise<void> {
  const b = crate.active;
  if (!b) return;
  const id = b.id;
  crate.say(id, "cmd", `$ ${input}`);

  const t = tokenize(input);
  const cmd = t[0];

  switch (cmd) {
    case "help": {
      crate.say(id, "note", " the milkman answers to:");
      for (const e of HELP) {
        crate.say(id, "note", `   ${e.usage.padEnd(22)} ${e.desc}`);
      }
      return;
    }

    case "pour": {
      const overrides: PourOverrides = {};
      let watch = false;
      let openAfter = false;
      for (let i = 1; i < t.length; i++) {
        switch (t[i]) {
          case "--port": {
            const p = Number(t[++i]);
            if (!Number.isInteger(p) || p < 1 || p > 65535) {
              crate.say(id, "err", " --port needs a number between 1 and 65535");
              return;
            }
            overrides.port = p;
            break;
          }
          case "--script": {
            const s = t[++i];
            if (!s || s.startsWith("--")) {
              crate.say(id, "err", " --script needs a name (try the cmd? menu)");
              return;
            }
            overrides.script = s;
            break;
          }
          case "--watch":
            watch = true;
            break;
          case "--open":
            openAfter = true;
            break;
          default:
            crate.say(id, "err", ` unknown flag "${t[i]}" ─ try help`);
            return;
        }
      }
      if (openAfter) crate.autoOpen[id] = true;
      await rerunBottle(id, overrides);
      if (watch) await watchBottle(id, true).catch((e) => crate.say(id, "err", ` ${e}`));
      return;
    }

    case "cap":
      await stopBottle(id);
      return;

    case "open":
      if (b.url) void open(b.url);
      else crate.say(id, "err", " nothing fresh to open ─ pour first");
      return;

    case "reveal":
      await reveal(b.path).catch((e) => crate.say(id, "err", ` ${e}`));
      return;

    case "env":
      try {
        const msg = await bootstrapEnv(id);
        crate.say(id, "ok", ` ${msg} ─ re-pouring`);
        await rerunBottle(id);
      } catch (e) {
        crate.say(id, "err", ` ${e}`);
      }
      return;

    case "copy": {
      const text = logText(b);
      try {
        await copyToClipboard(text);
        crate.say(id, "ok", ` log copied ─ ${text.split("\n").length} lines`);
      } catch {
        crate.say(id, "err", " clipboard refused the delivery");
      }
      return;
    }

    case "watch": {
      const on = t[1] !== "off";
      await watchBottle(id, on).catch((e) => crate.say(id, "err", ` ${e}`));
      return;
    }

    case "clear":
      b.lines = [];
      b.foldedCount = 0;
      return;

    default:
      crate.say(id, "err", ` unknown command "${cmd}" ─ try help`);
  }
}

function logText(b: BottleView): string {
  const src = crate.showFullRaw ? b.rawLog : b.lines.map((l) => l.text);
  return src.join("\n");
}

async function copyToClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // WebView2 fallback
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    ta.remove();
  }
}

// ---------------------------------------------------------------- highlight

const esc = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Bash-flavored live highlighting for the prompt mirror. */
export function highlight(src: string): string {
  let first = true;
  return src.replace(/(\s+)|(\S+)/g, (_m, ws: string, tok: string) => {
    if (ws) return esc(ws);
    let cls = "t-arg";
    if (first) {
      cls = COMMAND_NAMES.includes(tok) ? "t-cmd" : "t-bad";
      first = false;
    } else if (/^--[a-z-]+$/.test(tok)) cls = "t-flag";
    else if (/^\d+$/.test(tok)) cls = "t-num";
    else if (/^".*"?$/.test(tok)) cls = "t-str";
    return `<span class="${cls}">${esc(tok)}</span>`;
  });
}
