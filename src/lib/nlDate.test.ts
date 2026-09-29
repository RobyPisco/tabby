import { describe, expect, it } from "vitest";
import { parseItalianDate } from "./nlDate";

// Martedì 29 settembre 2026, ore 17:40.
const NOW = new Date(2026, 8, 29, 17, 40);

function at(y: number, m: number, d: number, h: number, min = 0) {
  return new Date(y, m - 1, d, h, min);
}

describe("parseItalianDate", () => {
  it.each([
    ["tra 2 ore", at(2026, 9, 29, 19, 40)],
    ["fra 10 minuti", at(2026, 9, 29, 17, 50)],
    ["tra mezz'ora", at(2026, 9, 29, 18, 10)],
    ["tra un'ora", at(2026, 9, 29, 18, 40)],
    ["domani alle 9", at(2026, 9, 30, 9)],
    ["Domani alle 18:30", at(2026, 9, 30, 18, 30)],
    ["dopodomani mattina", at(2026, 10, 1, 9)],
    ["domani sera", at(2026, 9, 30, 20)],
    ["alle 8 di sera", at(2026, 9, 29, 20)],
    ["alle 21", at(2026, 9, 29, 21)],
    ["alle 9", at(2026, 9, 30, 9)], // già passate oggi → domani
    ["stasera", at(2026, 9, 29, 20)],
    ["lunedì", at(2026, 10, 5, 9)],
    ["lunedì prossimo alle 10", at(2026, 10, 5, 10)],
    ["martedì", at(2026, 10, 6, 9)], // oggi è martedì → la settimana prossima
    ["il 5 ottobre alle 18.15", at(2026, 10, 5, 18, 15)],
    ["3 marzo", at(2027, 3, 3, 9)], // già passato quest'anno → l'anno prossimo
    ["5/10 alle 7", at(2026, 10, 5, 7)],
    ["oggi pomeriggio alle 4", at(2026, 9, 29, 16)],
  ])("%s", (text, expected) => {
    expect(parseItalianDate(text, NOW)).toEqual(expected);
  });

  it.each(["", "boh", "alle 25", "domani forse", "tra poco"])("non capisce «%s»", (text) => {
    expect(parseItalianDate(text, NOW)).toBeNull();
  });
});
