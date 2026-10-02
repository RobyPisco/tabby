import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { COLORS } from "$lib/notes";

/** Stesse chiavi di `Settings` in src-tauri/src/settings.rs. */
export type Settings = {
  side: "right" | "left";
  note_width: number;
  monitor: string | null;
  editor_font: "hand" | "sans" | "mono";
  editor_size: number;
  new_note_color: string;
  sound: boolean;
  sound_name: string;
  /** Ripete la notifica ogni N minuti finché non è gestita; 0 = mai. */
  repeat_minutes: number;
  theme: "auto" | "light" | "dark";
  /** Trasparenza del vetro delle schede nel deck, 0-100. */
  deck_transparency: number;
  /** Integrazione Google Calendar attiva. */
  gcal_enabled: boolean;
  /** Quanto avanti importare gli eventi: "1w"-"3w" settimane, "1m"-"3m" mesi. */
  gcal_sync_range: string;
  /** ID dei calendari selezionati. */
  gcal_calendar_ids: string[];
  /** Usa il proxy di Windows (PAC, credenziali dell'utente) per le chiamate a Google. */
  gcal_use_proxy: boolean;
  /** Backup automatico giornaliero. */
  backup_enabled: boolean;
  /** Cartella dei backup; `null` = `backups` nella cartella dei dati. */
  backup_dir: string | null;
  backup_keep: number;
  /** "Non disturbare" fino a questo momento (secondi Unix). */
  dnd_until: number | null;
  /** Fascia oraria senza notifiche ("HH:MM"). */
  quiet_enabled: boolean;
  quiet_from: string;
  quiet_to: string;
  /** Ora di "Domani" quando si posticipa ("HH:MM"). */
  tomorrow_time: string;
};

export const FONTS: Record<Settings["editor_font"], { label: string; css: string }> = {
  hand: { label: "A mano (Segoe Print)", css: '"Segoe Print", "Segoe UI", cursive' },
  sans: { label: "Normale (Segoe UI)", css: '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif' },
  mono: { label: "Monospazio (Cascadia)", css: '"Cascadia Mono", Consolas, monospace' },
};

/** Valori reattivi, aggiornati quando un'altra finestra salva. */
export const settings = $state<Settings>({
  side: "right",
  note_width: 340,
  monitor: null,
  editor_font: "hand",
  editor_size: 18,
  new_note_color: "cycle",
  sound: true,
  sound_name: "reminder",
  repeat_minutes: 0,
  theme: "auto",
  deck_transparency: 45,
  gcal_enabled: false,
  gcal_sync_range: "2w",
  gcal_calendar_ids: [],
  gcal_use_proxy: false,
  backup_enabled: true,
  backup_dir: null,
  backup_keep: 7,
  dnd_until: null,
  quiet_enabled: false,
  quiet_from: "22:00",
  quiet_to: "08:00",
  tomorrow_time: "09:00",
});

/** "09:00" → ore e minuti. */
export function splitTime(text: string): [number, number] {
  const [h, m] = text.split(":").map(Number);
  return [h || 0, m || 0];
}

/** Larghezza della finestra del deck: deve restare uguale a `dock_width` in settings.rs. */
export function dockWidth(s: Settings = settings): number {
  return s.note_width + 200;
}

export function colorForNewNote(existing: number): string {
  return settings.new_note_color === "cycle" ? COLORS[existing % COLORS.length] : settings.new_note_color;
}

/** Carattere e dimensione del testo delle note, letti dal tema dell'editor. */
function applyEditorStyle() {
  const root = document.documentElement.style;
  root.setProperty("--editor-font", FONTS[settings.editor_font].css);
  root.setProperty("--editor-size", `${settings.editor_size}px`);
}

function assign(next: Settings) {
  Object.assign(settings, next);
  applyEditorStyle();
}

/** Da chiamare all'avvio di ogni finestra. */
export async function initSettings(): Promise<() => void> {
  assign(await invoke<Settings>("get_settings"));
  const unlisten = await listen<Settings>("settings-changed", ({ payload }) => assign(payload));
  return unlisten;
}

export async function saveSettings(changes: Partial<Settings>) {
  assign(await invoke<Settings>("set_settings", { settings: { ...settings, ...changes } }));
}
