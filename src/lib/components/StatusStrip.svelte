<script lang="ts">
  import { crate } from "../bottles.svelte";

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });

  const bottle = $derived(crate.active);
  const elapsed = $derived.by(() => {
    if (!bottle) return "";
    const end = bottle.deliveredAt ?? now;
    const s = Math.max(0, Math.floor((end - bottle.startedAt) / 1000));
    const mm = String(Math.floor(s / 60)).padStart(2, "0");
    const ss = String(s % 60).padStart(2, "0");
    return `${mm}:${ss}`;
  });

  const stateWord = $derived(bottle ? bottle.state.toUpperCase() : "IDLE");
</script>

<footer class="strip">
  <span class="lead">STATUS &#9656;</span>
  {#if bottle}
    {#if bottle.profile}
      <span class="badge">{bottle.profile.stack}</span>
      {#if bottle.profile.manager}<span class="dot">&#183;</span><span class="badge">{bottle.profile.manager}</span>{/if}
      {#if bottle.profile.framework}<span class="dot">&#183;</span><span class="badge">{bottle.profile.framework}</span>{/if}
      {#if bottle.profile.confidence}<span class="conf">[{bottle.profile.confidence}]</span>{/if}
    {/if}
    {#if bottle.port}<span class="port">PORT {bottle.port}</span>{/if}
    <span class="grow"></span>
    <span class="state s-{bottle.state}">{stateWord}</span>
    <span class="time">{elapsed}</span>
  {:else}
    <span class="grow"></span>
    <span class="state">IDLE</span>
  {/if}
</footer>

<style>
  .strip {
    height: 28px;
    background: var(--raised);
    border-top: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--skim);
  }
  .lead {
    color: var(--skim);
  }
  .badge {
    color: var(--cream);
  }
  .dot {
    color: var(--line);
  }
  .conf {
    color: var(--skim);
  }
  .port {
    color: var(--cold);
    margin-left: 8px;
  }
  .grow {
    flex: 1;
  }
  .state {
    letter-spacing: 0.12em;
  }
  .s-fresh { color: var(--fresh); }
  .s-spoiled { color: var(--spoiled); }
  .s-inspecting, .s-bottling, .s-chilling, .s-delivering { color: var(--butter); }
  .s-capped { color: var(--skim); }
  .time {
    color: var(--cream);
  }
</style>
