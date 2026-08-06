<script lang="ts">
  import { crate } from "../bottles.svelte";
  import { loadHistory, stopBottle } from "../ipc";
  import Prompt from "./Prompt.svelte";
  import type { Line } from "../types";

  const FRAMES = ["\u2500", "\\", "\u2502", "/"];
  let frame = $state(0);
  let scroller: HTMLDivElement | undefined = $state();
  let pinned = $state(true); // autoscroll unless the user scrolls up

  $effect(() => {
    const t = setInterval(() => (frame = (frame + 1) % FRAMES.length), 140);
    return () => clearInterval(t);
  });

  const bottle = $derived(crate.active);
  const running = $derived(
    bottle !== null &&
      ["inspecting", "bottling", "chilling", "delivering"].includes(bottle.state)
  );
  const resting = $derived(
    bottle !== null && !running && bottle.lines.length === 0
  );

  // a restored bottle replays its persisted last log into scrollback (L)
  $effect(() => {
    const b = crate.active;
    if (b && b.restored && !b.historyLoaded) {
      void loadHistory(b);
    }
  });

  function badges(): string {
    const p = bottle?.profile;
    if (!p) return "an unopened delivery";
    return [p.stack, p.manager, p.framework].filter(Boolean).join(" \u00b7 ");
  }

  // autoscroll on new lines
  $effect(() => {
    if (!bottle) return;
    void bottle.lines.length;
    void crate.showFullRaw;
    if (pinned && scroller) {
      requestAnimationFrame(() => {
        if (scroller) scroller.scrollTop = scroller.scrollHeight;
      });
    }
  });

  function onScroll() {
    if (!scroller) return;
    pinned =
      scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 24;
  }

  function classFor(line: Line): string {
    return `ln ${line.kind}`;
  }

  function spin(line: Line): string {
    // active stage header gets the live spinner
    return line.kind === "stage" && running ? ` ${FRAMES[frame]}` : "";
  }
</script>

