import { toast } from 'sonner';

/**
 * 统一的「动作结果反馈」——**失败时不能报成功**。
 *
 * 背景：各 store 的写操作会把错误收敛进自己的 `error` 状态而不是抛出（界面因此不会崩），
 * 但调用处若写成 `void action(id).then(() => toast.success(...))`，失败时也会弹出成功提示
 * ——用户会以为数据已经改了。约定：这些写操作返回 `boolean`，调用处用本函数统一反馈。
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
  const detail = errorMessage?.trim();
  toast.error(detail ? `${fallbackError}：${detail}` : fallbackError);
}
