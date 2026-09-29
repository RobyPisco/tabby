<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import type { Folder, TagCount } from "$lib/notes";

  /** `folder`: null = tutte, 0 = senza cartella, altrimenti l'id. */
  let {
    folders,
    tags,
    folder = $bindable(),
    tag = $bindable(),
    onselect,
    onchanged,
    onimport,
    onexportall,
    onsettings,
  }: {
    folders: Folder[];
    tags: TagCount[];
    folder: number | null;
    tag: string | null;
    onselect: () => void;
    onchanged: () => void;
    onimport: () => void;
    onexportall: () => void;
    onsettings: () => void;
  } = $props();

  /** Cartella in modifica: "new" per quella nuova, altrimenti l'id da rinominare. */
  let editing = $state<"new" | number | null>(null);
  let draft = $state("");
  let error = $state("");
  let confirmDeleteId = $state<number | null>(null);
  let input = $state<HTMLInputElement>();

  function pickFolder(value: number | null) {
    folder = value;
    confirmDeleteId = null;
    onselect();
  }

  function pickTag(value: string) {
    tag = tag === value ? null : value;
    onselect();
  }

  async function startEditing(target: "new" | number, name = "") {
    editing = target;
    draft = name;
    error = "";
    await tick();
    input?.focus();
    input?.select();
  }

  async function commit() {
    const name = draft.trim();
    if (!name) {
      editing = null;
      return;
    }
    try {
      if (editing === "new") {
        const id = await invoke<number>("create_folder", { name });
        folder = id;
        onselect();
      } else if (editing !== null) {
        await invoke("rename_folder", { id: editing, name });
      }
      editing = null;
      onchanged();
    } catch (e) {
      error = String(e);
    }
  }

  function onInputKeydown(event: KeyboardEvent) {
    event.stopPropagation(); // Esc chiude solo la modifica, non la finestra
    if (event.key === "Enter") commit();
    else if (event.key === "Escape") editing = null;
  }

  async function remove(id: number) {
    if (confirmDeleteId !== id) {
      confirmDeleteId = id;
      return;
    }
    await invoke("delete_folder", { id });
    confirmDeleteId = null;
    if (folder === id) folder = null;
    onchanged();
    onselect();
  }
</script>

