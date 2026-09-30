<script lang="ts">
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { markdown, markdownKeymap, markdownLanguage } from "@codemirror/lang-markdown";
  import { Compartment, EditorSelection, EditorState } from "@codemirror/state";
  import { EditorView, keymap, placeholder as placeholderExt, tooltips } from "@codemirror/view";
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import ImageViewer from "$lib/ImageViewer.svelte";
  import {
    checklistInputRule,
    toggleBold,
    toggleChecklist as toggleChecklistCommand,
    toggleItalic,
    toggleStrike,
    toggleTaskOnLine,
  } from "$lib/editor/commands";
  import { linkClicks, noteCompletions } from "$lib/editor/links";
  import { livePreview } from "$lib/editor/livePreview";
  import { handleDrop, handlePaste, initMedia } from "$lib/editor/media";
  import { noteTheme } from "$lib/editor/theme";

  let {
    value,
    onchange,
    onopenlink,
    placeholder = "Scrivi qui…",
    readonly = false,
  }: {
    value: string;
    onchange: (value: string) => void;
    /** Clic su un link [[titolo]]. */
    onopenlink?: (title: string) => void;
    placeholder?: string;
    readonly?: boolean;
  } = $props();

  let host = $state<HTMLDivElement>();
  let view: EditorView | undefined;
  type ImageInfo = { src: string; name: string | null; x: number; y: number };
  let viewing = $state<ImageInfo | null>(null);
  let menu = $state<ImageInfo | null>(null);
  let menuNote = $state("");
  const editable = new Compartment();

  onMount(() => {
    let destroyed = false;
    host?.addEventListener("tabby-image", onImageEvent);
    initMedia().finally(() => {
      if (destroyed || !host) return;
      view = new EditorView({
        parent: host,
        state: EditorState.create({
          doc: value,
          extensions: [
            history(),
            markdown({ base: markdownLanguage }),
            keymap.of([
              { key: "Mod-b", run: toggleBold },
              { key: "Mod-i", run: toggleItalic },
              { key: "Mod-Shift-x", run: toggleStrike },
              { key: "Mod-l", run: toggleChecklistCommand },
              { key: "Mod-Enter", run: toggleTaskOnLine },
              ...markdownKeymap, // Invio continua elenchi e caselle
              ...historyKeymap,
              ...defaultKeymap,
              indentWithTab,
            ]),
            checklistInputRule,
            livePreview,
            noteCompletions,
            linkClicks((title) => onopenlink?.(title)),
            // "absolute": i suggerimenti restano al posto giusto anche nel deck specchiato (lato sinistro).
            tooltips({ position: "absolute" }),
            noteTheme,
            EditorView.lineWrapping,
            placeholderExt(placeholder),
            editable.of(EditorState.readOnly.of(readonly)),
            EditorView.domEventHandlers({ paste: handlePaste, drop: handleDrop }),
            EditorView.updateListener.of((update) => {
              if (update.docChanged) onchange(update.state.doc.toString());
            }),
          ],
        }),
      });
    });
    return () => {
      destroyed = true;
      host?.removeEventListener("tabby-image", onImageEvent);
      view?.destroy();
    };
  });

  // Testo cambiato da fuori (altra finestra, modello): sostituisce il documento.
  $effect(() => {
    const next = value;
    if (!view || next === view.state.doc.toString()) return;
    const head = Math.min(view.state.selection.main.head, next.length);
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: next },
      selection: EditorSelection.cursor(head),
    });
  });

  $effect(() => {
    view?.dispatch({ effects: editable.reconfigure(EditorState.readOnly.of(readonly)) });
  });

  export function focus(atEnd = false) {
    if (!view) return;
    if (atEnd) view.dispatch({ selection: EditorSelection.cursor(view.state.doc.length) });
    view.focus();
  }

  function onImageEvent(event: Event) {
    const { kind, ...info } = (event as CustomEvent<ImageInfo & { kind: "open" | "menu" }>).detail;
    menuNote = "";
    if (kind === "open") viewing = info;
    else menu = info;
  }

  async function copyImage() {
    if (!menu) return;
    try {
      const blob = await (await fetch(menu.src)).blob();
      const bitmap = await createImageBitmap(blob);
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      canvas.getContext("2d")!.drawImage(bitmap, 0, 0);
      const png = await new Promise<Blob>((ok, ko) => canvas.toBlob((b) => (b ? ok(b) : ko()), "image/png"));
      await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
      menu = null;
    } catch (e) {
      console.error("immagine non copiata", e);
      menuNote = "Copia non riuscita";
    }
  }

  async function saveImageAs() {
    const name = menu?.name;
    menu = null;
    if (!name) return;
    const dest = await save({ defaultPath: name.replace(/^[0-9a-f]{16}-/, "") });
    if (dest) await invoke("save_media_as", { name, dest }).catch((e) => console.error("immagine non salvata", e));
  }

  function openExternally() {
    const name = menu?.name;
    menu = null;
    if (name) invoke("open_media", { name }).catch((e) => console.error("file non aperto", e));
  }

  /** Toglie dalla nota la riga `![](media/…)` dell'immagine (il file sparisce al prossimo avvio se non serve più). */
  function removeImage() {
    const name = menu?.name;
    menu = null;
    if (!view || !name) return;
    const doc = view.state.doc;
    for (let n = 1; n <= doc.lines; n++) {
      const line = doc.line(n);
      if (line.text.trim() === `![](media/${name})`) {
        // Toglie anche un a capo, per non lasciare una riga vuota.
        const from = n === doc.lines && line.from > 0 ? line.from - 1 : line.from;
        const to = n === doc.lines ? line.to : line.to + 1;
        view.dispatch({ changes: { from, to } });
        break;
      }
    }
    view.focus();
  }

  /** Per il pulsante "casella" nell'intestazione della nota. */
  export function toggleChecklist() {
    if (!view) return;
    toggleChecklistCommand(view);
    view.focus();
  }
