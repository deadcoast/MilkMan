<script lang="ts">
  let { over = false }: { over?: boolean } = $props();
</script>

{#if over}
  <div class="veil" aria-hidden="true">
    <div class="target">
      <pre class="art">{`  ____
 |    |
 |____|
/      \\
|      |
|______|`}</pre>
      <span class="label">RELEASE TO DELIVER</span>
    </div>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    background: rgba(12, 12, 16, 0.88);
    display: grid;
    place-items: center;
    z-index: 50;
  }
  .target {
    border: 1px dashed var(--cold);
    padding: 34px 60px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }
  .art {
    color: var(--cold);
    font-family: var(--mono);
    font-size: var(--cell);
    line-height: 1.4;
  }
  .label {
    color: var(--cream);
    font-family: var(--mono);
    font-size: var(--cell);
    letter-spacing: 0.3em;
  }
</style>
