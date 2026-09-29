/** Ora corrente (secondi Unix) condivisa e reattiva: aggiorna i promemoria scaduti e i tempi relativi. */
export const clock = $state({ now: Date.now() / 1000 });

setInterval(() => (clock.now = Date.now() / 1000), 15_000);
