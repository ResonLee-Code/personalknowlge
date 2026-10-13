import { errorMessage } from '@/lib/format';
import { call } from '@/services/api';

/**
 * 把「界面上看得见的失败」写进应用日志（Track B3 / E4）。
 *
 * 背景：保存 / 导出 / 恢复 / 扫描这些失败以前只弹一个 toast，用户报障时我们手上
 * 只有一句界面描述、日志里查不到——排障全靠猜。现在前端把**同一条原因**交给后端，
 * 与 Rust 侧共用 `…/logs/app.log`，按时间一搜就能对上。
 *
 * 三条纪律：
 * 1. **只记原因，不记正文**（正文不进日志是既有口径，后端也会截断到 500 字兜底）；
 * 2. 这个函数**永不抛错**：写日志失败不能变成新的失败源，也不能打断主流程；
 * 3. 用 `void` 调用（fire-and-forget），不阻塞用户操作。
 */
export async function logAppFailure(
  scope: string,
  error: unknown,
  level: 'error' | 'warn' | 'info' = 'error',
): Promise<void> {
  const message = errorMessage(error);
  if (!message.trim()) return;
  try {
    await call('app_log', { level, scope, message });
  } catch {
    // 写不进日志就算了：真正的失败已经在界面上告诉用户了
  }
}
