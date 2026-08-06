<script lang="ts">
  import { helpEntries, highlight, runCli } from "../cli";

  let value = $state("");
  let menuOpen = $state(false);
  let inputEl: HTMLInputElement | undefined = $state();
  let history: string[] = [];
  let hIdx = -1;

  const hi = $derived(highlight(value));
  const entries = $derived(menuOpen ? helpEntries() : []);

  async function submit() {
    const v = value.trim();
    if (!v) return;
    history.unshift(v);
    if (history.length > 50) history.pop();
    hIdx = -1;
    value = "";
    await runCli(v);
  }

  function onKey(e: KeyboardEvent) {
    // keep ↑/↓ for input history; the crate uses them only outside inputs
    if (e.key === "Enter") {
      e.preventDefault();
      void submit();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (hIdx + 1 < history.length) value = history[++hIdx];
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (hIdx > 0) value = history[--hIdx];
      else {
        hIdx = -1;
        value = "";
      }
    } else if (e.key === "Escape") {
      menuOpen = false;
      inputEl?.blur();
    }
  }

  function pick(insert: string) {
    value = insert;
    menuOpen = false;
    inputEl?.focus();
  }
</script>

<div class="promptbar">
  <span class="ps1">$</span>
  <div class="inputwrap">
    <pre class="mirror" aria-hidden="true">{@html hi}</pre>
    <input
      bind:this={inputEl}
      bind:value
      onkeydown={onKey}
      class="real"
      type="text"
      spellcheck="false"
      autocomplete="off"
      placeholder="pour --port 3000 · help"
      aria-label="milkman command input"
    />
  </div>
  <button
    class="qmark"
    class:open={menuOpen}
    onclick={(e) => {
      e.stopPropagation();
      menuOpen = !menuOpen;
    }}
    title="the milkman's commands"
    aria-expanded={menuOpen}
  >cmd?</button>

  {#if menuOpen}
    <div class="cmdmenu" role="menu">
      {#each entries as en (en.insert)}
        <button role="menuitem" class="cmdrow" onclick={() => pick(en.insert)}>
          <span class="usage">{en.usage}</span>
          <span class="desc">{en.desc}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<svelte:window onclick={() => (menuOpen = false)} />

<style>
  .promptbar {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
    padding: 5px 10px;
    background: var(--void);
    border: 1px solid var(--line);
  }
  .ps1 {
    color: var(--fresh);
    font-family: var(--mono);
    font-size: var(--cell);
    line-height: 1;
  }
  .inputwrap {
    position: relative;
    flex: 1;
    min-width: 0;
    height: calc(var(--cell) + 6px);
  }
  .mirror,
  .real {
    position: absolute;
    inset: 0;
    margin: 0;
    padding: 3px 0;
    border: none;
    background: transparent;
    font-family: var(--mono);
    font-size: var(--cell);
    line-height: 1;
    letter-spacing: normal;
    white-space: pre;
    overflow: hidden;
  }
  .mirror {
    pointer-events: none;
    color: var(--cream);
  }
  .real {
    width: 100%;
    color: transparent;
    caret-color: var(--cream);
    outline: none;
  }
  .real::placeholder {
    color: var(--line);
  }
  .real::selection {
    background: rgba(111, 183, 255, 0.35);
    color: transparent;
  }

  /* bash-ish token colors, rendered in the mirror */
  .mirror :global(.t-cmd) { color: var(--butter); }
  .mirror :global(.t-bad) { color: var(--spoiled); }
  .mirror :global(.t-flag) { color: var(--cold); }
  .mirror :global(.t-num) { color: var(--fresh); }
  .mirror :global(.t-str) { color: var(--fresh); }
  .mirror :global(.t-arg) { color: var(--cream); }

  .qmark {
    font-family: var(--mono);
    font-weight: 400;
    font-size: 10px;
    letter-spacing: normal;
    color: var(--skim);
    background: transparent;
    border: 1px solid var(--line);
    padding: 3px 8px;
    cursor: pointer;
  }
  .qmark:hover,
  .qmark.open {
    color: var(--cream);
    border-color: var(--cream);
  }

  .cmdmenu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 6px);
    width: 340px;
    max-height: 320px;
    overflow-y: auto;
    background: var(--chrome);
    border: 1px solid var(--line);
    z-index: 6;
    display: flex;
    flex-direction: column;
  }
  .cmdrow {
    display: flex;
    align-items: baseline;
    gap: 10px;
    width: 100%;
    padding: 6px 10px;
    background: transparent;
    border: none;
    font-weight: 400;
    letter-spacing: normal;
    text-align: left;
    cursor: pointer;
  }
  .cmdrow:hover {
    background: var(--raised);
  }
  .usage {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--butter);
    white-space: nowrap;
  }
  .desc {
    font-family: var(--mono);
    font-size: 9px;
    color: var(--skim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
