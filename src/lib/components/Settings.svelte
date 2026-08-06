<script lang="ts">
  import { crate } from "../bottles.svelte";
  import { CELL_BASE, settings } from "../settings";

  let autoOpen = $state(settings.autoOpen);
  let cell = $state(settings.cell);

  function toggleAutoOpen() {
    autoOpen = !autoOpen;
    settings.setAutoOpen(autoOpen);
  }

  function bumpCell(d: number) {
    settings.setCell(settings.cell + d);
    cell = settings.cell;
  }

  function resetCell() {
    settings.setCell(CELL_BASE);
    cell = settings.cell;
  }
</script>

<div
  class="veil"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) crate.showSettings = false;
  }}
>
  <div class="panel" role="dialog" aria-modal="true" aria-label="about milkman">
    <span class="panel-title">&#9552;[ THE MILKMAN ]&#9552;</span>

    <p class="blurb">
      MILKMAN v0.0.1 &#9472; drop a folder or a claude.ai .zip and it is
      inspected, bottled, chilled, and delivered fresh to a local port.
    </p>

    <div class="row">
      <span class="label">auto-open on FRESH</span>
      <button class="toggle" class:on={autoOpen} onclick={toggleAutoOpen}>
        {autoOpen ? "ON" : "OFF"}
      </button>
    </div>

    <div class="row">
      <span class="label">terminal size &#9472; {cell}px</span>
      <span class="zoomctl">
        <button onclick={() => bumpCell(-1)} title="ctrl −">&#8722;</button>
        <button onclick={resetCell} title="ctrl 0">reset</button>
        <button onclick={() => bumpCell(1)} title="ctrl +">+</button>
      </span>
    </div>

    <div class="section">HOTKEYS</div>
    <div class="keys">
      <span>ctrl+c</span><span>cap the running route</span>
      <span>R</span><span>re-pour the active bottle</span>
      <span>L</span><span>full log / fold</span>
      <span>&#8593; &#8595;</span><span>switch bottles (crate)</span>
      <span>ctrl +/&#8722;/0</span><span>terminal zoom</span>
    </div>

    <div class="section">CLI</div>
    <p class="blurb dim">
      the prompt under the terminal answers to: pour [--port n] [--script name]
      [--watch] [--open] &#183; cap &#183; open &#183; reveal &#183; env &#183;
      copy &#183; watch off &#183; clear &#183; help
    </p>

    <div class="section">PORTS</div>
    <p class="blurb dim">
      when a doorstep is taken, the route re-pours across: 3000 3001 4000 4173
      4321 5000 5173 5174 8000 8080 8081 8888
    </p>

    <button class="close" onclick={() => (crate.showSettings = false)}>CLOSE</button>
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    background: rgba(12, 12, 16, 0.82);
    display: grid;
    place-items: center;
    z-index: 40;
  }
  .panel {
    position: relative;
    width: 440px;
    max-width: calc(100vw - 80px);
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    background: var(--chrome);
    border: 4px double var(--line);
    padding: 22px 20px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .panel-title {
    position: absolute;
    top: 0;
    left: 18px;
    transform: translateY(-50%);
    background: var(--chrome);
    padding: 0 6px;
    font-family: var(--mono);
    font-size: var(--cell);
    color: var(--cream);
    line-height: 1;
  }
  .blurb {
    font-family: var(--mono);
    font-size: 10px;
    line-height: 1.7;
    color: var(--cream);
  }
  .blurb.dim {
    color: var(--skim);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .label {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--cream);
  }
  .toggle {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    min-width: 52px;
    color: var(--skim);
  }
  .toggle.on {
    color: var(--fresh);
    border-color: var(--fresh);
  }
  .zoomctl {
    display: flex;
    gap: 6px;
  }
  .zoomctl button {
    font-family: var(--mono);
    font-size: 10px;
    padding: 3px 10px;
  }
  .section {
    font-family: var(--mono);
    font-size: 9px;
    letter-spacing: 0.2em;
    color: var(--skim);
    border-bottom: 1px solid var(--line);
    padding-bottom: 3px;
    margin-top: 4px;
  }
  .keys {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 16px;
    font-family: var(--mono);
    font-size: 10px;
  }
  .keys span:nth-child(odd) {
    color: var(--butter);
  }
  .keys span:nth-child(even) {
    color: var(--skim);
  }
  .close {
    align-self: flex-end;
    margin-top: 6px;
  }
</style>
