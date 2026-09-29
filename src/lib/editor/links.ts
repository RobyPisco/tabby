import { autocompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { EditorView } from "@codemirror/view";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { NoteRef, TagCount } from "$lib/notes";

/** `[[Titolo della nota]]` */
export const WIKILINK = /\[\[([^[\]\n]+)\]\]/g;
/** `#parola` a inizio riga o dopo uno spazio (come `extract_tags` in organize.rs). */
export const TAG = /(^|\s)#([\p{L}\p{N}_-]*\p{L}[\p{L}\p{N}_-]*)/gu;

/** Dopo "[[" propone i titoli delle note. */
async function linkCompletions(context: CompletionContext): Promise<CompletionResult | null> {
  const match = context.matchBefore(/\[\[[^[\]\n]*/);
  if (!match) return null;
  const titles = await invoke<NoteRef[]>("list_titles");
  const closed = context.state.sliceDoc(context.pos, context.pos + 2) === "]]";
  return {
    from: match.from + 2,
    options: titles.map((note) => ({
      label: note.title,
      type: "text",
      apply: closed ? note.title : `${note.title}]]`,
    })),
    validFor: /^[^[\]\n]*$/,
  };
}

/** Dopo "#" propone i tag già usati. */
async function tagCompletions(context: CompletionContext): Promise<CompletionResult | null> {
  const match = context.matchBefore(/(?:^|\s)#[\p{L}\p{N}_-]*/u);
  if (!match) return null;
  const tags = await invoke<TagCount[]>("list_tags");
  if (tags.length === 0) return null;
  return {
    from: match.from + match.text.indexOf("#") + 1,
    options: tags.map((t) => ({ label: t.tag, type: "keyword", detail: String(t.count) })),
    validFor: /^[\p{L}\p{N}_-]*$/u,
  };
}

export const noteCompletions = autocompletion({
  override: [linkCompletions, tagCompletions],
  icons: false,
});

/**
 * Clic su un link: `[[Titolo]]` apre la nota, un indirizzo web si apre nel browser,
 * un allegato 📎 con il suo programma. Sulla riga dove si sta scrivendo il clic
 * serve a posizionare il cursore, quindi lì si apre solo con Ctrl+clic.
 */
export function linkClicks(openNote: (title: string) => void) {
  return EditorView.domEventHandlers({
    mousedown(event, view) {
      const target = (event.target as HTMLElement).closest<HTMLElement>("[data-link], [data-href], [data-media]");
      if (!target) return false;
      const line = view.state.doc.lineAt(view.posAtDOM(target));
      const editingLine =
        view.hasFocus && view.state.selection.ranges.some((r) => r.from <= line.to && r.to >= line.from);
      if (editingLine && !event.ctrlKey) return false;
      event.preventDefault();
      const { link, href, media } = target.dataset;
      if (link) openNote(link);
      else if (href) openUrl(href).catch((e) => console.error("link non aperto", e));
      else if (media) invoke("open_media", { name: media }).catch((e) => console.error("file non aperto", e));
      return true;
    },
  });
}
