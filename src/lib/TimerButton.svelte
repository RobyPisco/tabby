<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { formatLeft, timer, type Timer } from "$lib/timer.svelte";

  let { noteId }: { noteId: number } = $props();

  let open = $state(false);
  let root = $state<HTMLElement>();
  let button = $state<HTMLButtonElement>();
  let custom = $state(20);

  const mine = $derived(timer.current?.note_id === noteId ? timer.current : null);
  const elsewhere = $derived(timer.current !== null && timer.current.note_id !== noteId);

  const FOCUS = [15, 25, 50];
  const BREAKS = [5, 15];

  async function start(minutes: number, kind: Timer["kind"]) {
    open = false;
    if (minutes >= 1) await invoke("timer_start", { noteId, minutes: Math.round(minutes), kind });
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation(); // chiude solo il riquadro, non il deck o la finestra
      open = false;
      button?.focus();
    }
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (open && !root?.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="timer" bind:this={root}>
  <button
    bind:this={button}
    class="toggle"
    class:running={mine}
    class:pause={mine?.kind === "break"}
    onmousedown={(e) => e.preventDefault()}
    onclick={() => (open = !open)}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label="Timer"
    title={mine ? `${mine.kind === "focus" ? "Concentrazione" : "Pausa"}: mancano ${formatLeft(timer.left)}` : "Timer / pomodoro"}
  >
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="11" r="6.5" />
      <path d="M10 7.5V11l2 1.5M8 2.5h4" />
    </svg>
    {#if mine}<span class="left">{formatLeft(timer.left)}</span>{/if}
  </button>

  {#if open}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="panel" role="dialog" aria-label="Timer" tabindex="-1" onkeydown={onKeydown}>
      {#if mine}
        <p class="big">{formatLeft(timer.left)}</p>
        <p class="sub">{mine.kind === "focus" ? "🍅 Concentrazione" : "☕ Pausa"} · {mine.minutes} min</p>
        <div class="row">
          <button onclick={() => invoke("timer_extend", { minutes: 5 })}>+5 min</button>
          <button class="stop" onclick={() => { open = false; invoke("timer_stop"); }}>Ferma</button>
        </div>
      {:else}
        {#if elsewhere}
          <p class="note">C'è già un timer su un'altra nota: partendo qui lo sostituisci.</p>
        {/if}
        <span class="label">🍅 Concentrazione</span>
        <div class="row">
          {#each FOCUS as m (m)}
            <button onclick={() => start(m, "focus")}>{m} min</button>
          {/each}
        </div>
        <span class="label">☕ Pausa</span>
        <div class="row">
          {#each BREAKS as m (m)}
            <button onclick={() => start(m, "break")}>{m} min</button>
          {/each}
        </div>
        <span class="label">A scelta</span>
        <form class="row" onsubmit={(e) => { e.preventDefault(); start(custom, "focus"); }}>
          <input type="number" min="1" max="480" bind:value={custom} aria-label="Minuti" />
          <span class="unit">min</span>
          <button class="primary" type="submit">Avvia</button>
        </form>
      {/if}
      <p class="hint">Alla fine arriva una notifica, anche in "Non disturbare".</p>
    </div>
  {/if}
</div>

<style>
  .timer {
    position: relative;
    display: flex;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 26px;
    height: 26px;
    padding: 0 5px;
    box-sizing: border-box;
    justify-content: center;
    border: 0;
    border-radius: 8px;
    background: var(--bell-bg, rgba(0, 0, 0, 0.08));
    color: var(--bell-fg, rgba(0, 0, 0, 0.55));
    cursor: pointer;
  }
  .toggle:hover {
    filter: brightness(0.92);
  }
  .toggle svg {
    flex: none;
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .toggle.running {
    background: #d9482b;
    color: #fff;
  }
  .toggle.running.pause {
    background: #2b8a5a;
  }
  .left {
    font: 700 12px "Segoe UI", system-ui, sans-serif;
    font-variant-numeric: tabular-nums;
  }

  .panel {
    position: absolute;
    top: calc(100% + 8px);
    right: -70px;
    z-index: 10;
    width: 240px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: 14px;
    background: #fff;
    color: #1d1d22;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.28);
    font: 13px "Segoe UI", system-ui, sans-serif;
    outline: none;
  }
  .label {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #6b6b76;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
  }
  .row button {
    flex: 1;
    padding: 6px 4px;
    border: 1px solid #e2e2e8;
    border-radius: 8px;
    background: #f6f6f8;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
    color: inherit;
    cursor: pointer;
  }
  .row button:hover {
    background: #ececf2;
  }
  .row .primary {
    border: 0;
    background: #3b6fe0;
    color: #fff;
  }
  .row .primary:hover {
    background: #3263cc;
  }
  .row .stop {
    color: #c9372c;
  }
  input {
    width: 64px;
    padding: 5px 8px;
    border: 1px solid #d6d6de;
    border-radius: 8px;
    font: 13px "Segoe UI", system-ui, sans-serif;
    color: inherit;
    color-scheme: light;
  }
  .unit {
    color: #6b6b76;
  }
  .big {
    margin: 0;
    font: 700 30px "Segoe UI", system-ui, sans-serif;
    font-variant-numeric: tabular-nums;
    text-align: center;
  }
  .sub {
    margin: -6px 0 4px;
    color: #6b6b76;
    text-align: center;
  }
  .note,
  .hint {
    margin: 0;
    color: #6b6b76;
    font-size: 12px;
  }
  .hint {
    margin-top: 2px;
  }
  button:focus-visible,
  input:focus-visible {
    outline: 2px solid #3b6fe0;
    outline-offset: 1px;
  }
</style>
