<script lang="ts">
  import { crate } from "../bottles.svelte";
  import { removeBottle, rerunBottle } from "../ipc";
  import type { BottleView } from "../types";

  function ringClass(b: BottleView): string {
    switch (b.state) {
      case "fresh":
        return "ring-fresh";
      case "spoiled":
        return "ring-spoiled";
      case "capped":
        return "ring-capped";
      default:
        return "ring-running";
    }
  }

  function pick(b: BottleView) {
    crate.activeId = b.id;
    // waking a capped bottle re-runs its route
    if (b.state === "capped") void rerunBottle(b.id);
  }

  function drop(e: MouseEvent, b: BottleView) {
    e.stopPropagation();
    void removeBottle(b.id);
    crate.remove(b.id);
  }
</script>

<nav class="crate" aria-label="crate">
  <div class="brand" title="MILKMAN">M</div>
  {#each crate.list as b (b.id)}
    <button
      class="slot {ringClass(b)}"
      class:active={crate.activeId === b.id}
      onclick={() => pick(b)}
      title={`${b.name} — ${b.state}`}
    >
      <span class="glyph">{b.profile?.glyph ?? "?"}</span>
      <span class="name">{b.name.slice(0, 7)}</span>
      <span
        class="x"
        role="button"
        tabindex="-1"
        aria-label="remove"
        onclick={(e) => drop(e, b)}
        onkeydown={() => {}}
      >&times;</span>
    </button>
  {/each}
  <div class="hint" aria-hidden="true">+<span class="hint-text">drag a package anywhere</span></div>
</nav>

<style>
  .crate {
    width: 72px;
    background: var(--chrome);
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 12px 0;
    overflow-y: auto;
  }

  .brand {
    font-family: var(--mono);
    font-size: 18px;
    color: var(--cream);
    border: 1px solid var(--line);
    width: 48px;
    height: 30px;
    display: grid;
    place-items: center;
    margin-bottom: 4px;
  }

  .slot {
    position: relative;
    width: 48px;
    height: 48px;
    padding: 0;
    background: var(--raised);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    animation: slosh 180ms ease-out;
  }
  .slot.active {
    border-color: var(--cream);
  }

  .glyph {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--cream);
  }
  .name {
    font-family: var(--mono);
    font-size: 8px;
    color: var(--skim);
    max-width: 44px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .slot::before {
    content: "";
    position: absolute;
    top: 4px;
    right: 4px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .ring-running::before {
    background: var(--cold);
    animation: blink 1.2s step-end infinite;
  }
  .ring-fresh::before {
    background: var(--fresh);
  }
  .ring-spoiled::before {
    background: var(--spoiled);
  }
  .ring-capped::before {
    background: var(--skim);
  }

  .x {
    position: absolute;
    top: -1px;
    left: 2px;
    color: var(--skim);
    font-size: 11px;
    line-height: 1;
    opacity: 0;
    cursor: pointer;
  }
  .slot:hover .x {
    opacity: 1;
  }
  .x:hover {
    color: var(--spoiled);
  }

  .hint {
    color: var(--line);
    font-family: var(--mono);
    font-size: 16px;
    width: 48px;
    height: 48px;
    border: 1px dashed var(--line);
    display: grid;
    place-items: center;
    position: relative;
  }
  .hint-text {
    display: none;
  }
  .hint:hover .hint-text {
    display: block;
    position: absolute;
    left: 56px;
    top: 50%;
    transform: translateY(-50%);
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--skim);
    font-size: 9px;
    padding: 4px 8px;
    white-space: nowrap;
    z-index: 5;
  }
</style>
