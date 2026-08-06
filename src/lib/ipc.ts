// Bridge to the Rust core: commands out, route events in.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { crate } from "./bottles.svelte";
import { settings } from "./settings";
import type { BottleInfo, BottleView, LinePayload, LogLine, StatePayload } from "./types";

export async function loadProject(path: string): Promise<void> {
  try {
    const info = await invoke<BottleInfo>("load_project", { path });
    crate.add(info);
  } catch (e) {
    // surface intake errors on whatever bottle is active, or ignore silently
    const active = crate.active;
    if (active) {
      crate.onLine({
        bottle: active.id,
        kind: "err",
        text: `SPOILED \u2500 ${String(e)}`,
        rewrite: false,
      });
    }
    console.error("load_project:", e);
  }
}

/** The [+] button: browse for a package (folder or .zip) and bottle it. */
export async function pickPackage(kind: "folder" | "zip"): Promise<void> {
  const path =
    kind === "folder"
      ? await openDialog({
          directory: true,
          title: "MILKMAN ─ choose a package folder",
        })
      : await openDialog({
          filters: [{ name: "package archive", extensions: ["zip"] }],
          title: "MILKMAN ─ choose a package .zip",
        });
  if (typeof path === "string") await loadProject(path);
}

export interface PourOverrides {
  port?: number;
  script?: string;
}

export const stopBottle = (id: number) => invoke("stop_bottle", { id });
export const rerunBottle = (id: number, overrides: PourOverrides = {}) =>
  invoke("rerun_bottle", {
    id,
    port: overrides.port ?? null,
    script: overrides.script ?? null,
  });
export const removeBottle = (id: number) => invoke("remove_bottle", { id });
export const open = (url: string) => openUrl(url);
export const reveal = (path: string) => revealItemInDir(path);
export const watchBottle = (id: number, on: boolean) =>
  invoke("watch_bottle", { id, on });
export const bootstrapEnv = (id: number) =>
  invoke<string>("bootstrap_env", { id });

/** Lazily replay a restored bottle's persisted last log into scrollback. */
export async function loadHistory(b: BottleView): Promise<void> {
  b.historyLoaded = true;
  const lines = await invoke<LogLine[]>("bottle_log", { id: b.id });
  if (b.rawLog.length === 0 && lines.length) {
    b.rawLog = lines.map((l) => l.text);
  }
}

export async function wire(onDragState: (over: boolean) => void): Promise<void> {
  await listen<LinePayload>("route:line", (e) => crate.onLine(e.payload));
  await listen<StatePayload>("route:state", (e) => {
    crate.onState(e.payload);
    // `pour --open` or the global auto-open setting: FRESH pops the browser
    if (e.payload.state === "fresh" && e.payload.url) {
      const once = crate.autoOpen[e.payload.bottle];
      if (once || settings.autoOpen) {
        delete crate.autoOpen[e.payload.bottle];
        void openUrl(e.payload.url);
      }
    }
  });

  await getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "over" || payload.type === "enter") {
      onDragState(true);
    } else if (payload.type === "leave") {
      onDragState(false);
    } else if (payload.type === "drop") {
      onDragState(false);
      for (const p of payload.paths) void loadProject(p);
    }
  });

  const restored = await invoke<BottleInfo[]>("restore_crate");
  for (const info of restored) crate.add(info, false);
  if (restored.length && crate.activeId === null) {
    crate.activeId = restored[0].id;
  }
}
