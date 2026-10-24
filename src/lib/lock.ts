import type { Note } from "./notes";

/**
 * Cosa mostrare per una nota: `unlocked` è il contenuto reale già recuperato con
 * `unlock_note` per QUESTA apertura, o `null` se non è stato ancora verificato il PIN.
 * Una nota non protetta, o non ancora sbloccata, resta quella arrivata dal backend
 * (già oscurata lì se protetta, vedi `redact_if_locked` in db.rs).
 */
export function displayedNote(note: Note, unlocked: Note | null): Note {
  if (!note.locked || !unlocked) return note;
  return unlocked;
}
