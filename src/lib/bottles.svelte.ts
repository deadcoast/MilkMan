// Crate state — every bottle's lines, stage, and delivery status.

import type {
  BottleInfo,
  BottleState,
  BottleView,
  LinePayload,
  StatePayload,
} from "./types";

const RAW_CAP = 200; // visible raw lines per bottle before folding
const RAW_LOG_CAP = 5000; // full log retention

class Crate {
  bottles = $state<Record<number, BottleView>>({});
  activeId = $state<number | null>(null);
  showFullRaw = $state(false);

  get active(): BottleView | null {
    return this.activeId !== null ? (this.bottles[this.activeId] ?? null) : null;
  }

  get list(): BottleView[] {
    return Object.values(this.bottles).sort((a, b) => a.id - b.id);
  }

  add(info: BottleInfo, activate = true) {
    if (!this.bottles[info.id]) {
      this.bottles[info.id] = {
        id: info.id,
        name: info.name,
        path: info.path,
        state: info.state,
        profile: info.stack
          ? {
              stack: info.stack,
              glyph: info.stack.slice(0, 2),
              manager: "",
              framework: null,
              confidence: "",
            }
          : null,
        url: null,
        port: null,
        lines: [],
        rawLog: [],
        foldedCount: 0,
        startedAt: Date.now(),
        deliveredAt: null,
      };
    } else {
      // re-drop: reset the route
      const b = this.bottles[info.id];
      b.lines = [];
      b.rawLog = [];
      b.foldedCount = 0;
      b.state = info.state;
      b.url = null;
      b.port = null;
      b.startedAt = Date.now();
      b.deliveredAt = null;
    }
    if (activate) this.activeId = info.id;
  }

  remove(id: number) {
    delete this.bottles[id];
    if (this.activeId === id) {
      const rest = this.list;
      this.activeId = rest.length ? rest[rest.length - 1].id : null;
    }
  }

  onLine(p: LinePayload) {
    const b = this.bottles[p.bottle];
    if (!b) return;

    // a new stage header retires the previous one
    if (p.kind === "stage") {
      for (const line of b.lines) {
        if (line.kind === "stage") line.kind = "stagedone";
      }
    }

    if (p.kind === "raw") {
      b.rawLog.push(p.text);
      if (b.rawLog.length > RAW_LOG_CAP) b.rawLog.shift();

      const last = b.lines[b.lines.length - 1];
      if (p.rewrite && last && last.kind === "raw") {
        last.text = p.text; // progress-bar rewrite collapses in place
        return;
      }
      // fold old raw lines past the cap within the trailing raw run
      let runLen = 0;
      for (let i = b.lines.length - 1; i >= 0; i--) {
        const k = b.lines[i].kind;
        if (k === "raw" || k === "fold") runLen++;
        else break;
      }
      if (runLen >= RAW_CAP) {
        // remove the oldest raw line of the run, keep/update the fold marker
        const runStart = b.lines.length - runLen;
        if (b.lines[runStart].kind === "fold") {
          b.lines.splice(runStart + 1, 1);
        } else {
          b.lines.splice(runStart, 1);
          b.lines.splice(runStart, 0, { kind: "fold", text: "" });
        }
        b.foldedCount++;
        const fold = b.lines[runStart];
        fold.text = `\u25b2 ${b.foldedCount} lines folded \u2500 press L for the full log`;
      }
      b.lines.push({ kind: "raw", text: p.text });
      return;
    }

    b.lines.push({ kind: p.kind, text: p.text });
  }

  onState(p: StatePayload) {
    const b = this.bottles[p.bottle];
    if (!b) return;
    b.state = p.state;
    if (p.profile) b.profile = p.profile;
    if (p.url) b.url = p.url;
    if (p.port) b.port = p.port;
    if (p.state === "fresh" && !b.deliveredAt) b.deliveredAt = Date.now();
    if (p.state === "inspecting") {
      b.url = null;
      b.port = null;
      b.deliveredAt = null;
    }
  }
}

export const crate = new Crate();
