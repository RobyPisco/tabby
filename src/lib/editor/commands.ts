import { EditorSelection, type ChangeSpec } from "@codemirror/state";
import { EditorView, type Command } from "@codemirror/view";

const TASK = /^(\s*)([-*+]) \[( |x|X)\] /;
const BULLET = /^(\s*)([-*+]) /;

/** Spunta o toglie la spunta della casella che inizia in `pos` ("[ ]" / "[x]"). */
export function toggleTaskAt(view: EditorView, pos: number): boolean {
  const marker = view.state.sliceDoc(pos, pos + 3);
  if (!/^\[( |x|X)\]$/.test(marker)) return false;
  const checked = marker[1] !== " ";
  view.dispatch({ changes: { from: pos + 1, to: pos + 2, insert: checked ? " " : "x" } });
  return true;
}

/** Ctrl+Invio: spunta la casella della riga del cursore. */
export const toggleTaskOnLine: Command = (view) => {
  const line = view.state.doc.lineAt(view.state.selection.main.head);
  const match = TASK.exec(line.text);
  if (!match) return false;
  return toggleTaskAt(view, line.from + match[1].length + 2);
};

/** Trasforma le righe selezionate in caselle da spuntare, o le riporta a testo normale. */
export const toggleChecklist: Command = (view) => {
  const { state } = view;
  const lines = new Set<number>();
  for (const range of state.selection.ranges) {
    const first = state.doc.lineAt(range.from).number;
    const last = state.doc.lineAt(range.to).number;
    for (let n = first; n <= last; n++) lines.add(n);
  }
  const allTasks = [...lines].every((n) => TASK.test(state.doc.line(n).text));
  const changes: ChangeSpec[] = [];
  for (const n of lines) {
    const line = state.doc.line(n);
    const task = TASK.exec(line.text);
    const bullet = BULLET.exec(line.text);
    if (allTasks && task) {
      changes.push({ from: line.from, to: line.from + task[0].length, insert: task[1] });
    } else if (!task && bullet) {
      changes.push({ from: line.from + bullet[0].length, insert: "[ ] " });
    } else if (!task) {
      const indent = /^\s*/.exec(line.text)![0];
      changes.push({ from: line.from + indent.length, insert: "- [ ] " });
    }
  }
  view.dispatch({ changes });
  return true;
};

function wrapWith(marker: string): Command {
  return (view) => {
    view.dispatch(
      view.state.changeByRange((range) => {
        const before = view.state.sliceDoc(range.from - marker.length, range.from);
        const after = view.state.sliceDoc(range.to, range.to + marker.length);
        if (before === marker && after === marker) {
          return {
            changes: [
              { from: range.from - marker.length, to: range.from },
              { from: range.to, to: range.to + marker.length },
            ],
            range: EditorSelection.range(range.from - marker.length, range.to - marker.length),
          };
        }
        return {
          changes: [
            { from: range.from, insert: marker },
            { from: range.to, insert: marker },
          ],
          range: EditorSelection.range(range.from + marker.length, range.to + marker.length),
        };
      }),
    );
    return true;
  };
}

export const toggleBold = wrapWith("**");
export const toggleItalic = wrapWith("_");
export const toggleStrike = wrapWith("~~");

/** Scrivere "[]" o "[ ]" a inizio riga e poi spazio crea una casella da spuntare. */
export const checklistInputRule = EditorView.inputHandler.of((view, from, to, text) => {
  if (text !== " ") return false;
  const line = view.state.doc.lineAt(from);
  const before = view.state.sliceDoc(line.from, from);
  const match = /^(\s*)\[ ?\]$/.exec(before);
  if (!match) return false;
  const insert = `${match[1]}- [ ] `;
  view.dispatch({
    changes: { from: line.from, to, insert },
    selection: { anchor: line.from + insert.length },
  });
  return true;
});