{#snippet editField()}
  <li class="edit">
    <input
      bind:this={input}
      bind:value={draft}
      onkeydown={onInputKeydown}
      onblur={commit}
      placeholder="Nome della cartella"
      maxlength="60"
    />
    {#if error}<span class="error">{error}</span>{/if}
  </li>
{/snippet}

<nav class="sidebar" aria-label="Cartelle e tag">
  <div class="heading">
    <span>Cartelle</span>
    <button class="add" onclick={() => startEditing("new")} title="Nuova cartella" aria-label="Nuova cartella">+</button>
  </div>
  <ul>
    <li>
      <button class="item" class:on={folder === null} onclick={() => pickFolder(null)}>
        <span class="name">Tutte le cartelle</span>
      </button>
    </li>
    <li>
      <button class="item" class:on={folder === 0} onclick={() => pickFolder(0)}>
        <span class="name">Senza cartella</span>
      </button>
    </li>
    {#each folders as f (f.id)}
      {#if editing === f.id}
        {@render editField()}
      {:else}
        <li class="folder">
          <button
            class="item"
            class:on={folder === f.id}
            onclick={() => pickFolder(f.id)}
            ondblclick={() => startEditing(f.id, f.name)}
            title="Doppio clic per rinominare"
          >
            <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M2.5 5.5a1.5 1.5 0 0 1 1.5-1.5h3.6l1.8 2h6.6a1.5 1.5 0 0 1 1.5 1.5v7a1.5 1.5 0 0 1-1.5 1.5H4a1.5 1.5 0 0 1-1.5-1.5Z" /></svg>
            <span class="name">{f.name}</span>
            <span class="count">{f.count}</span>
          </button>
          <button
            class="delete"
            class:confirm={confirmDeleteId === f.id}
            onclick={() => remove(f.id)}
            title="Elimina la cartella (le note restano)"
            aria-label="Elimina la cartella {f.name}"
          >
            {confirmDeleteId === f.id ? "Elimina?" : "✕"}
          </button>
        </li>
      {/if}
    {/each}
    {#if editing === "new"}
      {@render editField()}
    {/if}
  </ul>

  <div class="heading"><span>Tag</span></div>
  {#if tags.length === 0}
    <p class="hint">Scrivi <b>#parola</b> in una nota per creare un tag.</p>
  {:else}
    <div class="tags">
      {#each tags as t (t.tag)}
        <button class="tag" class:on={tag === t.tag} onclick={() => pickTag(t.tag)}>
          #{t.tag} <span>{t.count}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="footer">
    <button class="item" onclick={onimport}>
      <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 3v9m0 0-3.5-3.5M10 12l3.5-3.5M4 14v2.5h12V14" /></svg>
      <span class="name">Importa…</span>
    </button>
    <button class="item" onclick={onexportall}>
      <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 12V3m0 0L6.5 6.5M10 3l3.5 3.5M4 14v2.5h12V14" /></svg>
      <span class="name">Esporta tutto…</span>
    </button>
    <button class="item" onclick={onsettings}>
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r="2.6" />
        <path d="M10 2.5v2M10 15.5v2M17.5 10h-2M4.5 10h-2M15.3 4.7l-1.4 1.4M6.1 13.9l-1.4 1.4M15.3 15.3l-1.4-1.4M6.1 6.1 4.7 4.7" />
      </svg>
      <span class="name">Impostazioni</span>
    </button>
  </div>
</nav>

<style>
  button {
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
    overflow-y: auto;
    padding: 14px 10px;
    border-right: 1px solid var(--line);
    background: var(--bg);
  }
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 8px 4px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .add {
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    font-size: 16px;
    line-height: 1;
  }
  .add:hover {
    background: var(--hover);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    position: relative;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
  }
  .item:hover {
    background: var(--hover);
  }
  .item.on {
    background: var(--selected);
    font-weight: 600;
  }
  .item svg {
    width: 15px;
    height: 15px;
    flex: none;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.6;
    stroke-linejoin: round;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .delete {
    position: absolute;
    right: 4px;
    top: 50%;
    translate: 0 -50%;
    display: none;
    padding: 2px 6px;
    border: 0;
    border-radius: 6px;
    background: var(--pane);
    color: var(--muted);
    font-size: 11px;
  }
  .folder:hover .count {
    visibility: hidden;
  }
  .folder:hover .delete,
  .delete.confirm {
    display: block;
  }
  .delete.confirm {
    background: var(--danger);
    color: #fff;
  }
  .edit {
    padding: 2px 4px;
  }
  .edit input {
    width: 100%;
    box-sizing: border-box;
    padding: 5px 8px;
    border: 1px solid var(--focus);
    border-radius: 8px;
    background: var(--pane);
    color: var(--text);
    font: inherit;
    outline: none;
  }
  .error {
    display: block;
    padding: 4px 4px 0;
    color: var(--danger);
    font-size: 11px;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 2px 6px;
  }
  .tag {
    padding: 3px 8px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--pane);
    font-size: 12px;
    font-weight: 600;
  }
  .tag span {
    color: var(--muted);
    font-weight: 400;
  }
  .tag.on {
    border-color: var(--focus);
    background: var(--focus);
    color: #fff;
  }
  .tag.on span {
    color: rgba(255, 255, 255, 0.8);
  }
  .footer {
    margin-top: auto;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
  .footer svg {
    stroke-linecap: round;
  }
  .hint {
    margin: 0;
    padding: 2px 8px;
    color: var(--muted);
    font-size: 12px;
  }
</style>
