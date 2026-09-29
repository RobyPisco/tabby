<script lang="ts">
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { markdown, markdownKeymap, markdownLanguage } from "@codemirror/lang-markdown";
  import { Compartment, EditorSelection, EditorState } from "@codemirror/state";
  import { EditorView, keymap, placeholder as placeholderExt, tooltips } from "@codemirror/view";
  import { onMount } from "svelte";
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
  const editable = new Compartment();

  onMount(() => {
    let destroyed = false;
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

  /** Per il pulsante "casella" nell'intestazione della nota. */
  export function toggleChecklist() {
    if (!view) return;
    toggleChecklistCommand(view);
    view.focus();
  }
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .editor :global(.cm-editor) {
    flex: 1;
    min-height: 0;
  }
</style>
