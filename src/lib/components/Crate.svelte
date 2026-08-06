<script lang="ts">
  import { crate } from "../bottles.svelte";
  import { pickPackage, removeBottle } from "../ipc";
  import type { BottleView } from "../types";

  let addOpen = $state(false);

  async function addFrom(kind: "folder" | "zip") {
    addOpen = false;
    await pickPackage(kind);
  }

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

  // selecting selects — re-pouring is explicit (RE-POUR button, R, `pour`)
  function pick(b: BottleView) {
    crate.activeId = b.id;
  }

  function drop(e: MouseEvent, b: BottleView) {
    e.stopPropagation();
    void removeBottle(b.id);
    crate.remove(b.id);
  }
</script>

<nav class="crate" aria-label="crate">
  <button
    class="brand"
    title="about the milkman"
    onclick={(e) => {
      e.stopPropagation();
      crate.showSettings = !crate.showSettings;
    }}
  >M</button>
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
  <div class="add-wrap">
    <button
      class="add"
      class:open={addOpen}
      onclick={(e) => {
        e.stopPropagation();
        addOpen = !addOpen;
      }}
      title="add a package — or drag one anywhere"
      aria-label="add a package"
      aria-expanded={addOpen}
    >+</button>
    {#if addOpen}
      <div class="menu" role="menu">
        <button role="menuitem" onclick={() => addFrom("folder")}>FOLDER&#8230;</button>
        <button role="menuitem" onclick={() => addFrom("zip")}>.ZIP&#8230;</button>
        <span class="menu-hint">or drag a package anywhere</span>
      </div>
    {/if}
  </div>
</nav>

<svelte:window onclick={() => (addOpen = false)} />

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
    font-weight: 400;
    font-size: 18px;
    letter-spacing: normal;
    color: var(--cream);
    background: transparent;
    border: 1px solid var(--line);
    width: 48px;
    height: 30px;
    padding: 0;
    display: grid;
    place-items: center;
    margin-bottom: 4px;
    cursor: pointer;
  }
  .brand:hover {
    border-color: var(--cream);
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

  .add-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .add {
    color: var(--skim);
    font-family: var(--mono);
    font-size: 16px;
    width: 48px;
    height: 48px;
    padding: 0;
    background: transparent;
    border: 1px dashed var(--line);
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .add:hover,
  .add.open {
    color: var(--cream);
  }
  .menu {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 60px;
  }
  .menu button {
    width: 100%;
    padding: 5px 0;
    font-family: var(--mono);
    font-size: 9px;
    letter-spacing: 0.06em;
  }
  .menu-hint {
    color: var(--skim);
    font-family: var(--mono);
    font-size: 8px;
    line-height: 1.5;
    text-align: center;
  }
</style>
