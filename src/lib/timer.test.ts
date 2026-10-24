import { describe, expect, it } from "vitest";
import { secondsLeft } from "./timer.svelte";

describe("secondsLeft", () => {
  it("restituisce i secondi rimanenti per un timer non ancora scaduto", () => {
    const timer = { note_id: 1, kind: "focus" as const, minutes: 25, ends_at: 1000 };
    expect(secondsLeft(timer, 990_000)).toBe(10);
  });

  it("un timer già scaduto restituisce 0, mai negativo", () => {
    const timer = { note_id: 1, kind: "focus" as const, minutes: 25, ends_at: 1000 };
    expect(secondsLeft(timer, 1_010_000)).toBe(0);
  });

  it("arrotonda per eccesso: 4.1 secondi rimanenti diventano 5", () => {
    const timer = { note_id: 1, kind: "focus" as const, minutes: 25, ends_at: 1000 };
    expect(secondsLeft(timer, 995_900)).toBe(5);
  });
});
