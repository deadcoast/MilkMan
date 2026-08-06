<script lang="ts">
  import { crate } from "../bottles.svelte";
  import { open, rerunBottle, stopBottle } from "../ipc";

  const bottle = $derived(crate.active);
  const show = $derived(
    bottle !== null &&
      (bottle.state === "fresh" ||
        bottle.state === "spoiled" ||
        bottle.state === "capped")
  );
</script>

{#if bottle && show}
  <aside class="slip" class:spoiled={bottle.state === "spoiled"}>
    {#if bottle.state === "fresh"}
      <span class="mark fresh">&#9679; FRESH</span>
      {#if bottle.url}
        <span class="url" title={bottle.url}>{bottle.url}</span>
        <button class="cold" onclick={() => bottle.url && open(bottle.url)}>OPEN</button>
      {/if}
      <button onclick={() => stopBottle(bottle.id)}>STOP</button>
    {:else if bottle.state === "spoiled"}
      <span class="mark bad">&#9679; SPOILED</span>
      <span class="url">route failed &#9472; details above</span>
      <button onclick={() => rerunBottle(bottle.id)}>RE-POUR</button>
    {:else}
      <span class="mark capped">&#9679; CAPPED</span>
      <span class="url dim">route stopped &#9472; R re-pours</span>
      <button onclick={() => rerunBottle(bottle.id)}>RE-POUR</button>
    {/if}
  </aside>
{/if}

<style>
  .slip {
    position: absolute;
    left: 20px;
    right: 20px;
    bottom: 48px; /* clears the prompt bar */
    background: var(--raised);
    border: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    animation: slideup 240ms ease-out;
    z-index: 4;
  }
  .slip.spoiled {
    border-color: var(--spoiled);
  }
  .mark {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.08em;
  }
  .mark.fresh {
    color: var(--fresh);
  }
  .mark.bad {
    color: var(--spoiled);
  }
  .mark.capped {
    color: var(--skim);
  }
  .url.dim {
    color: var(--skim);
  }
  .url {
    flex: 1;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--cold);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .slip.spoiled .url {
    color: var(--skim);
  }
</style>
