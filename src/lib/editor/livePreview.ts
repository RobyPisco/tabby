import { syntaxTree } from "@codemirror/language";
import type { Range } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import { toggleTaskAt } from "./commands";
import { TAG, WIKILINK } from "./links";
import { attachmentLabel, mediaName, mediaUrl } from "./media";

/**
 * Anteprima dal vivo del Markdown: il testo resta Markdown, ma fuori dalla riga su cui
 * si sta scrivendo i simboli (#, **, _, ~~, `) spariscono, "- [ ]" diventa una casella
 * cliccabile, `![](media/x.png)` mostra l'immagine, `[[Titolo]]` diventa un link
 * e `#parola` un'etichetta.
 */

class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }
  eq(other: CheckboxWidget) {
    return other.checked === this.checked;
  }
  toDOM(view: EditorView) {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = this.checked;
    box.className = "cm-task-box";
    box.setAttribute("aria-label", this.checked ? "Fatto" : "Da fare");
    box.addEventListener("mousedown", (event) => {
      event.preventDefault(); // niente spostamento del cursore
      toggleTaskAt(view, view.posAtDOM(box));
    });
    return box;
  }
  ignoreEvent() {
    return true;
  }
}

class ImageWidget extends WidgetType {
  constructor(
    readonly src: string,
    readonly alt: string,
    readonly name: string | null,
  ) {
    super();
  }
  eq(other: ImageWidget) {
    return other.src === this.src;
  }
  toDOM() {
    const img = document.createElement("img");
    img.src = this.src;
    img.alt = this.alt;
    img.className = "cm-image";
    img.draggable = false;
    img.title = "Clic per ingrandire";
    // L'editor che contiene l'immagine ascolta questi eventi (vedi NoteEditor.svelte).
    const notify = (kind: "open" | "menu", event: MouseEvent) => {
      event.preventDefault();
      img.dispatchEvent(
        new CustomEvent("tabby-image", {
          bubbles: true,
          detail: { kind, src: this.src, name: this.name, x: event.clientX, y: event.clientY },
        }),
      );
    };
    img.addEventListener("click", (event) => notify("open", event));
    img.addEventListener("contextmenu", (event) => notify("menu", event));
    return img;
  }
  ignoreEvent() {
    return true;
  }
}

/** Allegato (`[📎 nome](media/…)`): un'etichetta che apre il file con un clic. */
class AttachmentWidget extends WidgetType {
  constructor(
    readonly name: string,
    readonly label: string,
  ) {
    super();
  }
  eq(other: AttachmentWidget) {
    return other.name === this.name && other.label === this.label;
  }
  toDOM() {
    const chip = document.createElement("span");
    chip.className = "cm-attachment";
    chip.dataset.media = this.name;
    chip.textContent = this.label.startsWith("📎") ? this.label : `📎 ${this.label}`;
    chip.title = "Apri il file";
    return chip;
  }
  ignoreEvent() {
    return false;
  }
}

const hidden = Decoration.replace({});

function linkMark(href: string) {
  return Decoration.mark({ class: "cm-link-open", attributes: { "data-href": href } });
}
const taskDone = Decoration.line({ class: "cm-task-done" });
const tagMark = Decoration.mark({ class: "cm-tag" });
const IMAGE = /^!\[([^\]]*)\]\(([^)\s]+)\)$/;

/** Le righe con il cursore mostrano il Markdown così com'è, per poterlo modificare. */
function activeLines(view: EditorView): Set<number> {
  const lines = new Set<number>();
  if (!view.hasFocus) return lines;
  for (const range of view.state.selection.ranges) {
    const first = view.state.doc.lineAt(range.from).number;
    const last = view.state.doc.lineAt(range.to).number;
    for (let n = first; n <= last; n++) lines.add(n);
  }
  return lines;
}

