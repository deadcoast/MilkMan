<script lang="ts">
  import { onMount } from "svelte";
  import { crate } from "./lib/bottles.svelte";
  import { rerunBottle, stopBottle, wire } from "./lib/ipc";
  import { applyCell, CELL_BASE, settings } from "./lib/settings";
  import Crate from "./lib/components/Crate.svelte";
  import Terminal from "./lib/components/Terminal.svelte";
  import StatusStrip from "./lib/components/StatusStrip.svelte";
  import DeliverySlip from "./lib/components/DeliverySlip.svelte";
  import DropZone from "./lib/components/DropZone.svelte";
  import Settings from "./lib/components/Settings.svelte";

  let dragOver = $state(false);

  onMount(() => {
    applyCell(settings.cell);
    void wire((over) => (dragOver = over));

    const INTERRUPTIBLE = ["inspecting", "bottling", "chilling", "delivering", "fresh"];
    const REPOURABLE = ["capped", "spoiled", "fresh"];
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      const typing = el?.tagName === "INPUT" || el?.tagName === "TEXTAREA";

      // ctrl+c interrupts the active route — unless the user is copying text
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
        const b = crate.active;
        const input = typing ? (el as HTMLInputElement) : null;
        const copying =
          (window.getSelection()?.toString() ?? "") !== "" ||
          (input !== null && input.selectionStart !== input.selectionEnd);
        if (b && !copying && INTERRUPTIBLE.includes(b.state)) {
          e.preventDefault();
          void stopBottle(b.id);
        }
        return;
      }

      // ctrl +/−/0 — terminal zoom
      if (e.ctrlKey || e.metaKey) {
        if (e.key === "=" || e.key === "+") {
          e.preventDefault();
          settings.setCell(settings.cell + 1);
        } else if (e.key === "-") {
          e.preventDefault();
          settings.setCell(settings.cell - 1);
        } else if (e.key === "0") {
          e.preventDefault();
          settings.setCell(CELL_BASE);
        }
        return;
      }

      if (typing) return; // the prompt owns bare keys while focused

      if (e.key === "Escape" && crate.showSettings) {
        crate.showSettings = false;
        return;
      }

      // ↑/↓ — walk the crate
      if (e.key === "ArrowUp" || e.key === "ArrowDown") {
        const list = crate.list;
        if (!list.length) return;
        e.preventDefault();
        const idx = list.findIndex((b) => b.id === crate.activeId);
        const step = e.key === "ArrowDown" ? 1 : -1;
        const next = list[(idx + step + list.length) % list.length];
        crate.activeId = next.id;
        return;
      }

      // R — explicit re-pour
      if (e.key.toLowerCase() === "r") {
        const b = crate.active;
        if (b && REPOURABLE.includes(b.state)) {
          void rerunBottle(b.id);
        }
        return;
      }

      if (e.key.toLowerCase() === "l" && crate.active) {
        crate.showFullRaw = !crate.showFullRaw;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<main class="shell">
  <Crate />
  <section class="stage">
    <div class="terminal-wrap">
      <Terminal />
      <DeliverySlip />
    </div>
    <StatusStrip />
  </section>
</main>
<DropZone over={dragOver} />
{#if crate.showSettings}
  <Settings />
{/if}

<style>
  .shell {
    height: 100%;
    display: flex;
  }
  .stage {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .terminal-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 14px 14px 8px;
  }
</style>
