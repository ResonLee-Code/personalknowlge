-- 决策缓存（D2 / N6：DecisionCache）。
--
-- 为什么要缓存：同一批材料在一次研究会话里会被反复判定（打分 / 冲突 / 后续步骤），
-- 每次都调模型既慢又费额度。缓存键是**内容寻址**的：
--
--   key = sha256(kind + 查询 + 每个候选的 (id, 标题, 片段, 时效) + 候选链的档位列表)
--
-- 于是：
-- - 材料内容一变，键就变 → **天然不命中**，不需要额外的失效逻辑；
-- - 用户改了模型配置（候选链变了）→ 键也变 → 不会拿旧档位的结果糊弄人；
-- - 另加 TTL 兜底（过期即视为没有），并提供 purge / clear 供重建索引时清理。
--
-- 只存派生结果：删掉整张表只是让下次重算一遍。

CREATE TABLE IF NOT EXISTS decision_cache (
    cache_key  TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    provider   TEXT NOT NULL,
    -- DecisionResult[] 的 JSON
    payload    TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    hits       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_decision_cache_expires ON decision_cache(expires_at);
