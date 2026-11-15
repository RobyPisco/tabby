import { settings } from "$lib/settings.svelte";

/**
 * Lingua dell'interfaccia: quella scelta nelle impostazioni, oppure ("auto") quella di
 * Windows, che la webview riporta in `navigator.language`: italiano se è italiano,
 * altrimenti inglese. La stessa regola, lato backend, è in `src-tauri/src/i18n.rs`.
 */
export type Lang = "it" | "en";

export function lang(): Lang {
  if (settings.language === "it" || settings.language === "en") return settings.language;
  const system = typeof navigator === "undefined" ? "it" : navigator.language || "it";
  return system.toLowerCase().startsWith("it") ? "it" : "en";
}

/** Il testo nella lingua attuale (reattivo: cambia subito se si cambia lingua). */
export function tr(it: string, en: string): string {
  return lang() === "en" ? en : it;
}

/** Locale per date e numeri. */
export function locale(): string {
  return lang() === "en" ? "en-GB" : "it-IT";
}

/** "1 nota" / "3 note". */
export function plural(n: number, it: [string, string], en: [string, string]): string {
  const [one, many] = lang() === "en" ? en : it;
  return `${n} ${n === 1 ? one : many}`;
}
