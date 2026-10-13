import { toast } from 'sonner';

/** 失败提示看板停留时间：比默认长，够读完一行原因（Track F2）。 */
const FAILURE_DURATION_MS = 8000;

/**
 * 统一的「动作结果反馈」——**失败时不能报成功**。
 *
 * 背景：各 store 的写操作会把错误收敛进自己的 `error` 状态而不是抛出（界面因此不会崩），
 * 但调用处若写成 `void action(id).then(() => toast.success(...))`，失败时也会弹出成功提示
 * ——用户会以为数据已经改了。约定：这些写操作返回 `boolean`，调用处用本函数统一反馈。
 *
 * Track F2（2026-10-14）：失败时把**完整原因**放进 toast 的 `description`（不再挤在标题里
 * 被截断），停留时间放长，并给一个「复制详情」动作——用户可以把原因直接贴进报障。
 * 同一条原因也会被后端写进应用日志（B3），两边按时间能对上。
 */
export function reportAction(
  ok: boolean,
  success: string,
  errorMessage?: string | null,
  fallbackError = '操作失败',
): void {
  if (ok) {
    toast.success(success);
    return;
  }
  reportFailure(fallbackError, errorMessage);
}

/**
 * 统一的**失败**提示（Track F2）：所有失败路径都走这里，别再各写各的 `toast.error`。
 *
 * 三件事一次做齐：
 * 1. 标题只放**摘要**（长原因不再挤在标题里被截断），完整原因放 `description`；
 * 2. 停留时间放长到 8 秒，够用户读完；
 * 3. 给一个「复制详情」动作——报障时直接贴出来。
 *
 * 同一条原因也会被后端写进应用日志（B3），两边按时间能对上。
 */
export function reportFailure(
  summary: string,
  detail?: string | null,
  options?: { duration?: number },
): void {
  const message = detail?.trim();
  if (!message) {
    toast.error(summary);
    return;
  }
  toast.error(summary, {
    description: message,
    duration: options?.duration ?? FAILURE_DURATION_MS,
    action: {
      label: '复制详情',
      onClick: () => {
        // 剪贴板不可用不该变成新的失败源：静默即可，文本在界面上仍可手动选中
        void navigator.clipboard?.writeText(`${summary}：${message}`).catch(() => undefined);
      },
    },
  });
}