function build(view: EditorView): DecorationSet {
  const { state } = view;
  const active = activeLines(view);
  const decorations: Range<Decoration>[] = [];
  const isActive = (pos: number) => active.has(state.doc.lineAt(pos).number);

  for (const { from, to } of view.visibleRanges) {
    syntaxTree(state).iterate({
      from,
      to,
      enter(node) {
        switch (node.name) {
          case "HeaderMark": {
            if (isActive(node.from)) break;
            // Nasconde anche lo spazio dopo i #.
            const end = state.sliceDoc(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
            decorations.push(hidden.range(node.from, end));
            break;
          }
          case "EmphasisMark":
          case "StrikethroughMark":
            if (!isActive(node.from)) decorations.push(hidden.range(node.from, node.to));
            break;
          case "CodeMark":
            if (node.node.parent?.name === "InlineCode" && !isActive(node.from)) {
              decorations.push(hidden.range(node.from, node.to));
            }
            break;
          case "ListMark": {
            // Il trattino prima di una casella sparisce: resta solo la casella.
            if (node.node.nextSibling?.name !== "Task" || isActive(node.from)) break;
            const end = state.sliceDoc(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
            decorations.push(hidden.range(node.from, end));
            break;
          }
          case "TaskMarker": {
            const checked = /x/i.test(state.sliceDoc(node.from, node.to));
            decorations.push(
              Decoration.replace({ widget: new CheckboxWidget(checked) }).range(node.from, node.to),
            );
            if (checked) decorations.push(taskDone.range(state.doc.lineAt(node.from).from));
            break;
          }
          case "Link": {
            const url = node.node.getChild("URL");
            if (!url) break;
            const href = state.sliceDoc(url.from, url.to);
            const media = mediaName(href);
            const active = isActive(node.from);
            const marks = node.node.getChildren("LinkMark");
            if (media && !active) {
              const label = /^\[([^\]]*)\]/.exec(state.sliceDoc(node.from, node.to))?.[1] || attachmentLabel(media);
              decorations.push(
                Decoration.replace({ widget: new AttachmentWidget(media, label) }).range(node.from, node.to),
              );
            } else if (/^https?:\/\//.test(href) && marks.length >= 2) {
              // Fuori dalla riga attiva si vede solo il testo del link: [testo](indirizzo) → testo.
              if (!active) {
                decorations.push(hidden.range(marks[0].from, marks[0].to), hidden.range(marks[1].from, node.to));
              }
              if (marks[0].to < marks[1].from) decorations.push(linkMark(href).range(marks[0].to, marks[1].from));
            }
            return false;
          }
          case "URL": {
            // Indirizzo scritto così com'è (https://… o www.…).
            const href = state.sliceDoc(node.from, node.to);
            if (/^(https?:\/\/|www\.)/.test(href)) {
              decorations.push(linkMark(href.startsWith("www.") ? `https://${href}` : href).range(node.from, node.to));
            }
            break;
          }
          case "Image": {
            if (isActive(node.from)) break;
            const match = IMAGE.exec(state.sliceDoc(node.from, node.to));
            const src = match && mediaUrl(match[2]);
            if (src) {
              decorations.push(
                Decoration.replace({ widget: new ImageWidget(src, match[1], mediaName(match[2])) }).range(node.from, node.to),
              );
            }
            return false;
          }
        }
      },
    });
  }
  // Link e tag non fanno parte della grammatica Markdown: si cercano riga per riga.
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = state.doc.lineAt(pos);
      const lineActive = active.has(line.number);
      for (const match of line.text.matchAll(WIKILINK)) {
        const start = line.from + match.index;
        const end = start + match[0].length;
        if (!lineActive) {
          decorations.push(hidden.range(start, start + 2), hidden.range(end - 2, end));
        }
        const link = Decoration.mark({ class: "cm-wikilink", attributes: { "data-link": match[1].trim() } });
        decorations.push(link.range(start + 2, end - 2));
      }
      for (const match of line.text.matchAll(TAG)) {
        const start = line.from + match.index + match[1].length;
        decorations.push(tagMark.range(start, start + 1 + match[2].length));
      }
      pos = line.to + 1;
    }
  }
  return Decoration.set(decorations, true);
}

export const livePreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = build(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged || update.focusChanged) {
        this.decorations = build(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);
