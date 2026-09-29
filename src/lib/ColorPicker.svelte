<script lang="ts">
  import { COLOR_NAMES, COLORS, vivid } from "$lib/notes";

  let { color, onpick }: { color: string; onpick: (color: string) => void } = $props();

  let open = $state(false);
  let root = $state<HTMLElement>();

  function pick(next: string) {
    open = false;
    if (next !== color) onpick(next);
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (open && !root?.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="picker" bind:this={root}>
  <button
    class="current"
    style:background={vivid(color)}
    onclick={() => (open = !open)}
    aria-haspopup="true"
    aria-expanded={open}
    aria-label="Colore della nota"
    title="Colore"
  ></button>
  {#if open}
    <div class="swatches" role="menu">
      {#each COLORS as c (c)}
        <button
          role="menuitemradio"
          aria-checked={c === color}
          class:selected={c === color}
          style:background={c}
          style:--ring={vivid(c)}
          onclick={() => pick(c)}
          aria-label={COLOR_NAMES[c]}
          title={COLOR_NAMES[c]}
        ></button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    display: flex;
  }
  .current {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 2px solid rgba(255, 255, 255, 0.85);
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.18);
    cursor: pointer;
  }
  .swatches {
    position: absolute;
    top: calc(100% + 8px);
    right: -8px;
    z-index: 10;
    display: flex;
    gap: 6px;
    padding: 8px;
    border-radius: 12px;
    background: #fff;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  }
  .swatches button {
    width: 22px;
    height: 22px;
    padding: 0;
    border: 2px solid var(--ring);
    border-radius: 50%;
    cursor: pointer;
  }
  .swatches button.selected {
    box-shadow:
      0 0 0 2px #fff,
      0 0 0 4px var(--ring);
  }
</style>
