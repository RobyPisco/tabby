import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type Note = {
  id: number;
  title: string;
  body: string;
  color: string;
  archived: boolean;
  created_at: number;
  updated_at: number;
  /** Momento in cui è finita nel cestino, `null` se non è nel cestino. */
  deleted_at: number | null;
  /** Prossima scadenza del promemoria (già posticipata, se è il caso), `null` se non c'è. */
  remind_at: number | null;
  repeat: Repeat | null;
  pinned: boolean;
  folder_id: number | null;
  /** ID evento Google Calendar; `null` per le note normali. Nota in sola lettura. */
  gcal_event_id: string | null;
};

export type Folder = { id: number; name: string; count: number };
export type FolderCounts = { all: number; unfiled: number };
export type TagCount = { tag: string; count: number };
export type NoteRef = { id: number; title: string };
export type Version = { id: number; title: string; body: string; saved_at: number };

export type Repeat = "none" | "daily" | "weekly" | "monthly";
export type ReminderAction = "snooze10" | "snooze60" | "tomorrow" | "done";

export const REPEAT_LABELS: Record<Repeat, string> = {
  none: "Una volta",
  daily: "Ogni giorno",
  weekly: "Ogni settimana",
  monthly: "Ogni mese",
};

export type Filter = "all" | "active" | "archived" | "trash";

export const COLORS = ["#b5d3f7", "#b3e5cf", "#d4c8f2", "#f7dc82", "#f7c4b5", "#f2b8d4"];
export const COLOR_NAMES: Record<string, string> = {
  "#b5d3f7": "Azzurro",
  "#b3e5cf": "Verde",
  "#d4c8f2": "Lilla",
  "#f7dc82": "Giallo",
  "#f7c4b5": "Pesca",
  "#f2b8d4": "Rosa",
};

/** Versione satura e più scura del colore pastello della nota (pillola, barre colorate). */
export function vivid(color: string): string {
  return `oklch(from ${color} calc(l - 0.12) calc(c * 3 + 0.05) h)`;
}

/** Richiama `reload` quando un'altra finestra modifica le note. */
export function onNotesChangedElsewhere(reload: () => void): Promise<() => void> {
  const self = getCurrentWindow().label;
  return listen<{ source: string }>("notes-changed", ({ payload }) => {
    if (payload.source !== self) reload();
  });
}

/** Tempo trascorso in forma breve: "ora", "23m", "15h", "3g", poi la data. */
export function relativeTime(unixSeconds: number, nowSeconds = Date.now() / 1000): string {
  const diff = Math.max(0, nowSeconds - unixSeconds);
  if (diff < 60) return "ora";
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`;
  if (diff < 7 * 86400) return `${Math.floor(diff / 86400)}g`;
  return new Date(unixSeconds * 1000).toLocaleDateString("it-IT", { day: "numeric", month: "short" });
}

export function fullDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString("it-IT", {
    day: "numeric",
    month: "long",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/** Scadenza in forma breve: "oggi 18:00", "domani 09:00", "lun 09:00", "6 ott 09:00". */
export function formatReminder(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  const time = date.toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" });
  const days = Math.round((startOfDay(date) - startOfDay(new Date())) / 86_400_000);
  if (days === 0) return `oggi ${time}`;
  if (days === 1) return `domani ${time}`;
  if (days === -1) return `ieri ${time}`;
  if (days > 1 && days < 7) return `${date.toLocaleDateString("it-IT", { weekday: "short" })} ${time}`;
  const sameYear = date.getFullYear() === new Date().getFullYear();
  const day = date.toLocaleDateString("it-IT", {
    day: "numeric",
    month: "short",
    year: sameYear ? undefined : "numeric",
  });
  return `${day} ${time}`;
}

export function isOverdue(note: Note, nowSeconds: number): boolean {
  return note.remind_at !== null && note.remind_at <= nowSeconds;
}

export type Template = { label: string; title: string; body: string };

/** Modelli rapidi proposti quando una nota è vuota. */
export const TEMPLATES: Template[] = [
  { label: "Lista spesa", title: "Spesa", body: "- [ ] " },
  { label: "Da fare", title: "Da fare", body: "- [ ] " },
  {
    label: "Riunione",
    title: "Riunione",
    body: "## Partecipanti\n- \n\n## Punti\n- \n\n## Azioni\n- [ ] ",
  },
  { label: "Scaletta", title: "Scaletta", body: "1. \n2. \n3. " },
];

/** Caselle spuntate e totali, per mostrare "2/5" nell'elenco; `null` se la nota non ne ha. */
export function checklistProgress(body: string): { done: number; total: number } | null {
  const boxes = body.match(/^\s*[-*+] \[( |x|X)\]/gm);
  if (!boxes) return null;
  return { done: boxes.filter((b) => /x/i.test(b)).length, total: boxes.length };
}

/** Prima riga di testo senza simboli Markdown, per le anteprime. */
export function previewLine(body: string): string {
  for (const raw of body.split("\n")) {
    const line = raw
      .replace(/!\[[^\]]*\]\([^)]*\)/g, "🖼 immagine")
      .replace(/\[\[([^[\]\n]+)\]\]/g, "$1")
      .replace(/^\s*(#{1,6}\s+|[-*+]\s+\[( |x|X)\]\s*|[-*+]\s+|\d+\.\s+|>\s*)/, "")
      .replace(/(\*\*|__|~~|`)/g, "")
      .trim();
    if (line) return line;
  }
  return "Nessun testo";
}