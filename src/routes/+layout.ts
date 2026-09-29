// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const ssr = false;

// Il menu del clic destro della webview (Indietro, Aggiorna, Stampa…) non ha senso in un'app:
// resta solo nei campi di testo e nelle note, dove serve per copiare e incollare.
if (typeof document !== "undefined") {
  document.addEventListener("contextmenu", (event) => {
    const target = event.target as HTMLElement | null;
    if (!target?.closest("input, textarea, [contenteditable='true']")) event.preventDefault();
  });
}
