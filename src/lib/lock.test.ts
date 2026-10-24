import { describe, expect, it } from "vitest";
import { displayedNote } from "./lock";
import type { Note } from "./notes";

function note(overrides: Partial<Note> = {}): Note {
  return {
    id: 1,
    title: "Titolo",
    body: "Corpo",
    color: "#fff",
    archived: false,
    created_at: 0,
    updated_at: 0,
    deleted_at: null,
    remind_at: null,
    repeat: null,
    pinned: false,
    folder_id: null,
    gcal_event_id: null,
    event_at: null,
    locked: false,
    ...overrides,
  };
}

describe("displayedNote", () => {
  it("restituisce la nota originale quando non è protetta", () => {
    const n = note();
    expect(displayedNote(n, null)).toEqual(n);
  });

  it("nota protetta senza contenuto sbloccato: resta il placeholder arrivato dal backend", () => {
    const n = note({ locked: true, title: "🔒 Nota protetta", body: "" });
    expect(displayedNote(n, null)).toEqual(n);
  });

  it("nota protetta con contenuto sbloccato: restituisce il contenuto reale, non il placeholder", () => {
    const locked = note({ locked: true, title: "🔒 Nota protetta", body: "" });
    const unlocked = note({ locked: true, title: "Segreto", body: "contenuto reale" });
    expect(displayedNote(locked, unlocked)).toEqual(unlocked);
  });
});
