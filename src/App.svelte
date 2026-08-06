<script lang="ts">
  import { onMount } from "svelte";
  import { crate } from "./lib/bottles.svelte";
  import { wire } from "./lib/ipc";
  import Crate from "./lib/components/Crate.svelte";
  import Terminal from "./lib/components/Terminal.svelte";
  import StatusStrip from "./lib/components/StatusStrip.svelte";
  import DeliverySlip from "./lib/components/DeliverySlip.svelte";
  import DropZone from "./lib/components/DropZone.svelte";

  let dragOver = $state(false);

  onMount(() => {
    void wire((over) => (dragOver = over));

    const onKey = (e: KeyboardEvent) => {
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
