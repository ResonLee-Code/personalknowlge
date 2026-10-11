/**
 * 编辑器当前装载的内容是否属于这篇笔记。
 *
 * 为什么需要它：切笔记时 store 的 `detail` 先变、编辑器的 `content/title` 要等同步 effect 才跟上，
 * 中间那一帧两者是「错配」的；如果这时落盘（防抖到点、或切笔记前的 flush），
 * 就会把上一篇的内容写成新笔记的内容——**真机上表现为编辑区在 A/B 之间频闪，并伴随丢正文**
 * （2026-10-11 验收时四篇笔记被写空）。单独成文件既能被单测直接覆盖，
 * 也满足 Fast Refresh 只导出组件的要求。
 */
/**
 * 编辑器当前装载的内容是否属于这篇笔记。
 *
 * 为什么需要它：切笔记时 store 的 `detail` 先变、编辑器的 `content/title` 要等同步 effect 才跟上，
 * 中间那一帧两者是「错配」的；如果这时落盘（防抖到点、或切笔记前的 flush），
 * 就会把上一篇的内容写成新笔记的内容——**真机上表现为编辑区在 A/B 之间频闪，并伴随丢正文**。
 * 导出成纯函数便于单测（这是防线，不能只靠肉眼）。
 */
export function editorContentBelongsTo(
  loadedNoteId: string | null,
  detailNoteId: string | null,
): boolean {
  return Boolean(loadedNoteId) && loadedNoteId === detailNoteId;
}
