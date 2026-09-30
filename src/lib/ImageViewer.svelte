<script lang="ts">
  import { onMount } from "svelte";

  let { src, alt = "", onclose }: { src: string; alt?: string; onclose: () => void } = $props();

  /** 1 = adattata alla finestra; oltre, si ingrandisce e si scorre. */
  let zoom = $state(1);

  function setZoom(next: number) {
    zoom = Math.min(8, Math.max(1, next));
  }

  function onwheel(event: WheelEvent) {
    // A zoom 1 la rotella non fa nulla; oltre, con Ctrl zoom, altrimenti scorre.
    if (!event.ctrlKey) return;
    event.preventDefault();
    setZoom(zoom * (event.deltaY < 0 ? 1.2 : 1 / 1.2));
  }

  onMount(() => {
    // In cattura: Esc chiude solo il visualizzatore, non la nota sotto.
    const onkey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onclose();
      }
    };
    window.addEventListener("keydown", onkey, true);
    return () => window.removeEventListener("keydown", onkey, true);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="viewer" onclick={onclose} {onwheel}>
  <div class="bar" onclick={(e) => e.stopPropagation()}>
    <button onclick={() => setZoom(zoom / 1.4)} aria-label="Riduci" title="Riduci">−</button>
    <span>{Math.round(zoom * 100)}%</span>
    <button onclick={() => setZoom(zoom * 1.4)} aria-label="Ingrandisci" title="Ingrandisci">+</button>
    <button onclick={() => setZoom(1)} title="Adatta alla finestra">Adatta</button>
    <button onclick={onclose} aria-label="Chiudi" title="Chiudi (Esc)">✕</button>
  </div>
  <div class="stage" style:--zoom={zoom}>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events -->
    <img
      {src}
      {alt}
      draggable="false"
      class:zoomed={zoom > 1}
      onclick={(e) => {
        e.stopPropagation();
        setZoom(zoom > 1 ? 1 : 2.5);
      }}
    />
  </div>
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: rgba(8, 10, 16, 0.92);
    overflow: auto;
    display: flex;
  }
  .stage {
    margin: auto;
    padding: 44px 12px 12px;
    /* A zoom 1 l'immagine sta nella finestra; poi la larghezza cresce e si scorre. */
    width: calc(100% * var(--zoom));
    box-sizing: border-box;
    display: flex;
  }
  img {
    margin: auto;
    max-width: 100%;
    max-height: calc(100vh - 56px);
    cursor: zoom-in;
    border-radius: 6px;
  }
  img.zoomed {
    max-height: none;
    width: 100%;
    cursor: zoom-out;
  }
  .bar {
    position: fixed;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    border-radius: 999px;
    background: rgba(30, 34, 46, 0.95);
    color: #fff;
    font-size: 12px;
  }
  .bar button {
    border: 0;
    border-radius: 999px;
    padding: 3px 9px;
    background: rgba(255, 255, 255, 0.12);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .bar button:hover {
    background: rgba(255, 255, 255, 0.25);
  }
  .bar span {
    min-width: 40px;
    text-align: center;
  }
</style>
