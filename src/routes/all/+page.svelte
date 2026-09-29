<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import { clock } from "$lib/clock.svelte";
  import ColorPicker from "$lib/ColorPicker.svelte";
  import NoteEditor from "$lib/NoteEditor.svelte";
  import ReminderLine from "$lib/ReminderLine.svelte";
  import ReminderPicker from "$lib/ReminderPicker.svelte";
  import SettingsPanel from "$lib/SettingsPanel.svelte";
  import { colorForNewNote, initSettings, settings } from "$lib/settings.svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import Templates from "$lib/Templates.svelte";
  import VersionsPanel from "$lib/VersionsPanel.svelte";
  import {
    checklistProgress,
    formatReminder,
    fullDate,
    isOverdue,
    onNotesChangedElsewhere,
    previewLine,
    relativeTime,
    vivid,
    type Filter,
    type Folder,
    type FolderCounts,
    type Note,
    type NoteRef,
    type ReminderAction,
    type Repeat,
    type TagCount,
    type Template,
  } from "$lib/notes";

  const SEARCH_DEBOUNCE_MS = 120;
  const SAVE_DEBOUNCE_MS = 250;
  const FILTERS: { value: Filter; label: string }[] = [
    { value: "all", label: "Tutte" },
    { value: "active", label: "Attive" },
    { value: "archived", label: "Archiviate" },
    { value: "trash", label: "Cestino" },
  ];
  const TRASH_RETENTION_DAYS = 30; // come in db.rs

  let notes = $state<Note[]>([]);
  let query = $state("");
  let filter = $state<Filter>("all");
  let selectedId = $state<number | null>(null);
  let checked = $state<number[]>([]);
  let folders = $state<Folder[]>([]);
  let counts = $state<FolderCounts>({ all: 0, unfiled: 0 });
  let tags = $state<TagCount[]>([]);
  /** null = tutte le cartelle, 0 = senza cartella, altrimenti l'id. */
  let folder = $state<number | null>(null);
  let tag = $state<string | null>(null);
  let showVersions = $state(false);
  let showSettings = $state(false);
  /** Messaggio breve in basso (export, import, errori). */
  let notice = $state<{ text: string; error: boolean } | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  let systemDark = $state(window.matchMedia("(prefers-color-scheme: dark)").matches);
  const dark = $derived(settings.theme === "dark" || (settings.theme === "auto" && systemDark));
  let backlinks = $state<NoteRef[]>([]);
  /** Cambia a ogni ricaricamento, per aggiornare i link entranti. */
  let refreshKey = $state(0);
  let confirmDelete = $state(false);
  let confirmEmpty = $state(false);
  let searchInput = $state<HTMLInputElement>();
  let titleInput = $state<HTMLInputElement>();
  let editor = $state<NoteEditor>();
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  const selected = $derived(notes.find((n) => n.id === selectedId) ?? null);
  /** Le azioni agiscono sulle note spuntate o, se nessuna, su quella selezionata. */
  const targets = $derived(checked.length > 0 ? checked : selectedId !== null ? [selectedId] : []);
  const allChecked = $derived(notes.length > 0 && checked.length === notes.length);
  const inTrash = $derived(filter === "trash");

  async function reload() {
    await flushSave();
    const [list, folderList, folderCounts, tagList] = await Promise.all([
      invoke<Note[]>("search_notes", { query, filter, folder, tag }),
      invoke<Folder[]>("list_folders"),
      invoke<FolderCounts>("folder_counts"),
      invoke<TagCount[]>("list_tags"),
    ]);
    notes = list;
    folders = folderList;
    counts = folderCounts;
    tags = tagList;
    refreshKey++;
    checked = checked.filter((id) => notes.some((n) => n.id === id));
    if (!notes.some((n) => n.id === selectedId)) selectedId = notes[0]?.id ?? null;
  }

  function onSearchInput() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(reload, SEARCH_DEBOUNCE_MS);
  }

  function setFilter(value: Filter) {
    filter = value;
    checked = [];
    confirmDelete = false;
    confirmEmpty = false;
    reload();
  }

  async function flushSave() {
    if (saveTimer === undefined) return;
    clearTimeout(saveTimer);
    saveTimer = undefined;
    await saveSelected();
  }

  async function saveSelected() {
    const note = notes.find((n) => n.id === selectedId);
    if (note) await invoke("update_note", { id: note.id, title: note.title, body: note.body });
  }

  function scheduleSave() {
    if (selected) selected.updated_at = Math.floor(Date.now() / 1000);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      saveSelected();
    }, SAVE_DEBOUNCE_MS);
  }

  async function select(id: number) {
    if (id === selectedId) return;
    await flushSave();
    selectedId = id;
    confirmDelete = false;
    showVersions = false;
  }

  function toggleChecked(id: number) {
    checked = checked.includes(id) ? checked.filter((c) => c !== id) : [...checked, id];
    confirmDelete = false;
  }

  function toggleAll() {
    checked = allChecked ? [] : notes.map((n) => n.id);
    confirmDelete = false;
  }

  async function setArchived(archived: boolean) {
    if (targets.length === 0) return;
    await flushSave();
    await invoke("set_archived", { ids: targets, archived });
    checked = [];
    await reload();
  }

  /** Fuori dal cestino "Elimina" sposta nel cestino; dentro elimina per sempre, con conferma. */
  async function deleteTargets() {
    if (targets.length === 0) return;
    if (!inTrash) {
      await flushSave();
      await invoke("set_trashed", { ids: targets, trashed: true });
    } else if (!confirmDelete) {
      confirmDelete = true;
      return;
    } else {
      await invoke("delete_notes", { ids: targets });
      confirmDelete = false;
    }
    checked = [];
    await reload();
  }

  async function restoreFromTrash() {
    if (targets.length === 0) return;
    await invoke("set_trashed", { ids: targets, trashed: false });
    checked = [];
    await reload();
  }

  async function emptyTrash() {
    if (!confirmEmpty) {
      confirmEmpty = true;
      return;
    }
    await invoke("empty_trash");
    confirmEmpty = false;
    await reload();
  }

  function setColor(color: string) {
    if (!selected) return;
    selected.color = color;
    invoke("set_color", { id: selected.id, color });
  }

  function setBody(body: string) {
    if (!selected) return;
    selected.body = body;
    scheduleSave();
  }

  async function applyTemplate(template: Template) {
    if (!selected) return;
    if (!selected.title.trim()) selected.title = template.title;
    setBody(template.body);
    await tick();
    editor?.focus(true);
  }

  function setReminder(dueAt: number, repeat: Repeat) {
    if (!selected) return;
    selected.remind_at = dueAt;
    selected.repeat = repeat;
    invoke("set_reminder", { noteId: selected.id, dueAt, repeat });
  }

  function clearReminder() {
    if (!selected) return;
    selected.remind_at = null;
    selected.repeat = null;
    invoke("clear_reminder", { noteId: selected.id });
  }

  async function reminderAction(action: ReminderAction) {
    if (!selected) return;
    await invoke("reminder_action", { noteId: selected.id, action });
    await reload();
  }

  async function togglePin() {
    if (!selected) return;
    await invoke("set_pinned", { id: selected.id, pinned: !selected.pinned });
    await reload();
  }

  async function moveToFolder(ids: number[], folderId: number | null) {
    if (ids.length === 0) return;
    await flushSave();
    await invoke("set_folder", { ids, folderId });
    checked = [];
    await reload();
  }

  /** Mostra una nota qualsiasi, togliendo i filtri che la nasconderebbero. */
  async function showNote(id: number) {
    await flushSave();
    const visible = notes.some((n) => n.id === id);
    if (!visible) {
      query = "";
      filter = "all";
      folder = null;
      tag = null;
      await reload();
    }
    await select(id);
    document.getElementById(`note-${id}`)?.scrollIntoView({ block: "nearest" });
  }

  /** Clic su [[titolo]]: apre la nota, o la crea se non esiste. */
  async function openLink(title: string) {
    const found = await invoke<Note | null>("find_note_by_title", { title });
    if (found) return showNote(found.id);
    const note = await createNote(title.trim(), "");
    await showNote(note.id);
    editor?.focus();
  }

  async function createNote(title: string, body: string): Promise<Note> {
    await flushSave();
    const count = (await invoke<Note[]>("list_notes")).length;
    const note = await invoke<Note>("create_note", { color: colorForNewNote(count) });
    if (title || body) await invoke("update_note", { id: note.id, title, body });
    if (folder !== null && folder !== 0) await invoke("set_folder", { ids: [note.id], folderId: folder });
    return note;
  }

  // Anche la barra del titolo di Windows segue il tema scelto.
  $effect(() => {
    getCurrentWindow().setTheme(settings.theme === "auto" ? null : settings.theme);
  });

  $effect(() => {
    void refreshKey;
    const id = selectedId;
    if (id === null) {
      backlinks = [];
      return;
    }
    invoke<NoteRef[]>("backlinks", { id }).then((list) => {
      if (selectedId === id) backlinks = list;
    });
  });

  function flash(text: string, error = false) {
    clearTimeout(noticeTimer);
    notice = { text, error };
    noticeTimer = setTimeout(() => (notice = null), error ? 8000 : 4000);
  }

  /** Esegue un'azione di export/import mostrando l'esito o l'errore. */
  async function attempt(action: () => Promise<string | null>) {
    try {
      const message = await action();
      if (message) flash(message);
    } catch (e) {
      flash(`Operazione non riuscita: ${e}`, true);
    }
  }

  const NOTE_FILTERS = [
    { name: "Markdown", extensions: ["md"] },
    { name: "Testo", extensions: ["txt"] },
  ];

  function formatOf(path: string): "md" | "txt" {
    return path.toLowerCase().endsWith(".txt") ? "txt" : "md";
  }

  /** "Esporta…" della nota selezionata: un file .md o .txt. */
  function exportSelected() {
    attempt(async () => {
      if (!selected) return null;
      await flushSave();
      const name = (selected.title.trim() || "Nota").replace(/[<>:"/\\|?*]/g, "_");
      const path = await save({ defaultPath: `${name}.md`, filters: NOTE_FILTERS });
      if (!path) return null;
      await invoke("export_combined", { ids: [selected.id], path, format: formatOf(path) });
      return `Esportata in ${path}`;
    });
  }

  /** Un file per nota in una cartella scelta (con le immagini in "media"). */
  function exportToFolder(ids: number[]) {
    attempt(async () => {
      if (ids.length === 0) return null;
      await flushSave();
      const dir = await open({ directory: true, title: "Cartella in cui esportare le note" });
      if (typeof dir !== "string") return null;
      const count = await invoke<number>("export_notes", { ids, dir, format: "md" });
      return `${count} ${count === 1 ? "nota esportata" : "note esportate"} in ${dir}`;
    });
  }

  /** Tutte le note scelte in un unico file. */
  function exportToSingleFile(ids: number[]) {
    attempt(async () => {
      if (ids.length === 0) return null;
      await flushSave();
      const path = await save({ defaultPath: "Note.md", filters: NOTE_FILTERS });
      if (!path) return null;
      await invoke("export_combined", { ids, path, format: formatOf(path) });
      return `${ids.length} note esportate in ${path}`;
    });
  }

  async function exportAll() {
    const all = await invoke<Note[]>("search_notes", { query: "", filter: "all", folder: null, tag: null });
    exportToFolder(all.map((n) => n.id));
  }

  function importNotes() {
    attempt(async () => {
      const paths = await open({
        multiple: true,
        title: "Note da importare",
        filters: [{ name: "Note", extensions: ["md", "markdown", "txt"] }],
      });
      if (!paths || paths.length === 0) return null;
      const count = await invoke<number>("import_files", { paths });
      await reload();
      return `${count} ${count === 1 ? "nota importata" : "note importate"}`;
    });
  }

  function daysLeft(note: Note): number {
    const elapsed = (Date.now() / 1000 - (note.deleted_at ?? 0)) / 86400;
    return Math.max(0, Math.ceil(TRASH_RETENTION_DAYS - elapsed));
  }

  async function newNote() {
    // Con un tag selezionato la nota nuova lo contiene già, così resta visibile.
    const note = await createNote("", tag ? `#${tag} ` : "");
    query = "";
    if (filter === "archived" || filter === "trash") filter = "all";
    await reload();
    selectedId = note.id;
    await tick();
    titleInput?.focus();
  }

  /** Frecce su/giù nel campo di ricerca per scorrere l'elenco. */
  function moveSelection(step: number) {
    if (notes.length === 0) return;
    const index = notes.findIndex((n) => n.id === selectedId);
    const next = Math.min(notes.length - 1, Math.max(0, index + step));
    select(notes[next].id);
    document.getElementById(`note-${notes[next].id}`)?.scrollIntoView({ block: "nearest" });
  }

  function onSearchKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      moveSelection(event.key === "ArrowDown" ? 1 : -1);
    }
  }

  async function onKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey && event.key.toLowerCase() === "f") || (event.ctrlKey && event.key.toLowerCase() === "k")) {
      event.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    } else if (event.ctrlKey && event.key.toLowerCase() === "n") {
      event.preventDefault();
      newNote();
    } else if (event.key === "Escape") {
      if (confirmDelete || confirmEmpty) {
        confirmDelete = false;
        confirmEmpty = false;
      } else if (query !== "" && document.activeElement === searchInput) {
        query = "";
        reload();
      } else {
        await flushSave();
        getCurrentWindow().hide();
      }
    }
  }


  onMount(() => {
    reload();
    const unlistenChanges = onNotesChangedElsewhere(reload);
    // Link dal deck a una nota archiviata.
    const unlistenSelect = listen<number>("select-note", ({ payload }) => showNote(payload));
    const settingsReady = initSettings();
    const unlistenSettings = listen("open-settings", () => (showSettings = true));
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onScheme = (e: MediaQueryListEvent) => (systemDark = e.matches);
    media.addEventListener("change", onScheme);
    const unlistenShown = listen("all-notes-shown", async () => {
      await reload();
      searchInput?.focus();
      searchInput?.select();
    });
    return () => {
      unlistenChanges.then((fn) => fn());
      unlistenShown.then((fn) => fn());
      unlistenSelect.then((fn) => fn());
      unlistenSettings.then((fn) => fn());
      settingsReady.then((fn) => fn());
      media.removeEventListener("change", onScheme);
      flushSave();
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app" class:dark>
  <Sidebar
    {folders}
    {counts}
    {tags}
    bind:folder
    bind:tag
    onselect={reload}
    onchanged={reload}
    onimport={importNotes}
    onexportall={exportAll}
    onsettings={() => (showSettings = true)}
  />

  <aside class="list-pane">
    <div class="search">
      <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m13 13 4 4" /></svg>
      <input
        bind:this={searchInput}
        bind:value={query}
        oninput={onSearchInput}
        onkeydown={onSearchKeydown}
        placeholder="Cerca nelle note…"
        spellcheck="false"
      />
      <span class="count">{notes.length} {notes.length === 1 ? "nota" : "note"}</span>
    </div>

    <div class="toolbar">
      <div class="filters" role="tablist">
        {#each FILTERS as f (f.value)}
          <button role="tab" aria-selected={filter === f.value} class:on={filter === f.value} onclick={() => setFilter(f.value)}>
            {f.label}
          </button>
        {/each}
      </div>
      {#if inTrash}
        <button
          class="new danger-fill"
          class:confirm={confirmEmpty}
          onclick={emptyTrash}
          disabled={notes.length === 0}
        >
          {confirmEmpty ? "Conferma" : "Svuota"}
        </button>
      {:else}
        <button class="new" onclick={newNote} title="Nuova nota (Ctrl+N)">+ Nuova</button>
      {/if}
    </div>

    {#if checked.length > 0}
      <div class="bulk">
        <label class="check">
          <input type="checkbox" checked={allChecked} onchange={toggleAll} />
        </label>
        <span>{checked.length} {checked.length === 1 ? "selezionata" : "selezionate"}</span>
        <span class="spacer"></span>
        {#if inTrash}
          <button onclick={restoreFromTrash}>Ripristina</button>
          <button class="danger" class:confirm={confirmDelete} onclick={deleteTargets}>
            {confirmDelete ? "Conferma" : "Elimina per sempre"}
          </button>
        {:else}
          <select
            class="move"
            aria-label="Sposta in una cartella"
            onchange={(e) => {
              const value = e.currentTarget.value;
              e.currentTarget.value = "";
              moveToFolder(checked, value === "none" ? null : Number(value));
            }}
          >
            <option value="" disabled selected>Sposta in…</option>
            <option value="none">Senza cartella</option>
            {#each folders as f (f.id)}
              <option value={f.id}>{f.name}</option>
            {/each}
          </select>
          <select
            class="move"
            aria-label="Esporta le note selezionate"
            onchange={(e) => {
              const how = e.currentTarget.value;
              e.currentTarget.value = "";
              if (how === "folder") exportToFolder(checked);
              else if (how === "file") exportToSingleFile(checked);
            }}
          >
            <option value="" disabled selected>Esporta…</option>
            <option value="folder">Un file per nota…</option>
            <option value="file">Tutte in un file…</option>
          </select>
          <button onclick={() => setArchived(true)}>Archivia</button>
          <button onclick={() => setArchived(false)}>Ripristina</button>
          <button class="danger" onclick={deleteTargets}>Elimina</button>
        {/if}
      </div>
    {/if}

    <ul class="list">
      {#each notes as note (note.id)}
        <li id="note-{note.id}" class:selected={note.id === selectedId} class:archived={note.archived}>
          <label class="check">
            <input type="checkbox" checked={checked.includes(note.id)} onchange={() => toggleChecked(note.id)} />
          </label>
          <button class="row" onclick={() => select(note.id)}>
            <span class="bar" style:background={vivid(note.color)}></span>
            <span class="text">
              <span class="title">
                {#if note.pinned}
                  <svg class="pin-icon" viewBox="0 0 20 20" aria-label="Fissata"><path d="M7.5 2.5h5l-.8 4.2 2.8 2.8v1.5H5.5V9.5l2.8-2.8-.8-4.2ZM10 11v6.5" /></svg>
                {/if}
                {note.title || "Senza titolo"}
              </span>
              <span class="preview">
                {#if checklistProgress(note.body)}
                  {@const progress = checklistProgress(note.body)!}
                  <span class="progress" class:complete={progress.done === progress.total}>
                    ☑ {progress.done}/{progress.total}
                  </span>
                {/if}
                {previewLine(note.body)}
              </span>
            </span>
            <span class="meta">
              {#if note.deleted_at !== null}
                <span class="badge off">Cestino</span>
              {:else}
                <span class="badge" class:off={note.archived}>{note.archived ? "Archiviata" : "Attiva"}</span>
              {/if}
              {#if note.remind_at !== null && note.deleted_at === null}
                <span class="remind" class:overdue={isOverdue(note, clock.now)}>
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <path d="M10 2.5a5 5 0 0 0-5 5v3.2L3.6 13.5h12.8L15 10.7V7.5a5 5 0 0 0-5-5Zm-2 13a2 2 0 0 0 4 0Z" />
                  </svg>
                  {formatReminder(note.remind_at)}
                </span>
              {:else}
                <span class="time">{relativeTime(note.updated_at, clock.now)}</span>
              {/if}
            </span>
          </button>
        </li>
      {:else}
        <li class="empty">
          {#if query}
            Nessuna nota trovata per “{query}”.
          {:else if tag}
            Nessuna nota con #{tag} qui.
          {:else if folder !== null}
            Nessuna nota in questa cartella.
          {:else if filter === "archived"}
            L'archivio è vuoto.
          {:else if inTrash}
            Il cestino è vuoto.
          {:else}
            Nessuna nota. Creane una con <b>+ Nuova</b>.
          {/if}
        </li>
      {/each}
    </ul>
  </aside>

  <main class="detail-pane">
    {#if selected}
      {#key selected.id}
        <article class="detail" style:--note={selected.color} style:--accent={vivid(selected.color)}>
          <header>
            <div class="status">
              <span class="dot"></span>
              {#if selected.deleted_at !== null}
                Nel cestino
              {:else}
                {selected.archived ? "Archiviata" : "Attiva · nel deck"}
                <select
                  class="folder-select"
                  aria-label="Cartella"
                  value={selected.folder_id ?? ""}
                  onchange={(e) =>
                    moveToFolder([selected!.id], e.currentTarget.value === "" ? null : Number(e.currentTarget.value))}
                >
                  <option value="">Senza cartella</option>
                  {#each folders as f (f.id)}
                    <option value={f.id}>{f.name}</option>
                  {/each}
                </select>
              {/if}
            </div>
            <div class="actions">
              {#if selected.deleted_at !== null}
                <button onclick={restoreFromTrash} disabled={checked.length > 0}>Ripristina</button>
                <button
                  class="danger"
                  class:confirm={confirmDelete}
                  onclick={deleteTargets}
                  disabled={checked.length > 0}
                >
                  {confirmDelete ? "Conferma eliminazione" : "Elimina per sempre"}
                </button>
              {:else}
                <button
                  class="icon-btn"
                  onmousedown={(e) => e.preventDefault()}
                  onclick={() => editor?.toggleChecklist()}
                  aria-label="Casella da spuntare"
                  title="Casella da spuntare (Ctrl+L)"
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
              <rect x="3" y="3" width="14" height="14" rx="3.5" />
              <path d="m6.5 10.2 2.4 2.4 4.6-5" />
            </svg>
                </button>
                <button
                  class="icon-btn"
                  class:on={selected.pinned}
                  onclick={togglePin}
                  aria-pressed={selected.pinned}
                  aria-label="Fissa in alto"
                  title={selected.pinned ? "Togli dalla cima" : "Fissa in alto"}
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M7.5 2.5h5l-.8 4.2 2.8 2.8v1.5H5.5V9.5l2.8-2.8-.8-4.2ZM10 11v6.5" /></svg>
                </button>
                <button
                  class="icon-btn"
                  class:on={showVersions}
                  onclick={() => (showVersions = !showVersions)}
                  aria-pressed={showVersions}
                  aria-label="Cronologia"
                  title="Cronologia delle versioni"
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <path d="M3.5 10a6.5 6.5 0 1 0 1.9-4.6M3.5 3.5v2.4h2.4M10 6.5V10l2.5 1.8" />
                  </svg>
                </button>
                <ReminderPicker
                  remindAt={selected.remind_at}
                  repeat={selected.repeat}
                  onset={setReminder}
                  onclear={clearReminder}
                />
                <ColorPicker color={selected.color} onpick={setColor} />
                {#if selected.archived}
                  <button onclick={() => setArchived(false)} disabled={checked.length > 0}>Ripristina</button>
                {:else}
                  <button onclick={() => setArchived(true)} disabled={checked.length > 0} title="Sposta la nota nell'archivio">
                    ✓ Segna completata
                  </button>
                {/if}
                <button onclick={exportSelected} disabled={checked.length > 0} title="Salva la nota come file .md o .txt">
                  Esporta…
                </button>
                <button class="danger" onclick={deleteTargets} disabled={checked.length > 0} title="Sposta nel cestino">
                  Elimina
                </button>
              {/if}
            </div>
          </header>
          {#if selected.deleted_at === null}
            <ReminderLine note={selected} onaction={reminderAction} />
          {/if}
          <div class="paper" class:readonly={selected.deleted_at !== null}>
            <input
              class="title"
              placeholder="Titolo"
              bind:this={titleInput}
              bind:value={selected.title}
              oninput={scheduleSave}
              readonly={selected.deleted_at !== null}
            />
            <NoteEditor
              bind:this={editor}
              value={selected.body}
              onchange={setBody}
              onopenlink={openLink}
              readonly={selected.deleted_at !== null}
            />
            {#if selected.body === "" && selected.deleted_at === null}
              <Templates onpick={applyTemplate} />
            {/if}
          </div>
          {#if backlinks.length > 0}
            <div class="backlinks">
              <span>Citata in</span>
              {#each backlinks as link (link.id)}
                <button onclick={() => showNote(link.id)}>{link.title || "Senza titolo"}</button>
              {/each}
            </div>
          {/if}
          <footer>
            <span>Creata il {fullDate(selected.created_at)}</span>
            {#if selected.deleted_at !== null}
              <span>
                Nel cestino · eliminata per sempre tra {daysLeft(selected)}
                {daysLeft(selected) === 1 ? "giorno" : "giorni"}
              </span>
            {:else}
              <span>Modificata il {fullDate(selected.updated_at)}</span>
            {/if}
          </footer>
        </article>
        {#if showVersions}
          <VersionsPanel
            noteId={selected.id}
            color={selected.color}
            onclose={() => (showVersions = false)}
            onrestored={async () => {
              showVersions = false;
              await reload();
            }}
          />
        {/if}
      {/key}
    {:else}
      <div class="placeholder">Seleziona una nota</div>
    {/if}
  </main>

  {#if showSettings}
    <SettingsPanel onclose={() => (showSettings = false)} />
  {/if}

  {#if notice}
    <div class="notice" class:error={notice.error} role="status">
      <span>{notice.text}</span>
      <button onclick={() => (notice = null)} aria-label="Chiudi">✕</button>
    </div>
  {/if}
</div>

<style>
  .app {
    --bg: #f6f6f8;
    --pane: #ffffff;
    --line: #e4e4ea;
    --text: #1d1d22;
    --muted: #6b6b76;
    --hover: #efeff4;
    --selected: #e6ecfb;
    --focus: #3b6fe0;
    --danger: #c9372c;

    position: fixed;
    inset: 0;
    display: grid;
    grid-template-columns: 200px minmax(330px, 380px) minmax(0, 1fr);
    background: var(--bg);
    color: var(--text);
    font: 13px/1.4 "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
  }
  /* Tema scuro: dalle impostazioni (Automatico segue Windows). */
  .app.dark {
    --bg: #1c1c20;
    --pane: #242429;
    --line: #34343b;
    --text: #ececf0;
    --muted: #9a9aa6;
    --hover: #2d2d33;
    --selected: #2c3550;
    --focus: #7aa2ff;
    --danger: #ff6b5e;
  }

  button {
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }

  /* Colonna sinistra */
  .list-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--pane);
    border-right: 1px solid var(--line);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 14px 10px;
    padding: 0 10px;
    height: 36px;
    border-radius: 10px;
    background: var(--hover);
  }
  .search svg {
    width: 16px;
    height: 16px;
    flex: none;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.8;
    stroke-linecap: round;
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    font: inherit;
    font-size: 14px;
    color: var(--text);
    outline: none;
  }
  .search:focus-within {
    box-shadow: 0 0 0 2px var(--focus);
  }
  .count {
    flex: none;
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px 10px;
  }
  .filters {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--hover);
  }
  .filters button {
    border: 0;
    border-radius: 6px;
    padding: 4px 10px;
    background: transparent;
    color: var(--muted);
    font-weight: 600;
  }
  .filters button.on {
    background: var(--pane);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }
  .new {
    margin-left: auto;
    border: 0;
    border-radius: 8px;
    padding: 5px 12px;
    background: var(--focus);
    color: #fff;
    font-weight: 600;
  }

  .bulk {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px 6px 4px;
    border-block: 1px solid var(--line);
    background: var(--selected);
    font-weight: 600;
  }
  .bulk .spacer {
    flex: 1;
  }
  .bulk button {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 3px 8px;
    background: var(--pane);
    font-size: 12px;
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin: 0;
    padding: 0 0 12px;
    list-style: none;
  }
  .list li {
    display: flex;
    align-items: stretch;
    border-bottom: 1px solid var(--line);
  }
  .list li:hover {
    background: var(--hover);
  }
  .list li.selected {
    background: var(--selected);
  }
  .check {
    display: flex;
    align-items: center;
    padding: 0 6px 0 12px;
    cursor: pointer;
  }
  .check input {
    width: 15px;
    height: 15px;
    margin: 0;
    accent-color: var(--focus);
    cursor: pointer;
  }
  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px 10px 4px;
    border: 0;
    background: transparent;
    text-align: left;
  }
  .bar {
    align-self: stretch;
    width: 5px;
    flex: none;
    border-radius: 3px;
  }
  li.archived .bar {
    opacity: 0.45;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-weight: 600;
    font-size: 14px;
  }
  .preview {
    color: var(--muted);
  }
  .meta {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .badge {
    padding: 1px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, #1f9d55 16%, transparent);
    color: #1b8a4b;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .notice {
    position: fixed;
    left: 50%;
    bottom: 18px;
    z-index: 60;
    translate: -50% 0;
    max-width: min(640px, calc(100vw - 40px));
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px 10px 16px;
    border-radius: 12px;
    background: var(--text);
    color: var(--pane);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  }
  .notice.error {
    background: var(--danger);
    color: #fff;
  }
  .notice span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .notice button {
    border: 0;
    background: transparent;
    color: inherit;
  }
  .badge.off {
    background: var(--hover);
    color: var(--muted);
  }
  .dark .badge {
    color: #5fd394;
  }
  .time {
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .detail {
    --bell-fg: var(--text);
    --bell-bg: var(--hover);
    --bell-set-bg: var(--focus);
    --line-bg: var(--hover);
    --line-fg: var(--text);
  }
  .remind {
    display: flex;
    align-items: center;
    gap: 3px;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }
  .remind svg {
    width: 11px;
    height: 11px;
    fill: currentColor;
  }
  .remind.overdue {
    color: var(--danger);
  }
  .empty {
    padding: 28px 16px;
    color: var(--muted);
    justify-content: center;
    text-align: center;
  }
  .list li.empty:hover {
    background: transparent;
  }

  /* Colonna destra */
  .detail-pane {
    min-width: 0;
    min-height: 0;
    display: flex;
  }
  .placeholder {
    margin: auto;
    color: var(--muted);
  }
  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 14px 20px 12px;
    gap: 12px;
  }
  .detail header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }
  .detail header button,
  .status {
    white-space: nowrap;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
    font-weight: 600;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
  }
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
  .actions button {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 5px 12px;
    background: var(--pane);
    font-weight: 600;
  }
  .actions button:hover:not(:disabled),
  .bulk button:hover {
    background: var(--hover);
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .danger {
    color: var(--danger);
  }
  .new.danger-fill {
    background: var(--danger);
  }
  .danger.confirm,
  .danger.confirm:hover,
  .danger-fill.confirm {
    background: var(--danger) !important;
    border-color: var(--danger) !important;
    color: #fff;
  }

  .paper {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 18px 22px;
    border-radius: 16px;
    background: var(--note);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.12);
    border-top: 5px solid var(--accent);
  }
  .paper .title {
    border: 0;
    background: transparent;
    outline: none;
    font: 700 20px "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    color: rgba(0, 0, 0, 0.85);
  }
  .paper {
    --image-max-h: 360px;
  }
  .pin-icon {
    width: 12px;
    height: 12px;
    margin-right: 3px;
    vertical-align: -1px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linejoin: round;
  }
  .icon-btn.on {
    border-color: var(--focus);
    background: var(--focus);
    color: #fff;
  }
  .folder-select,
  .move {
    max-width: 160px;
    padding: 3px 6px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--pane);
    color: var(--text);
    font: inherit;
    font-size: 12px;
  }
  .folder-select {
    margin-left: 6px;
    font-weight: 400;
  }
  .backlinks {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
  }
  .backlinks button {
    padding: 2px 9px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--pane);
    font-size: 12px;
  }
  .backlinks button:hover {
    background: var(--hover);
  }
  .progress {
    margin-right: 4px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .progress.complete {
    color: #1b8a4b;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--pane);
    color: var(--text);
  }
  .icon-btn:hover {
    background: var(--hover);
  }
  .icon-btn svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .paper input::placeholder {
    color: rgba(0, 0, 0, 0.35);
  }
  .paper.readonly {
    filter: saturate(0.35);
  }
  .detail footer {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    color: var(--muted);
    font-size: 12px;
  }
</style>
