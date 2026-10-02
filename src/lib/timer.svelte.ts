import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Stesso formato di `Timer` in src-tauri/src/timer.rs. */
export type Timer = { note_id: number; kind: "focus" | "break"; minutes: number; ends_at: number };

/** Timer in corso (al massimo uno) e secondi rimasti, aggiornati ogni secondo mentre gira. */
export const timer = $state<{ current: Timer | null; left: number }>({ current: null, left: 0 });

let ticker: ReturnType<typeof setInterval> | undefined;

function tick() {
  timer.left = timer.current ? Math.max(0, Math.ceil(timer.current.ends_at - Date.now() / 1000)) : 0;
}

function assign(next: Timer | null) {
  timer.current = next;
  tick();
  clearInterval(ticker);
  ticker = next ? setInterval(tick, 1000) : undefined;
}

/** Da chiamare all'avvio di ogni finestra che mostra il timer. */
export async function initTimer(): Promise<() => void> {
  assign(await invoke<Timer | null>("timer_get"));
  return listen<Timer | null>("timer-changed", ({ payload }) => assign(payload));
}

/** "4:05", oppure "1:02:30" oltre l'ora. */
export function formatLeft(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = String(seconds % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}
