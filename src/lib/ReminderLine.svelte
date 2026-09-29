<script lang="ts">
  import { clock } from "$lib/clock.svelte";
  import { formatReminder, isOverdue, REPEAT_LABELS, type Note, type ReminderAction } from "$lib/notes";

  let { note, onaction }: { note: Note; onaction: (action: ReminderAction) => void } = $props();

  const overdue = $derived(isOverdue(note, clock.now));
</script>

{#if note.remind_at !== null}
  <div class="line" class:overdue>
    <span class="when">
      {overdue ? "Scaduto" : "Promemoria"} · {formatReminder(note.remind_at)}
      {#if note.repeat && note.repeat !== "none"}
        <!-- Scaduto c'è meno spazio (ci sono i pulsanti): basta il simbolo. -->
        · <span title={REPEAT_LABELS[note.repeat]}>{overdue ? "↻" : REPEAT_LABELS[note.repeat].toLowerCase()}</span>
      {/if}
    </span>
    {#if overdue}
      <span class="actions">
        <button onclick={() => onaction("snooze10")} title="Posticipa di 10 minuti">10 min</button>
        <button onclick={() => onaction("tomorrow")} title="Posticipa a domani alle 9">Domani</button>
        <button class="done" onclick={() => onaction("done")}>Fatto</button>
      </span>
    {/if}
  </div>
{/if}

<style>
  .line {
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
  button {
    padding: 2px 8px;
    border: 0;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.22);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    background: rgba(255, 255, 255, 0.35);
  }
  button.done {
    background: #fff;
    color: #b52a1d;
  }
</style>
