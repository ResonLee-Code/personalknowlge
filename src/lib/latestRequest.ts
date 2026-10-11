/**
 * 「最后发起的请求才算数」守卫（异步竞态防护）。
 *
 * 为什么需要：快速切换笔记、连续输入关键词时，先发出的请求可能**后返回**。
 * 如果拿到结果就无条件写进 store，旧结果会覆盖新状态——用户看到的是
 * 「列表/选中的是 B，正文或结果却是 A」这种错位。
 *
 * 用法：
 * ```ts
 * const guard = createRequestGuard();
 * const isLatest = guard.begin();
 * const data = await fetchSomething();
 * if (!isLatest()) return; // 期间又发起了新请求，丢弃这次结果
 * setState(data);
 * ```
 */
export function createRequestGuard() {
  let current = 0;
  return {
    /** 标记一次新请求开始；返回「这次请求是否仍是最新」的判定函数。 */
    begin(): () => boolean {
      current += 1;
      const token = current;
      return () => token === current;
    },
  };
}
