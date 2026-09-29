<script lang="ts">
  import { clock } from "$lib/clock.svelte";
  import { parseItalianDate } from "$lib/nlDate";
  import { formatReminder, REPEAT_LABELS, type Repeat } from "$lib/notes";

  let {
    remindAt,
    repeat,
    onset,
    onclear,
    open = $bindable(false),
  }: {
    remindAt: number | null;
    repeat: Repeat | null;
    onset: (dueAt: number, repeat: Repeat) => void;
    onclear: () => void;
    open?: boolean;
  } = $props();

  let root = $state<HTMLElement>();
  let bellButton = $state<HTMLButtonElement>();
  let dateInput = $state<HTMLInputElement>();
  let phraseInput = $state<HTMLInputElement>();
  let phrase = $state("");
  const parsed = $derived(phrase.trim() ? parseItalianDate(phrase) : null);
  let value = $state("");
  let chosenRepeat = $state<Repeat>("none");

  const overdue = $derived(remindAt !== null && remindAt <= clock.now);

  /** "YYYY-MM-DDTHH:MM" in ora locale, il formato di <input type="datetime-local">. */
  function toInputValue(date: Date): string {
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
  }

  function at(daysFromToday: number, hour: number): Date {
    const d = new Date();
    d.setDate(d.getDate() + daysFromToday);
    d.setHours(hour, 0, 0, 0);
    return d;
  }

  function nextMonday(hour: number): Date {
    const d = new Date();
    const days = ((8 - d.getDay()) % 7) || 7;
    return at(days, hour);
  }

  /** Scelte rapide; "Stasera" diventa "Domani sera" quando le 18 sono passate. */
  const presets = $derived.by(() => {
    void clock.now;
    const inOneHour = new Date(Date.now() + 3_600_000);
    inOneHour.setSeconds(0, 0);
    const evening = new Date().getHours() < 18 ? { label: "Stasera 18:00", date: at(0, 18) } : { label: "Domani 18:00", date: at(1, 18) };
    return [
      { label: "Tra 1 ora", date: inOneHour },
      evening,
      { label: "Domani 9:00", date: at(1, 9) },
      { label: "Lunedì 9:00", date: nextMonday(9) },
    ];
  });

  $effect(() => {
    if (!open) return;
    // All'apertura parte dalla scadenza attuale o dall'ora piena successiva.
    const start = remindAt !== null ? new Date(remindAt * 1000) : new Date(Math.ceil(Date.now() / 3_600_000) * 3_600_000);
    value = toInputValue(start);
    chosenRepeat = repeat ?? "none";
    phrase = "";
    queueMicrotask(() => phraseInput?.focus());
  });

  function apply(date: Date) {
    open = false;
    onset(Math.floor(date.getTime() / 1000), chosenRepeat);
  }

  function applyInput() {
    const date = new Date(value);
    if (!Number.isNaN(date.getTime())) apply(date);
  }

  function clear() {
    open = false;
    onclear();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation(); // chiude solo il riquadro, non il deck o la finestra
      open = false;
      bellButton?.focus(); // il focus resta nella nota, che quindi non si richiude
    } else if (event.key === "Enter" && event.target === dateInput) {
      applyInput();
    } else if (event.key === "Enter" && event.target === phraseInput && parsed) {
      apply(parsed);
    }
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (open && !root?.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="picker" bind:this={root}>
  <button
    bind:this={bellButton}
    class="bell"
    class:set={remindAt !== null}
    class:overdue
    onclick={() => (open = !open)}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label="Promemoria"
    title="Promemoria"
  >
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <path d="M10 2.5a5 5 0 0 0-5 5v3.2L3.6 13.5h12.8L15 10.7V7.5a5 5 0 0 0-5-5Z" />
      <path d="M8 15.5a2 2 0 0 0 4 0" />
    </svg>
  </button>

  {#if open}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="panel" role="dialog" aria-label="Promemoria" tabindex="-1" onkeydown={onKeydown}>
      <label>
        <span>Quando</span>
        <input
          class="phrase"
          bind:this={phraseInput}
          bind:value={phrase}
          placeholder="es. domani alle 9, tra 2 ore"
          spellcheck="false"
        />
        {#if phrase.trim()}
          <small class:ok={parsed} class:ko={!parsed}>
            {parsed ? `→ ${formatReminder(Math.floor(parsed.getTime() / 1000))} · Invio per impostare` : "Non ho capito la data"}
          </small>
        {/if}
      </label>
      <div class="presets">
        {#each presets as preset (preset.label)}
          <button onclick={() => apply(preset.date)}>{preset.label}</button>
        {/each}
      </div>
      <label>
        <span>Data e ora</span>
        <input type="datetime-local" bind:this={dateInput} bind:value />
      </label>
      <label>
        <span>Ripeti</span>
        <select bind:value={chosenRepeat}>
          {#each Object.entries(REPEAT_LABELS) as [key, label] (key)}
            <option value={key}>{label}</option>
          {/each}
        </select>
      </label>
      <div class="buttons">
        {#if remindAt !== null}
          <button class="remove" onclick={clear}>Rimuovi</button>
        {/if}
        <button class="primary" onclick={applyInput}>Imposta</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    display: flex;
  }
  .bell {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: var(--bell-bg, rgba(0, 0, 0, 0.08));
    color: var(--bell-fg, rgba(0, 0, 0, 0.55));
    cursor: pointer;
  }
  .bell:hover {
    filter: brightness(0.92);
  }
  .bell svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  .bell.set {
    background: var(--bell-set-bg, rgba(0, 0, 0, 0.78));
    color: #fff;
  }
  .bell.set svg {
    fill: currentColor;
  }
  .bell.overdue {
    background: #d93b2b;
    animation: ring 1.6s ease-in-out infinite;
  }
  @keyframes ring {
    0%, 60%, 100% { transform: rotate(0); }
    10%, 30% { transform: rotate(-14deg); }
    20%, 40% { transform: rotate(14deg); }
  }

  .panel {
    position: absolute;
    top: calc(100% + 8px);
    right: -40px;
    z-index: 10;
    width: 270px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: 14px;
    background: #fff;
    color: #1d1d22;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.28);
    font: 13px "Segoe UI", system-ui, sans-serif;
    outline: none;
  }
  small {
    font-size: 12px;
    font-weight: 600;
  }
  small.ok {
    color: #1b7a44;
  }
  small.ko {
    color: #c9372c;
  }
  .presets {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .presets button {
    padding: 6px 4px;
    border: 1px solid #e2e2e8;
    border-radius: 8px;
    background: #f6f6f8;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
    color: inherit;
    cursor: pointer;
  }
  .presets button:hover {
    background: #ececf2;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  label span {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #6b6b76;
  }
  input,
  select {
    padding: 6px 8px;
    border: 1px solid #d6d6de;
    border-radius: 8px;
    background: #fff;
    font: 13px "Segoe UI", system-ui, sans-serif;
    color: inherit;
    color-scheme: light;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .buttons button {
    padding: 6px 14px;
    border-radius: 8px;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
    cursor: pointer;
  }
  .remove {
    margin-right: auto;
    border: 1px solid #e2e2e8;
    background: #fff;
    color: #c9372c;
  }
  .primary {
    border: 0;
    background: #3b6fe0;
    color: #fff;
  }
  button:focus-visible,
  input:focus-visible,
  select:focus-visible {
    outline: 2px solid #3b6fe0;
    outline-offset: 1px;
  }
</style>