<div class="terminal">
  <span class="title">&#9552;[ MILKMAN v0.0.1{bottle ? ` · ${bottle.name}` : ""} ]&#9552;</span>
  {#if bottle && running}
    <button
      class="cancel"
      onclick={() => bottle && stopBottle(bottle.id)}
      title="interrupt the route — hotkey: ctrl+c"
    >&#9552;[ ^C CANCEL ]&#9552;</button>
  {/if}
  <div class="frame">

  <div
    class="screen"
    bind:this={scroller}
    onscroll={onScroll}
    role="log"
    aria-label="route output"
  >
    {#if !bottle}
      <div class="empty">
        <pre class="bottle-art">{`      ____
     |    |
     |____|
    /      \\
   |  MILK  |
   |        |
   |________|`}</pre>
        <p class="empty-line">DROP A PACKAGE</p>
        <p class="empty-sub">a project folder, or a .zip straight from claude.ai</p>
      </div>
    {:else if crate.showFullRaw}
      <div class="ln note">&#9472;&#9472; full log ({bottle.rawLog.length} lines) &#9472; press L to fold &#9472;&#9472;</div>
      {#each bottle.rawLog as text}
        <div class="ln raw">{text}</div>
      {/each}
    {:else if resting}
      <div class="empty resting">
        <pre class="bottle-art">{`      ____
     |    |
     |____|
    /      \\
   |  MILK  |
   |        |
   |________|`}</pre>
        <p class="rest-name">{bottle.name}</p>
        <p class="rest-badges">{badges()}</p>
        <p class="rest-path" title={bottle.path}>{bottle.path}</p>
        {#if bottle.lastUrl}
          <p class="rest-url">last delivery &#9472; {bottle.lastUrl}</p>
        {/if}
        <p class="empty-sub">press R or type pour to deliver &#183; L for the last log</p>
      </div>
    {:else}
      {#each bottle.lines as line}
        <div class={classFor(line)}>{line.text}{spin(line)}</div>
      {/each}
      {#if running}
        <span class="cursor">&#9619;</span>
      {/if}
    {/if}
  </div>

  <div class="scanlines" aria-hidden="true"></div>
  </div>

  {#if bottle}
    <Prompt />
  {/if}
</div>

<style>
  /* the wrapper never clips, so the title and cancel tab can straddle the
     frame's top border without being cut off by .frame's overflow:hidden */
  .terminal {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .frame {
    position: relative;
    flex: 1;
    min-height: 0;
    background: var(--void);
    border: 4px double var(--line);
    overflow: hidden;
  }

  /* the title and cancel tabs float above the scrolling output: a padded
     underline plus a drop shadow separate them from the log beneath */
  .title {
    position: absolute;
    top: 0;
    left: 18px;
    transform: translateY(-50%);
    background: var(--chrome);
    padding: 4px 8px 5px;
    border-bottom: 1px solid var(--line);
    box-shadow: 0 5px 14px rgba(0, 0, 0, 0.65);
    font-family: var(--mono);
    font-size: var(--cell);
    color: var(--cream);
    z-index: 2;
    line-height: 1;
    max-width: calc(100% - 160px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cancel {
    position: absolute;
    top: 0;
    right: 18px;
    transform: translateY(-50%);
    background: var(--chrome);
    border: none;
    padding: 4px 8px 5px;
    border-bottom: 1px solid var(--line);
    box-shadow: 0 5px 14px rgba(0, 0, 0, 0.65);
    font-family: var(--mono);
    font-weight: 400;
    font-size: var(--cell);
    letter-spacing: normal;
    color: var(--spoiled);
    z-index: 2;
    line-height: 1;
    cursor: pointer;
  }
  .cancel:hover {
    color: var(--cream);
    border-bottom-color: var(--cream);
  }

  .screen {
    height: 100%;
    overflow-y: auto;
    padding: 18px 16px 24px;
    font-family: var(--mono);
    font-size: var(--cell);
    line-height: 2; /* 22px on an 11px cell — integer grid */
    user-select: text;
    -webkit-user-select: text;
  }

  .ln {
    white-space: pre-wrap;
    word-break: break-all;
  }

  .ln.stage {
    color: var(--butter);
    margin-top: 8px;
  }
  .ln.stagedone {
    color: var(--fresh);
    margin-top: 8px;
  }
  .ln.tree {
    color: var(--cream);
  }
  .ln.ok {
    color: var(--fresh);
  }
  .ln.warn {
    color: var(--butter);
  }
  .ln.err {
    color: var(--spoiled);
  }
  .ln.note {
    color: var(--skim);
  }
  .ln.url {
    color: var(--cold);
  }
  .ln.sig {
    color: var(--spoiled);
  }
  .ln.cmd {
    color: var(--cream);
    margin-top: 4px;
  }
  .ln.fold {
    color: var(--skim);
    border-top: 1px solid var(--line);
    margin-top: 4px;
    padding-top: 4px;
  }

  /* raw process output: dimmed, left-ruled containment */
  .ln.raw {
    color: var(--skim);
    border-left: 1px solid var(--line);
    padding-left: 10px;
    margin-left: 4px;
  }

  .cursor {
    display: inline-block;
    color: var(--cream);
    font-family: var(--mono);
    font-size: var(--cell);
    animation: blink 1.06s step-end infinite;
    margin-left: 4px;
  }

  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }
  .bottle-art {
    color: var(--line);
    font-family: var(--mono);
    font-size: var(--cell);
    line-height: 1.4;
  }
  .empty-line {
    color: var(--cream);
    font-family: var(--mono);
    font-size: var(--cell);
    letter-spacing: 0.3em;
  }
  .empty-sub {
    color: var(--skim);
    font-family: var(--mono);
    font-size: var(--cell);
  }

  /* a capped bottle at rest — the void, furnished */
  .rest-name {
    color: var(--cream);
    font-family: var(--mono);
    font-size: calc(var(--cell) + 2px);
    letter-spacing: 0.2em;
  }
  .rest-badges {
    color: var(--butter);
    font-family: var(--mono);
    font-size: var(--cell);
  }
  .rest-path {
    color: var(--skim);
    font-family: var(--mono);
    font-size: calc(var(--cell) - 2px);
    max-width: 80%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rest-url {
    color: var(--cold);
    font-family: var(--mono);
    font-size: var(--cell);
  }

  /* one texture cue: 3% scanlines + soft vignette, terminal only */
  .scanlines {
    pointer-events: none;
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse at center, transparent 62%, rgba(0, 0, 0, 0.22) 100%),
      repeating-linear-gradient(
        to bottom,
        rgba(255, 255, 255, 0.03) 0px,
        rgba(255, 255, 255, 0.03) 1px,
        transparent 1px,
        transparent 3px
      );
  }
</style>
