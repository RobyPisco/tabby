<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, tick } from "svelte";
  import { previewLine, relativeTime, vivid, type Note } from "$lib/notes";
  import { initSettings, settings } from "$lib/settings.svelte";

  const MAX_RESULTS = 8;

  let query = $state("");
  let results = $state<Note[]>([]);
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let systemDark = $state(window.matchMedia("(prefers-color-scheme: dark)").matches);
  const dark = $derived(settings.theme === "auto" ? systemDark : settings.theme === "dark");
  let searchId = 0;

  async function search() {
    const id = ++searchId;
    const found = await invoke<Note[]>("search_notes", { query, filter: "all", folder: null, tag: null });
    if (id !== searchId) return; // è già partita una ricerca più recente
    results = found.slice(0, MAX_RESULTS);
    active = 0;
  }

  async function hide() {
    await getCurrentWindow().hide();
  }

  async function openNote(note: Note | undefined) {
    if (!note) return;
    await invoke("open_from_search", { id: note.id, inDeck: !note.archived });
    await hide();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      hide();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      active = Math.min(active + 1, results.length - 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      active = Math.max(active - 1, 0);
    } else if (event.key === "Enter") {
      event.preventDefault();
      openNote(results[active]);
    }
  }

  onMount(() => {
    const settingsReady = initSettings();
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onScheme = (e: MediaQueryListEvent) => (systemDark = e.matches);
    media.addEventListener("change", onScheme);
    const unlistenShown = listen("search-shown", async () => {
      query = "";
      await search();
      await tick();
      input?.focus();
    });
    // Clic fuori: la barra sparisce, come il menu Start.
    const unlistenBlur = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused) hide();
    });
    search();
    return () => {
      media.removeEventListener("change", onScheme);
      settingsReady.then((fn) => fn());
      unlistenShown.then((fn) => fn());
      unlistenBlur.then((fn) => fn());
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="box" class:dark>
  <div class="field">
    <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m13 13 4.5 4.5" /></svg>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      bind:this={input}
      bind:value={query}
      oninput={search}
      placeholder="Cerca nelle note…"
      aria-label="Cerca nelle note"
      autofocus
      spellcheck="false"
    />
    <kbd>Esc</kbd>
  </div>
  {#if results.length > 0}
    <ul role="listbox" aria-label="Risultati">
      {#each results as note, i (note.id)}
        <li role="option" aria-selected={i === active}>
          <button class:on={i === active} onmouseenter={() => (active = i)} onclick={() => openNote(note)}>
            <span class="bar" style:background={vivid(note.color)}></span>
            <span class="text">
              <span class="title">{note.locked ? "🔒 " : ""}{note.title || "Senza titolo"}</span>
              {#if !note.locked}<span class="preview">{previewLine(note.body)}</span>{/if}
            </span>
            <span class="meta">{note.archived ? "Archiviata · " : ""}{relativeTime(note.updated_at)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty">{query.trim() ? "Nessuna nota trovata" : "Nessuna nota"}</p>
  {/if}
  <p class="hint">↑ ↓ per scegliere · Invio per aprire</p>
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }
  .box {
    --pane: #ffffff;
    --text: #1d1d1f;
    --muted: #6e6e73;
    --line: rgba(0, 0, 0, 0.08);
    --on: rgba(0, 120, 212, 0.1);
    box-sizing: border-box;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--pane);
    color: var(--text);
    font: 14px/1.35 "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
    border: 1px solid var(--line);
    border-radius: 12px;
    overflow: hidden;
  }
  .box.dark {
    --pane: #202124;
    --text: #f1f1f3;
    --muted: #a1a1a8;
    --line: rgba(255, 255, 255, 0.1);
    --on: rgba(96, 165, 250, 0.16);
  }
  .field {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--line);
  }
  .field svg {
    flex: none;
    width: 20px;
    height: 20px;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.8;
    stroke-linecap: round;
  }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 18px;
  }
  kbd {
    font: 11px "Segoe UI", sans-serif;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 1px 6px;
  }
  ul {
    flex: 1;
    margin: 0;
    padding: 6px;
    list-style: none;
    overflow-y: auto;
  }
  li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  li button.on {
    background: var(--on);
  }
  .bar {
    flex: none;
    width: 4px;
    align-self: stretch;
    border-radius: 2px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title,
  .preview {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-weight: 600;
  }
  .preview,
  .meta,
  .empty,
  .hint {
    color: var(--muted);
    font-size: 12px;
  }
  .meta {
    flex: none;
  }
  .empty {
    flex: 1;
    margin: 0;
    padding: 18px;
    text-align: center;
  }
  .hint {
    margin: 0;
    padding: 6px 14px 8px;
    border-top: 1px solid var(--line);
  }
</style>
