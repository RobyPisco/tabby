<script lang="ts">
  import { vivid } from "$lib/notes";

  /** Foglietto di calendario (mese in alto, giorno sotto) che distingue gli eventi di Google. */
  let { at, color, size = 34 }: { at: number; color: string; size?: number } = $props();

  const date = $derived(new Date(at * 1000));
  const month = $derived(date.toLocaleDateString("it-IT", { month: "short" }).replace(".", ""));
  const label = $derived(date.toLocaleDateString("it-IT", { weekday: "long", day: "numeric", month: "long" }));
</script>

<span class="tile" style:--accent={vivid(color)} style:--size="{size}px" title="Evento di Google Calendar · {label}">
  <span class="month">{month}</span>
  <span class="day">{date.getDate()}</span>
</span>

<style>
  .tile {
    flex: none;
    display: inline-flex;
    flex-direction: column;
    width: var(--size);
    border-radius: calc(var(--size) * 0.2);
    overflow: hidden;
    background: #fff;
    box-shadow:
      0 0 0 1px rgba(0, 0, 0, 0.12),
      0 1px 3px rgba(0, 0, 0, 0.18);
    text-align: center;
    font-family: "Segoe UI", system-ui, sans-serif;
    line-height: 1;
  }
  .month {
    padding: calc(var(--size) * 0.06) 0;
    background: var(--accent);
    color: #fff;
    font-size: calc(var(--size) * 0.26);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .day {
    padding: calc(var(--size) * 0.06) 0 calc(var(--size) * 0.08);
    color: #1d1f24;
    font-size: calc(var(--size) * 0.44);
    font-weight: 700;
  }
</style>
