/** Posizione verticale trascinabile della pillola/deck sul bordo dx/sx. */

/** Frazione 0.0-1.0 → pixel assoluto del "top" del deck, senza mai avvicinarsi a meno di `margin` dai bordi. */
export function offsetToTop(fraction: number, winHeight: number, deckHeight: number, margin: number): number {
  const minTop = margin;
  const maxTop = Math.max(winHeight - margin - deckHeight, minTop);
  return minTop + fraction * (maxTop - minTop);
}

/** Inverso di `offsetToTop`: pixel → frazione 0.0-1.0, da salvare dopo un trascinamento. */
export function topToOffset(top: number, winHeight: number, deckHeight: number, margin: number): number {
  const minTop = margin;
  const maxTop = Math.max(winHeight - margin - deckHeight, minTop);
  if (maxTop === minTop) return 0;
  return (top - minTop) / (maxTop - minTop);
}

/** Soglia minima di movimento perché un gesto conti come trascinamento invece di hover/clic. */
export const DRAG_THRESHOLD_PX = 4;

export function isDrag(dx: number, dy: number, thresholdPx: number = DRAG_THRESHOLD_PX): boolean {
  return Math.hypot(dx, dy) > thresholdPx;
}

/** Frazione di centro, usata dal doppio clic sulla pillola per ricentrarla. */
export const RECENTER_OFFSET = 0.5;
