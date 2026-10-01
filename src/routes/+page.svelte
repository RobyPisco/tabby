<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { clock } from "$lib/clock.svelte";
  import ColorPicker from "$lib/ColorPicker.svelte";
  import NoteEditor from "$lib/NoteEditor.svelte";
  import { saveFile } from "$lib/editor/media";
  import { colorForNewNote, dockWidth, initSettings, settings } from "$lib/settings.svelte";
  import ReminderLine from "$lib/ReminderLine.svelte";
  import ReminderPicker from "$lib/ReminderPicker.svelte";
  import Templates from "$lib/Templates.svelte";
  import {
    isOverdue,
    onNotesChangedElsewhere,
    vivid,
    type Note,
    type ReminderAction,
    type Repeat,
    type Template,
  } from "$lib/notes";

  type Phase = "rest" | "fan" | "open";

  const TAB_WIDTH = 172; // spazio occupato dalle schede sul bordo (scheda + margine)
  const TAB_HEIGHT = 40; // altezza massima; con molte note le schede si accorciano
  const TAB_MIN_HEIGHT = 24;
  const TAB_GAP = 8;
  const STAGGER_MS = 45;
  const TRIGGER_WIDTH = 32; // fascia di bordo che risveglia il deck
  const GLASS_ALPHA = { clear: 0.15, dense: 0.85 }; // opacità del vetro a 100% e a 0% di trasparenza
  const LEAVE_DELAY_MS = 350;
  const SAVE_DEBOUNCE_MS = 250;

  let notes = $state<Note[]>([]);
  let phase = $state<Phase>("rest");
  let openId = $state<number | null>(null);
  let winHeight = $state(800);
  let editing = $state(false);
  let titleInput = $state<HTMLInputElement>();
  let reminderOpen = $state(false);
  let editor = $state<NoteEditor>();
  /** Aperta da una notifica: resta aperta finché il cursore non passa sul deck. */
  let sticky = false;
  let interactive = false;
  let leaveTimer: ReturnType<typeof setTimeout> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  const PILL_SEG_MAX = 24;
  const PILL_SEG_MIN = 6;
  const DECK_MARGIN = 80; // spazio sopra e sotto il deck, compresi i pulsanti + e ☰

  /** Accorcia le linguette quanto basta perché il deck stia nello schermo. */
  const tabHeight = $derived.by(() => {
    if (notes.length === 0) return TAB_HEIGHT;
    const fit = (winHeight - 2 * DECK_MARGIN + TAB_GAP) / notes.length - TAB_GAP;
    return Math.round(Math.min(TAB_HEIGHT, Math.max(TAB_MIN_HEIGHT, fit)));
  });
  const pillSegHeight = $derived.by(() => {
    if (notes.length === 0) return PILL_SEG_MAX;
    const fit = (winHeight * 0.6) / notes.length - 5;
    return Math.round(Math.min(PILL_SEG_MAX, Math.max(PILL_SEG_MIN, fit)));
  });
  /** Larghezza della finestra e della nota aperta, dalle impostazioni. */
  const dockW = $derived(dockWidth());
  const noteW = $derived(settings.note_width);
  const noteH = $derived(Math.min(settings.note_width, Math.max(winHeight - 40, 200)));
  const left = $derived(settings.side === "left");
  const glassAlpha = $derived(
    GLASS_ALPHA.dense - (settings.deck_transparency / 100) * (GLASS_ALPHA.dense - GLASS_ALPHA.clear),
  );
  const noteTop = $derived.by(() => {
    const top = Math.max((winHeight - deckHeight) / 2, 8) - 20;
    return Math.max(12, Math.min(top, winHeight - noteH - 12));
  });
  const deckHeight = $derived(Math.max(notes.length * (tabHeight + TAB_GAP) - TAB_GAP, 40));
  const deckTop = $derived(Math.max((winHeight - deckHeight) / 2, 8));
  const openNote = $derived(notes.find((n) => n.id === openId) ?? null);

  function setInteractive(value: boolean) {
    if (interactive === value) return;
    interactive = value;
    invoke("set_interactive", { interactive: value });
  }

  function setPhase(next: Phase) {
    phase = next;
    setInteractive(next !== "rest");
  }

  /** Il cursore è sul deck (pillola/linguette) o sulla nota aperta? */
  function overDeck(rawX: number, y: number): boolean {
    // Sul bordo sinistro la pagina è specchiata: si ragiona come se fosse a destra.
    const x = left ? dockW - rawX : rawX;
    const noteOpen = phase === "open";
    const top = noteOpen ? Math.min(deckTop - 24, noteTop) : deckTop - 24;
    const bottom = Math.max(deckTop + deckHeight + 72, noteOpen ? noteTop + noteH + 12 : 0); // + e ☰ compresi
    if (y < top || y > bottom) return false;
    const deckLeft = dockW - (phase === "rest" ? TRIGGER_WIDTH : TAB_WIDTH);
    return x >= (noteOpen ? dockW - TAB_WIDTH - noteW - 12 : deckLeft);
  }

  /** Salva subito la nota in sospeso (chiamata alla chiusura e prima di archiviare). */
  async function flushSave() {
    if (saveTimer === undefined) return;
    clearTimeout(saveTimer);
    saveTimer = undefined;
    const note = notes.find((n) => n.id === openId);
    if (note) await invoke("update_note", { id: note.id, title: note.title, body: note.body });
  }

  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      const note = notes.find((n) => n.id === openId);
      if (note) invoke("update_note", { id: note.id, title: note.title, body: note.body });
    }, SAVE_DEBOUNCE_MS);
  }

  async function collapse() {
    await flushSave();
    openId = null;
    editing = false;
    sticky = false;
    reminderOpen = false;
    setPhase("rest");
  }

  async function reload() {
    notes = await invoke<Note[]>("list_notes");
    // La nota aperta è stata archiviata o eliminata dalla finestra "Tutte le note".
    if (openId !== null && !notes.some((n) => n.id === openId)) {
      openId = null;
      editing = false;
      if (phase === "open") setPhase("fan");
    }
  }

  onMount(() => {
    reload();
    const settingsReady = initSettings();
    const unlistenChanges = onNotesChangedElsewhere(reload);
    // Ctrl+Alt+N / Ctrl+Alt+P o menu della tray; il payload dice se aprire subito il promemoria.
    const unlistenNewNote = listen<boolean>("new-note", ({ payload }) => addNote(payload));
    // Clic su una notifica di promemoria.
    const unlistenOpenNote = listen<number>("open-note", async ({ payload }) => {
      await reload();
      if (!notes.some((n) => n.id === payload)) return;
      sticky = true;
      openNoteById(payload);
    });

    const unlisten = listen<{ x: number; y: number; height: number }>("cursor", ({ payload }) => {
      winHeight = payload.height;
      const inside = overDeck(payload.x, payload.y);
      if (inside) {
        sticky = false;
        clearTimeout(leaveTimer);
        leaveTimer = undefined;
        if (phase === "rest") setPhase("fan");
      } else if (phase !== "rest" && !editing && !sticky && leaveTimer === undefined) {
        leaveTimer = setTimeout(() => {
          leaveTimer = undefined;
          collapse();
        }, LEAVE_DELAY_MS);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
      unlistenChanges.then((fn) => fn());
      unlistenNewNote.then((fn) => fn());
      unlistenOpenNote.then((fn) => fn());
      settingsReady.then((fn) => fn());
    };
  });

  async function openNoteById(id: number) {
    await flushSave();
    openId = id;
    reminderOpen = false;
    setPhase("open");
  }

  async function addNote(withReminder = false) {
    await flushSave();
    const note = await invoke<Note>("create_note", { color: colorForNewNote(notes.length) });
    notes.push(note);
    openId = note.id;
    setPhase("open");
    await tick();
    if (withReminder) reminderOpen = true;
    else titleInput?.focus();
  }

  /** File, link o testo trascinati sul deck diventano una nota nuova
   *  (se cadono dentro la nota aperta, li gestisce già l'editor). */
  async function onDrop(event: DragEvent) {
    if (event.defaultPrevented || !event.dataTransfer) return;
    event.preventDefault();
    const data = event.dataTransfer;
    const files = [...data.files];
    const uri = data
      .getData("text/uri-list")
      .split(/\r?\n/)
      .find((line) => line && !line.startsWith("#"));
    const text = data.getData("text/plain").trim();
    let title = "";
    let body = "";
    if (files.length > 0) {
      title = files[0].name.replace(/\.[^.]+$/, "");
      body = (await Promise.all(files.map(saveFile))).join("\n");
    } else if (uri) {
      const label = text && text !== uri ? text.split("\n")[0] : "";
      title = label || new URL(uri).hostname.replace(/^www\./, "");
      body = `[${title}](${uri})`;
    } else if (text) {
      title = text.split("\n")[0].slice(0, 60);
      body = text;
    } else {
      return;
    }
    await flushSave();
    const note = await invoke<Note>("create_note", { color: colorForNewNote(notes.length) });
    await invoke("update_note", { id: note.id, title, body });
    await reload();
    await openNoteById(note.id);
  }

  function onDragOver(event: DragEvent) {
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
  }

  function setBody(body: string) {
    if (!openNote) return;
    openNote.body = body;
    scheduleSave();
  }

  async function togglePin() {
    if (!openNote) return;
    openNote.pinned = !openNote.pinned;
    await invoke("set_pinned", { id: openNote.id, pinned: openNote.pinned });
    await reload(); // le fissate vanno in cima al deck
  }

  /** Clic su [[titolo]]: apre la nota nel deck, oppure in "Tutte le note" se è archiviata;
   *  se non esiste ancora la crea con quel titolo. */
  async function openLink(title: string) {
    await flushSave();
    const wanted = title.trim().toLowerCase();
    const inDeck = notes.find((n) => n.title.trim().toLowerCase() === wanted);
    if (inDeck) return openNoteById(inDeck.id);
    const found = await invoke<Note | null>("find_note_by_title", { title });
    if (found) return invoke("show_note_in_all", { id: found.id });
    const note = await invoke<Note>("create_note", { color: colorForNewNote(notes.length) });
    await invoke("update_note", { id: note.id, title: title.trim(), body: "" });
    note.title = title.trim();
    notes.push(note);
    await openNoteById(note.id);
    await tick();
    editor?.focus();
  }

  async function applyTemplate(template: Template) {
    if (!openNote) return;
    if (!openNote.title.trim()) openNote.title = template.title;
    setBody(template.body);
    await tick();
    editor?.focus(true);
  }

  function setReminder(dueAt: number, repeat: Repeat) {
    if (!openNote) return;
    openNote.remind_at = dueAt;
    openNote.repeat = repeat;
    invoke("set_reminder", { noteId: openNote.id, dueAt, repeat });
    titleInput?.focus();
  }

  function clearReminder() {
    if (!openNote) return;
    openNote.remind_at = null;
    openNote.repeat = null;
    invoke("clear_reminder", { noteId: openNote.id });
  }

  async function reminderAction(action: ReminderAction) {
    if (!openNote) return;
    await invoke("reminder_action", { noteId: openNote.id, action });
    await reload();
  }

  function setColor(color: string) {
    if (!openNote) return;
    openNote.color = color;
    invoke("set_color", { id: openNote.id, color });
  }

  async function archiveOpen() {
    if (!openNote) return;
    const id = openNote.id;
    await flushSave();
    await invoke("set_archived", { ids: [id], archived: true });
    notes = notes.filter((n) => n.id !== id);
    openId = null;
    editing = false;
  }

  /** Sposta la nota nel cestino (si recupera dalla finestra "Tutte le note"). */
  async function trashOpen() {
    if (!openNote) return;
    const id = openNote.id;
    await flushSave();
    await invoke("set_trashed", { ids: [id], trashed: true });
    notes = notes.filter((n) => n.id !== id);
    openId = null;
    editing = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && phase !== "rest") {
      (document.activeElement as HTMLElement | null)?.blur();
      collapse();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="stage"
  class:left
  style:--tab-w="{TAB_WIDTH}px"
  style:--tab-h="{tabHeight}px"
  style:--seg-h="{pillSegHeight}px"
  style:--note-w="{noteW}px"
  style:--note-h="{noteH}px"
  style:--glass={glassAlpha}
  ondragover={onDragOver}
  ondrop={onDrop}
>
  <!-- Pillola a riposo -->
  <div class="pill" class:hidden={phase !== "rest"} style:top="{deckTop + deckHeight / 2}px">
    {#each notes as note (note.id)}
      <span style:--c={vivid(note.color)} class:due={isOverdue(note, clock.now)}></span>
    {/each}
  </div>

  <!-- Linguette a ventaglio -->
  <div class="deck" style:top="{deckTop}px">
    {#each notes as note, i (note.id)}
      <button
        class="tab"
        class:shown={phase !== "rest"}
        class:active={openId === note.id}
        class:due={isOverdue(note, clock.now)}
        class:has-bell={note.remind_at !== null}
        class:pinned={note.pinned}
        style:--c={vivid(note.color)}
        style:top="{i * (tabHeight + TAB_GAP)}px"
        style:transition-delay="{phase === "rest" ? 0 : i * STAGGER_MS}ms"
        onclick={() => openNoteById(note.id)}
        aria-label={note.title || "Senza titolo"}
      >
        <span class="dot"></span>
        <span class="label">{note.title || "Senza titolo"}</span>
        {#if note.pinned}
          <svg class="tab-pin" viewBox="0 0 20 20" aria-hidden="true"><path d="M7.5 2.5h5l-.8 4.2 2.8 2.8v1.5H5.5V9.5l2.8-2.8-.8-4.2ZM10 11v6.5" /></svg>
        {/if}
        {#if note.remind_at !== null}
          <svg class="tab-bell" viewBox="0 0 20 20" aria-hidden="true">
            <path d="M10 2.5a5 5 0 0 0-5 5v3.2L3.6 13.5h12.8L15 10.7V7.5a5 5 0 0 0-5-5Zm-2 13a2 2 0 0 0 4 0Z" />
          </svg>
        {/if}
      </button>
    {/each}
    <button
      class="add"
      class:shown={phase !== "rest"}
      style:top="{deckHeight + 12}px"
      onclick={() => addNote()}
      aria-label="Nuova nota"
    >+</button>
    <button
      class="add all"
      class:shown={phase !== "rest"}
      style:top="{deckHeight + 44}px"
      onclick={() => invoke("show_all_notes")}
      aria-label="Tutte le note"
      title="Tutte le note (Ctrl+Alt+L)"
    >☰</button>
  </div>

  <!-- Nota aperta a piena dimensione -->
  {#if openNote}
    {#key openNote.id}
      <div class="note-wrap" style:top="{noteTop}px">
      <article
        class="note"
        style:background={openNote.color}
        onfocusin={() => (editing = true)}
        onfocusout={() => (editing = false)}
      >
        <header>
          <input
            class="title"
            placeholder="Titolo"
            bind:this={titleInput}
            bind:value={openNote.title}
            oninput={scheduleSave}
            readonly={openNote.gcal_event_id !== null}
          />
          {#if openNote.gcal_event_id === null}
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
          {/if}
          <button
            class="icon-btn"
            class:on={openNote.pinned}
            onmousedown={(e) => e.preventDefault()}
            onclick={togglePin}
            aria-pressed={openNote.pinned}
            aria-label="Fissa in alto"
            title={openNote.pinned ? "Togli dalla cima" : "Fissa in alto"}
          >
            <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M7.5 2.5h5l-.8 4.2 2.8 2.8v1.5H5.5V9.5l2.8-2.8-.8-4.2ZM10 11v6.5" /></svg>
          </button>
          <ReminderPicker
            remindAt={openNote.remind_at}
            repeat={openNote.repeat}
            onset={setReminder}
            onclear={clearReminder}
            bind:open={reminderOpen}
          />
          <ColorPicker color={openNote.color} onpick={setColor} />
          <button class="archive" onclick={archiveOpen} title="Archivia">Archivia</button>
        </header>
        {#if openNote.gcal_event_id !== null}
          <div class="gcal-badge">
            <span>📅 Evento di Google Calendar · sola lettura</span>
            <button class="gcal-trash" onclick={trashOpen} title="Sposta nel cestino">Elimina</button>
          </div>
        {/if}
        <ReminderLine note={openNote} onaction={reminderAction} />
        <NoteEditor
          bind:this={editor}
          value={openNote.body}
          onchange={setBody}
          onopenlink={openLink}
          readonly={openNote.gcal_event_id !== null}
          placeholder={openNote.gcal_event_id !== null ? "Nessun dettaglio" : undefined}
        />
        {#if openNote.body === "" && openNote.gcal_event_id === null}
          <Templates onpick={applyTemplate} />
        {/if}
      </article>
      </div>
    {/key}
  {/if}
</div>

<style>
  .gcal-badge {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 0 6px;
    font-size: 11px;
    color: rgba(0, 0, 0, 0.6);
  }
  .gcal-trash {
    border: 0;
    border-radius: 8px;
    padding: 3px 8px;
    background: rgba(0, 0, 0, 0.08);
    font: 600 11px "Segoe UI", system-ui, sans-serif;
    color: rgba(0, 0, 0, 0.6);
    cursor: pointer;
  }
  .gcal-trash:hover {
    background: rgba(0, 0, 0, 0.16);
  }

  :global(html),
  :global(body) {
    background: transparent;
    font-family: "Segoe UI", system-ui, sans-serif;
  }

  .stage {
    position: fixed;
    inset: 0;
    overflow: hidden;
    user-select: none;
  }

  /* A riposo: sottili barre di luce sul bordo, una per nota. */
  .pill {
    position: absolute;
    right: 5px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    transform: translateY(-50%);
    transition: opacity 0.2s;
  }
  .pill.hidden {
    opacity: 0;
  }
  .pill span {
    width: 6px;
    height: var(--seg-h);
    border-radius: 4px;
    background: var(--c);
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.35),
      0 0 10px color-mix(in srgb, var(--c) 70%, transparent);
  }
  /* Promemoria scaduto: la barra lampeggia di rosso. */
  .pill span.due {
    animation: due-blink 1.2s ease-in-out infinite;
  }
  @keyframes due-blink {
    50% {
      background: #ff3b2f;
      box-shadow: 0 0 10px #ff3b2f;
    }
  }

  .deck {
    position: absolute;
    right: 0;
    width: var(--tab-w);
  }

  /* Schede di vetro: fondo scuro semitrasparente, così il testo si legge su qualsiasi sfondo. */
  .tab,
  .add {
    border: 1px solid rgba(255, 255, 255, 0.22);
    background: rgba(24, 27, 36, var(--glass));
    backdrop-filter: blur(14px) saturate(1.4);
    box-shadow:
      0 4px 14px rgba(0, 0, 0, 0.25),
      inset 0 1px 0 rgba(255, 255, 255, 0.18);
    color: #fff;
    cursor: pointer;
  }
  .tab {
    position: absolute;
    right: 8px;
    width: 150px;
    height: var(--tab-h);
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    border-radius: 14px;
    text-align: left;
    transform: translateX(calc(100% + 16px));
    opacity: 0;
    transition:
      transform 0.28s cubic-bezier(0.2, 0.9, 0.3, 1.1),
      width 0.2s,
      opacity 0.2s;
  }
  .tab.shown {
    transform: translateX(0);
    opacity: 1;
  }
  .tab:hover,
  .tab.active {
    width: 164px;
    border-color: rgba(255, 255, 255, 0.4);
  }
  .tab.active {
    background: rgba(24, 27, 36, min(1, calc(var(--glass) + 0.2)));
  }
  .tab.due {
    animation: due-glow 1.2s ease-in-out infinite;
  }
  @keyframes due-glow {
    50% {
      box-shadow:
        0 4px 14px rgba(0, 0, 0, 0.25),
        0 0 0 2px #ff3b2f;
    }
  }
  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--c);
    box-shadow:
      0 0 0 3px color-mix(in srgb, var(--c) 30%, transparent),
      0 0 10px var(--c);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    font-weight: 500;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.55);
  }
  .tab-pin,
  .tab-bell {
    flex: none;
    width: 13px;
    height: 13px;
  }
  .tab-pin {
    fill: none;
    stroke: rgba(255, 255, 255, 0.75);
    stroke-width: 1.8;
    stroke-linejoin: round;
  }
  .tab-bell {
    fill: rgba(255, 255, 255, 0.75);
  }
  .tab.due .tab-bell {
    fill: #ff6b5e;
  }

  .add {
    position: absolute;
    right: 8px;
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: 50%;
    font-size: 16px;
    line-height: 1;
    opacity: 0;
    transition: opacity 0.2s 0.2s;
  }
  .add.shown {
    opacity: 1;
  }
  .add.all {
    font-size: 12px;
  }

  /* Bordo sinistro: tutta la scena è specchiata, poi nota ed etichette tornano leggibili. */
  .stage.left {
    transform: scaleX(-1);
  }
  .stage.left .label,
  .stage.left .tab svg,
  .stage.left .add,
  .stage.left .note-wrap {
    transform: scaleX(-1);
  }
  .stage.left .tab {
    flex-direction: row-reverse;
  }
  .note-wrap {
    position: absolute;
    right: calc(var(--tab-w) + 6px);
    width: var(--note-w);
    height: var(--note-h);
  }
  .note {
    height: 100%;
    padding: 14px 16px 16px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-radius: 16px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    animation: slide-out 0.25s cubic-bezier(0.2, 0.9, 0.3, 1);
    user-select: text;
  }
  .note header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .title {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    outline: none;
    font: 700 15px "Segoe UI", system-ui, sans-serif;
    color: rgba(0, 0, 0, 0.85);
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.08);
    color: rgba(0, 0, 0, 0.55);
    cursor: pointer;
  }
  .icon-btn:hover {
    background: rgba(0, 0, 0, 0.16);
  }
  .icon-btn.on {
    background: rgba(0, 0, 0, 0.78);
    color: #fff;
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
  .archive {
    border: 0;
    border-radius: 8px;
    padding: 3px 8px;
    background: rgba(0, 0, 0, 0.08);
    font: 600 11px "Segoe UI", system-ui, sans-serif;
    color: rgba(0, 0, 0, 0.6);
    cursor: pointer;
  }
  .archive:hover {
    background: rgba(0, 0, 0, 0.16);
  }

  @keyframes slide-out {
    from {
      transform: translateX(60%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }
</style>
