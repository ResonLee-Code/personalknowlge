import type { EditorView } from '@codemirror/view';

export function handleSelectionContextMenu(
  view: EditorView,
  event: MouseEvent,
  callback?: (event: MouseEvent, text: string) => void,
): boolean {
  if (!callback) return false;
  const { from, to } = view.state.selection.main;
  if (to <= from) return false;
  const text = view.state.sliceDoc(from, to).trim();
  if (!text) return false;
  event.preventDefault();
  callback(event, text);
  return true;
}
