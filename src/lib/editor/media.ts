import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { EditorView } from "@codemirror/view";

let mediaDir: string | null = null;

const IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/** Da chiamare una volta prima di creare l'editor: serve per mostrare le immagini. */
export async function initMedia() {
  mediaDir ??= await invoke<string>("media_path");
}

/** `media/<nome>` → URL caricabile dalla webview; gli indirizzi http(s) restano come sono. */
export function mediaUrl(path: string): string | null {
  if (/^https?:\/\//.test(path)) return path;
  const name = mediaName(path);
  if (!name || mediaDir === null) return null;
  return convertFileSrc(`${mediaDir}\\${name}`);
}

/** Il nome del file se `path` è `media/<nome>`, altrimenti null. */
export function mediaName(path: string): string | null {
  return /^media\/([^/\\]+)$/.exec(path)?.[1] ?? null;
}

/** Nome leggibile di un allegato: senza il prefisso esadecimale aggiunto dall'app. */
export function attachmentLabel(name: string): string {
  return name.replace(/^[0-9a-f]{16}-/, "");
}

/**
 * Salva un file nella cartella media e restituisce il Markdown per citarlo:
 * `![](media/x.png)` per le immagini, `[📎 nome](media/x-nome)` per gli altri file.
 */
export async function saveFile(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (IMAGE_TYPES.includes(file.type)) {
    const ext = file.type.split("/")[1];
    const name = await invoke<string>("save_image", bytes, { headers: { "x-image-ext": ext } });
    return `![](media/${name})`;
  }
  const name = await invoke<string>("save_attachment", bytes, {
    headers: { "x-file-name": encodeURIComponent(file.name || "allegato") },
  });
  return `[📎 ${file.name || "allegato"}](media/${name})`;
}

function droppedFiles(data: DataTransfer | null): File[] {
  return [...(data?.files ?? [])];
}

async function insertFiles(view: EditorView, files: File[], at: number) {
  const parts = await Promise.all(files.map(saveFile));
  // Ogni file va su una riga sua.
  const line = view.state.doc.lineAt(at);
  const before = at > line.from ? "\n" : "";
  const after = at < line.to ? "\n" : "";
  const insert = `${before}${parts.join("\n")}${after}`;
  view.dispatch({ changes: { from: at, insert }, selection: { anchor: at + insert.length } });
}

/** Incollare (Ctrl+V) un'immagine o un file copiato da Esplora risorse. */
export function handlePaste(event: ClipboardEvent, view: EditorView): boolean {
  const files = droppedFiles(event.clipboardData);
  if (files.length === 0) return false;
  event.preventDefault();
  insertFiles(view, files, view.state.selection.main.head).catch((e) => console.error("file non salvato", e));
  return true;
}

/** Trascinare file dentro la nota. */
export function handleDrop(event: DragEvent, view: EditorView): boolean {
  const files = droppedFiles(event.dataTransfer);
  if (files.length === 0) return false;
  event.preventDefault();
  const pos = view.posAtCoords({ x: event.clientX, y: event.clientY }) ?? view.state.selection.main.head;
  insertFiles(view, files, pos).catch((e) => console.error("file non salvato", e));
  return true;
}
