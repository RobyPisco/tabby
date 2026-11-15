<script lang="ts">
  import { tr } from "$lib/i18n";
  import { clock } from "$lib/clock.svelte";
  import { parseDate } from "$lib/nlDate";
  import { formatReminder, isOverdue, REPEAT_LABELS, type Note, type ReminderAction } from "$lib/notes";
  import { settings } from "$lib/settings.svelte";

  let {
    note,
    onaction,
    onsnooze,
    snoozeOpen = $bindable(false),
  }: {
    note: Note;
    onaction: (action: ReminderAction) => void;
    /** Posticipo a un momento scelto (secondi Unix). */
    onsnooze: (until: number) => void;
    snoozeOpen?: boolean;
  } = $props();

  const overdue = $derived(isOverdue(note, clock.now));

  let root = $state<HTMLElement>();
  let phraseInput = $state<HTMLInputElement>();
  let phrase = $state("");
  const parsed = $derived(phrase.trim() ? parseDate(phrase) : null);

  /** Scelte rapide del posticipo; "Stasera" diventa "Domani sera" dopo le 18. */
  const choices = $derived.by(() => {
    void clock.now;
    const now = Date.now();
    const evening = new Date();
    const late = evening.getHours() >= 18;
    if (late) evening.setDate(evening.getDate() + 1);
    evening.setHours(18, 0, 0, 0);
    return [
      { label: "30 min", date: new Date(now + 30 * 60_000) },
      { label: tr("2 ore", "2 hours"), date: new Date(now + 2 * 3_600_000) },
      { label: tr("3 ore", "3 hours"), date: new Date(now + 3 * 3_600_000) },
      { label: late ? tr("Domani 18:00", "Tomorrow 18:00") : tr("Stasera 18:00", "Tonight 18:00"), date: evening },
    ];
  });

  $effect(() => {
    if (!snoozeOpen) return;
    phrase = "";
    queueMicrotask(() => phraseInput?.focus());
  });

  function snooze(date: Date) {
    snoozeOpen = false;
    onsnooze(Math.floor(date.getTime() / 1000));
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation();
      snoozeOpen = false;
    } else if (event.key === "Enter" && parsed) {
      snooze(parsed);
    }
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (snoozeOpen && !root?.contains(event.target as Node)) snoozeOpen = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

{#if note.remind_at !== null}
  <div class="line" class:overdue bind:this={root}>
    <span class="when">
      {overdue ? tr("Scaduto", "Overdue") : tr("Promemoria", "Reminder")} · {formatReminder(note.remind_at)}
      {#if note.repeat && note.repeat !== "none"}
        <!-- Scaduto c'è meno spazio (ci sono i pulsanti): basta il simbolo. -->
        · <span title={REPEAT_LABELS[note.repeat]}>{overdue ? "↻" : REPEAT_LABELS[note.repeat].toLowerCase()}</span>
      {/if}
    </span>
    {#if overdue}
      <span class="actions">
        <button onclick={() => onaction("snooze10")} title={tr("Posticipa di 10 minuti", "Snooze for 10 minutes")}>10 min</button>
        <button onclick={() => onaction("tomorrow")} title={`${tr("Posticipa a domani alle", "Snooze until tomorrow at")} ${settings.tomorrow_time}`}>{tr("Domani", "Tomorrow")}</button>
        <button
          class:on={snoozeOpen}
          onclick={() => (snoozeOpen = !snoozeOpen)}
          aria-expanded={snoozeOpen}
          aria-label={tr("Posticipa a un orario a scelta", "Snooze until a time of your choice")}
          title={tr("Posticipa a un orario a scelta", "Snooze until a time of your choice")}
        >⋯</button>
        <button class="done" onclick={() => onaction("done")}>{tr("Fatto", "Done")}</button>
      </span>
    {/if}
    {#if snoozeOpen}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div class="panel" role="dialog" aria-label={tr("Posticipa", "Snooze")} tabindex="-1" onkeydown={onKeydown}>
        <span class="label">{tr("Posticipa a", "Snooze until")}</span>
        <div class="choices">
          {#each choices as choice (choice.label)}
            <button onclick={() => snooze(choice.date)}>{choice.label}</button>
          {/each}
        </div>
        <input
          bind:this={phraseInput}
          bind:value={phrase}
          placeholder={tr("es. alle 15:30, tra 45 minuti, venerdì", "e.g. at 3:30pm, in 45 minutes, friday")}
          spellcheck="false"
        />
        {#if phrase.trim()}
          <small class:ok={parsed} class:ko={!parsed}>
            {parsed ? `→ ${formatReminder(Math.floor(parsed.getTime() / 1000))} · ${tr("Invio per posticipare", "Enter to snooze")}` : tr("Non ho capito la data", "I didn't understand the date")}
          </small>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .line {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
    padding: 3px 4px 3px 10px;
    border-radius: 8px;
    background: var(--line-bg, rgba(0, 0, 0, 0.07));
    color: var(--line-fg, rgba(0, 0, 0, 0.7));
    font: 600 12px "Segoe UI", system-ui, sans-serif;
  }
  .line.overdue {
    background: #d93b2b;
    color: #fff;
  }
  .when {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    gap: 4px;
  }
  .actions button {
    padding: 2px 8px;
    border: 0;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.22);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .actions button:hover,
  .actions button.on {
    background: rgba(255, 255, 255, 0.35);
  }
  .actions button.done {
    background: #fff;
    color: #b52a1d;
  }

  .panel {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 10;
    width: 250px;
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
  .choices {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .choices button {
    padding: 6px 4px;
    border: 1px solid #e2e2e8;
    border-radius: 8px;
    background: #f6f6f8;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
    color: inherit;
    cursor: pointer;
  }
  .choices button:hover {
    background: #ececf2;
  }
  input {
    padding: 6px 8px;
    border: 1px solid #d6d6de;
    border-radius: 8px;
    background: #fff;
    font: 13px "Segoe UI", system-ui, sans-serif;
    color: inherit;
    color-scheme: light;
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
  button:focus-visible,
  input:focus-visible {
    outline: 2px solid #3b6fe0;
    outline-offset: 1px;
  }
</style>
