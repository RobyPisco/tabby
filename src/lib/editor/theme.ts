import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

const HAND = '"Segoe Print", "Segoe UI", cursive';
const UI = '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif';

/** Aspetto "sticky note": sfondo trasparente (il colore è della nota), grafia a mano. */
const base = EditorView.theme({
  "&": {
    height: "100%",
    backgroundColor: "transparent",
    color: "rgba(0, 0, 0, 0.85)",
    fontSize: "var(--editor-size, 18px)",
    fontFamily: `var(--editor-font, ${HAND})`,
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "inherit", lineHeight: "1.5", overflow: "auto" },
  // Barra di scorrimento sottile e trasparente, al posto di quella grigia di Windows.
  ".cm-scroller::-webkit-scrollbar": { width: "6px" },
  ".cm-scroller::-webkit-scrollbar-button": { display: "none" },
  ".cm-scroller::-webkit-scrollbar-track": { background: "transparent" },
  ".cm-scroller::-webkit-scrollbar-thumb": { background: "rgba(0, 0, 0, 0.22)", borderRadius: "3px" },
  ".cm-content": { padding: "0", caretColor: "rgba(0, 0, 0, 0.85)" },
  ".cm-line": { padding: "0" },
  ".cm-placeholder": { color: "rgba(0, 0, 0, 0.35)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "rgba(0, 0, 0, 0.14) !important",
  },
  ".cm-task-box": {
    width: "15px",
    height: "15px",
    margin: "0 7px 0 1px",
    verticalAlign: "-1px",
    accentColor: "rgba(0, 0, 0, 0.75)",
    cursor: "pointer",
  },
  ".cm-wikilink": {
    color: "#1d4fb8",
    textDecoration: "underline",
    textDecorationStyle: "dotted",
    textUnderlineOffset: "3px",
    cursor: "pointer",
  },
  ".cm-link-open": {
    color: "#1d4fb8",
    textDecoration: "underline",
    textUnderlineOffset: "3px",
    cursor: "pointer",
  },
  ".cm-attachment": {
    display: "inline-block",
    padding: "1px 8px",
    borderRadius: "8px",
    background: "rgba(255, 255, 255, 0.55)",
    boxShadow: "0 1px 2px rgba(0, 0, 0, 0.15)",
    fontFamily: UI,
    fontSize: "0.8em",
    fontWeight: "600",
    cursor: "pointer",
  },
  ".cm-tag": {
    padding: "0 5px",
    borderRadius: "6px",
    background: "rgba(0, 0, 0, 0.09)",
    fontFamily: UI,
    fontSize: "0.8em",
    fontWeight: "600",
  },
  ".cm-tooltip.cm-tooltip-autocomplete": {
    border: "0",
    borderRadius: "10px",
    overflow: "hidden",
    background: "#fff",
    boxShadow: "0 6px 20px rgba(0, 0, 0, 0.25)",
    fontFamily: UI,
    fontSize: "13px",
  },
  ".cm-tooltip-autocomplete > ul > li": { padding: "4px 10px !important" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": { background: "#3b6fe0", color: "#fff" },
  ".cm-completionDetail": { marginLeft: "8px", opacity: "0.6", fontStyle: "normal" },
  ".cm-task-done": { textDecoration: "line-through", color: "rgba(0, 0, 0, 0.45)" },
  ".cm-image": {
    display: "block",
    maxWidth: "100%",
    maxHeight: "var(--image-max-h, 220px)",
    margin: "4px 0",
    borderRadius: "8px",
    boxShadow: "0 1px 4px rgba(0, 0, 0, 0.2)",
  },
});

const markdown = HighlightStyle.define([
  { tag: tags.heading1, fontFamily: UI, fontWeight: "700", fontSize: "1.35em" },
  { tag: tags.heading2, fontFamily: UI, fontWeight: "700", fontSize: "1.15em" },
  { tag: [tags.heading3, tags.heading4, tags.heading5, tags.heading6], fontFamily: UI, fontWeight: "700" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.link, color: "#1d4fb8", textDecoration: "underline" },
  { tag: tags.url, color: "#1d4fb8" },
  { tag: tags.monospace, fontFamily: "Cascadia Mono, Consolas, monospace", fontSize: "0.85em" },
  { tag: [tags.processingInstruction, tags.meta], color: "rgba(0, 0, 0, 0.4)" },
  { tag: tags.quote, fontStyle: "italic", color: "rgba(0, 0, 0, 0.65)" },
]);

export const noteTheme = [base, syntaxHighlighting(markdown)];