</script>

<div class="editor" bind:this={host}></div>

{#if viewing}
  <ImageViewer src={viewing.src} onclose={() => (viewing = null)} />
{/if}

{#if menu}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="menu-backdrop"
    onclick={() => (menu = null)}
    oncontextmenu={(e) => {
      e.preventDefault();
      menu = null;
    }}
  >
    <div
      class="menu"
      style:left="{Math.max(4, Math.min(menu.x, window.innerWidth - 190))}px"
      style:top="{Math.max(4, Math.min(menu.y, window.innerHeight - 170))}px"
      onclick={(e) => e.stopPropagation()}
    >
      <button
        onclick={() => {
          viewing = menu;
          menu = null;
        }}>Ingrandisci</button
      >
      <button onclick={copyImage}>Copia immagine</button>
      {#if menu.name}
        <button onclick={saveImageAs}>Salva con nome…</button>
        <button onclick={openExternally}>Apri con il programma predefinito</button>
        {#if !readonly}<button class="danger" onclick={removeImage}>Rimuovi dalla nota</button>{/if}
      {/if}
      {#if menuNote}<small>{menuNote}</small>{/if}
    </div>
  </div>
{/if}

<style>
  .editor {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 900;
  }
  .menu {
    position: fixed;
    width: 186px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    border-radius: 10px;
    background: rgba(30, 34, 46, 0.97);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
    color: #fff;
    font-size: 12px;
  }
  .menu button {
    border: 0;
    border-radius: 6px;
    padding: 6px 8px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: rgba(255, 255, 255, 0.14);
  }
  .menu .danger {
    color: #ff8a8a;
  }
  .menu small {
    padding: 4px 8px;
    color: #ffb4b4;
  }
  .editor :global(.cm-editor) {
    flex: 1;
    min-height: 0;
  }
</style>
