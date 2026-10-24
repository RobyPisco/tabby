import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Stesso formato di `Timer` in src-tauri/src/timer.rs. */
export type Timer = { note_id: number; kind: "focus" | "break"; minutes: number; ends_at: number };

/** Secondi rimanenti di un timer dato l'istante corrente (ms); mai negativo. */
export function secondsLeft(timer: Timer, nowMs: number): number {
  return Math.max(0, Math.ceil(timer.ends_at - nowMs / 1000));
}

/** Un timer attivo, con i secondi rimasti ricalcolati ogni secondo. */
export type ActiveTimer = Timer & { left: number };

/** Timer attivi, uno per nota (chiave: note_id). Più note possono averne uno ciascuna. */
export const timer = $state<{ active: Record<number, ActiveTimer> }>({ active: {} });

let ticker: ReturnType<typeof setInterval> | undefined;

function tick() {
  const nowMs = Date.now();
  for (const id in timer.active) {
    timer.active[id].left = secondsLeft(timer.active[id], nowMs);
  }
}

function assign(timers: Timer[]) {
  const nowMs = Date.now();
  const active: Record<number, ActiveTimer> = {};
  for (const t of timers) active[t.note_id] = { ...t, left: secondsLeft(t, nowMs) };
  timer.active = active;
  clearInterval(ticker);
  ticker = timers.length > 0 ? setInterval(tick, 1000) : undefined;
}

/** Da chiamare all'avvio di ogni finestra che mostra i timer. */
export async function initTimer(): Promise<() => void> {
  assign(await invoke<Timer[]>("timer_get"));
  return listen<Timer[]>("timer-changed", ({ payload }) => assign(payload));
}

/** "4:05", oppure "1:02:30" oltre l'ora. */
export function formatLeft(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = String(seconds % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}
