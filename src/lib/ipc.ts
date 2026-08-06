// Bridge to the Rust core: commands out, route events in.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { openUrl } from "@tauri-apps/plugin-opener";
import { crate } from "./bottles.svelte";
import type { BottleInfo, LinePayload, StatePayload } from "./types";

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

export const stopBottle = (id: number) => invoke("stop_bottle", { id });
export const rerunBottle = (id: number) => invoke("rerun_bottle", { id });
export const removeBottle = (id: number) => invoke("remove_bottle", { id });
export const open = (url: string) => openUrl(url);

export async function wire(onDragState: (over: boolean) => void): Promise<void> {
  await listen<LinePayload>("route:line", (e) => crate.onLine(e.payload));
  await listen<StatePayload>("route:state", (e) => crate.onState(e.payload));

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
