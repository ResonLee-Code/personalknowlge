/**
 * 把后端返回的「提醒」列表拼成一句人读的话。
 *
 * 为什么单独抽出来：扫描 / 打开工作区 / 外部文件变化三条路径都会带回 `warnings`
 * （「跳过无法读取的文件 xx.md」「重建 xx 的链接失败」等），以前**三条路径都把它丢掉了**——
 * 用户放进去的文件被跳过却毫无提示。这里统一成一个可测的纯函数。
 *
 * 返回 `null` 表示没有需要提示的内容（调用方据此跳过 toast，避免噪声）。
 */
export function formatWarningsToast(
  warnings: string[] | undefined | null,
  limit = 2,
): string | null {
  const list = (warnings ?? []).map((line) => line.trim()).filter((line) => line.length > 0);
  if (list.length === 0) return null;
  const head = list.slice(0, limit).join('；');
  if (list.length <= limit) return `注意：${head}`;
  return `注意（共 ${list.length} 条）：${head} 等`;
}
