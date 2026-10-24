import { describe, expect, it } from "vitest";
import { DRAG_THRESHOLD_PX, isDrag, offsetToTop, RECENTER_OFFSET, topToOffset } from "./deckPosition";

describe("offsetToTop", () => {
  it("fraction 0.5 centra il deck nello spazio disponibile", () => {
    // winHeight 800, deckHeight 200, margin 80 → spazio utile 80..520, centro 300.
    expect(offsetToTop(0.5, 800, 200, 80)).toBe(300);
  });

  it("deck troppo alto per i margini: resta ancorato al margine superiore", () => {
    // winHeight 300, deckHeight 250, margin 80 → maxTop (300-80-250=-30) < minTop (80).
    expect(offsetToTop(0, 300, 250, 80)).toBe(80);
    expect(offsetToTop(1, 300, 250, 80)).toBe(80);
  });

  it("fraction 0 e 1 restano esattamente ai margini, senza sforarli", () => {
    // winHeight 800, deckHeight 200, margin 80 → spazio utile 80..520.
    expect(offsetToTop(0, 800, 200, 80)).toBe(80);
    expect(offsetToTop(1, 800, 200, 80)).toBe(520);
  });

  it("deck che riempie esattamente lo spazio tra i margini: un solo punto possibile", () => {
    // winHeight 360, deckHeight 200, margin 80 → maxTop (360-80-200=80) == minTop (80).
    expect(offsetToTop(0, 360, 200, 80)).toBe(80);
    expect(offsetToTop(1, 360, 200, 80)).toBe(80);
  });
});

describe("topToOffset", () => {
  it("è l'inverso di offsetToTop nei casi normali (round-trip)", () => {
    const top = offsetToTop(0.25, 800, 200, 80);
    expect(topToOffset(top, 800, 200, 80)).toBeCloseTo(0.25);
  });

  it("deck troppo alto per i margini: la frazione è sempre 0 (minTop == maxTop)", () => {
    expect(topToOffset(80, 300, 250, 80)).toBe(0);
  });

  it("non clampa l'output: un top fuori dai margini dà una frazione fuori 0-1 (responsabilità del chiamante)", () => {
    // winHeight 800, deckHeight 200, margin 80 → spazio utile 80..520; top=50 è sopra il margine.
    expect(topToOffset(50, 800, 200, 80)).toBeLessThan(0);
  });
});

describe("isDrag", () => {
  it("nessun movimento non è un drag", () => {
    expect(isDrag(0, 0)).toBe(false);
  });

  it("movimento sotto soglia su un solo asse non è un drag", () => {
    expect(isDrag(DRAG_THRESHOLD_PX - 1, 0)).toBe(false);
  });

  it("movimento sopra soglia su un solo asse è un drag", () => {
    expect(isDrag(DRAG_THRESHOLD_PX + 1, 0)).toBe(true);
  });

  it("movimento diagonale valuta la distanza combinata, non un asse solo", () => {
    // dx=3, dy=3 → distanza ≈4.24: supera la soglia di 4px anche se nessun asse la supera da solo.
    expect(isDrag(3, 3, 4)).toBe(true);
  });

  it("accetta una soglia personalizzata", () => {
    expect(isDrag(5, 0, 10)).toBe(false);
  });
});

describe("RECENTER_OFFSET", () => {
  it("è il centro (0.5), usato dal doppio clic per ricentrare la pillola", () => {
    expect(RECENTER_OFFSET).toBe(0.5);
  });
});
