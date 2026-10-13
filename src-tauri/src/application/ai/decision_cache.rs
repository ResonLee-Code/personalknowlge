//! 决策缓存（D2 / N6）。
//!
//! 键是**内容寻址**的：`kind + 查询 + 每个候选的 (id, 标题, 片段, 时效) + 候选链的档位`。
//! 因此：
//! - 材料内容一变，键就变 → **天然不命中**，不需要额外的失效逻辑；
//! - 用户改了模型配置（候选链变了）→ 键也变，不会拿旧档位的结果糊弄人；
//! - TTL 只是兜底（材料没变、但结果也不该无限期复用）。

use rusqlite::{params, Connection, OptionalExtension};

use crate::application::ai::traits::{DecisionKind, DecisionResult, DecisionState};
use crate::core::error::{AppError, Result};
use crate::core::hash::sha256_hex_str;
use crate::core::time::{iso_after_secs, now_iso};

/// 缓存有效期（秒）。
pub const TTL_SECS: i64 = 600;

/// 命中缓存时拿回的东西。
#[derive(Debug, Clone)]
pub struct CachedDecision {
    pub provider: String,
    pub results: Vec<DecisionResult>,
}

pub struct DecisionCache;

impl DecisionCache {
    /// 内容寻址的缓存键。
    ///
    /// `chain` 是这一次可用的档位列表（例如 `["local.ollama", "rule"]`）——
    /// 把它算进键里，用户开关模型后不会命中旧档位的结果。
    pub fn key(kind: DecisionKind, state: &DecisionState, chain: &[String]) -> String {
        let mut canonical = String::new();
        canonical.push_str(&format!("{}\u{1}{}\n", kind.as_str(), state.query));
        for candidate in &state.candidates {
            canonical.push_str(&format!(
                "{}\u{1}{}\u{1}{}\u{1}{}\u{1}{:?}\u{1}{:?}\n",
                candidate.note_id,
                candidate.title,
                candidate.snippet,
                candidate.updated_at,
                candidate.source_reliability,
                candidate.valid_until,
            ));
        }
        canonical.push_str("chain\u{1}");
        for id in chain {
            canonical.push_str(id);
            canonical.push('\u{1}');
        }
        sha256_hex_str(&canonical)
    }

    /// 取缓存；**过期或坏数据一律当作没有**（顺手删掉，别让表越滚越大）。
    pub fn get(&self, conn: &Connection, key: &str) -> Result<Option<CachedDecision>> {
        let row = conn
            .query_row(
                "SELECT provider, payload, expires_at FROM decision_cache WHERE cache_key = ?1",
                params![key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((provider, payload, expires_at)) = row else {
            return Ok(None);
        };
        // 同一格式的 ISO 字符串可以直接比较（见 core::time::iso_after_secs）
        if expires_at <= now_iso() {
            Self::delete(conn, key)?;
            return Ok(None);
        }
        match serde_json::from_str::<Vec<DecisionResult>>(&payload) {
            Ok(results) => {
                conn.execute(
                    "UPDATE decision_cache SET hits = hits + 1 WHERE cache_key = ?1",
                    params![key],
                )?;
                Ok(Some(CachedDecision { provider, results }))
            }
            Err(_) => {
                // 坏数据（例如有人手工改过库）：当作没有，不留着坑人
                Self::delete(conn, key)?;
                Ok(None)
            }
        }
    }

    pub fn put(
        &self,
        conn: &Connection,
        key: &str,
        kind: DecisionKind,
        provider: &str,
        results: &[DecisionResult],
        ttl_secs: i64,
    ) -> Result<()> {
        let payload = serde_json::to_string(results)
            .map_err(|error| AppError::internal(format!("决策结果序列化失败：{error}")))?;
        conn.execute(
            "INSERT OR REPLACE INTO decision_cache
                (cache_key, kind, provider, payload, created_at, expires_at, hits)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            params![
                key,
                kind.as_str(),
                provider,
                payload,
                now_iso(),
                iso_after_secs(ttl_secs),
            ],
        )?;
        Ok(())
    }

    /// 清掉过期条目（返回清掉的条数）。
    pub fn purge_expired(&self, conn: &Connection) -> Result<usize> {
        Ok(conn.execute(
            "DELETE FROM decision_cache WHERE expires_at <= ?1",
            params![now_iso()],
        )?)
    }

    /// 全部清空（重建索引 / 换工作区时用）。
    pub fn clear(&self, conn: &Connection) -> Result<usize> {
        Ok(conn.execute("DELETE FROM decision_cache", [])?)
    }

    pub fn count(&self, conn: &Connection) -> Result<i64> {
        Ok(conn.query_row("SELECT COUNT(*) FROM decision_cache", [], |row| row.get(0))?)
    }

    fn delete(conn: &Connection, key: &str) -> Result<()> {
        conn.execute("DELETE FROM decision_cache WHERE cache_key = ?1", params![key])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::DecisionCandidate;

    fn state(snippet: &str) -> DecisionState {
        DecisionState {
            query: "Rust 异步调度".to_string(),
            candidates: vec![DecisionCandidate {
                note_id: "note_a".to_string(),
                title: "调度实现".to_string(),
                snippet: snippet.to_string(),
                updated_at: "2026-10-12T10:00:00+08:00".to_string(),
                source_reliability: Some(4),
                valid_until: None,
            }],
        }
    }

    fn chain() -> Vec<String> {
        vec!["local.ollama".to_string(), "rule".to_string()]
    }

    #[test]
    fn key_changes_when_candidate_content_changes() {
        let before = DecisionCache::key(DecisionKind::Relevance, &state("原文"), &chain());
        let after = DecisionCache::key(DecisionKind::Relevance, &state("改了内容"), &chain());
        assert_ne!(before, after, "内容变了键必须变，这是天然失效的地基");
    }

    #[test]
    fn key_changes_when_the_provider_chain_changes() {
        let base = DecisionCache::key(DecisionKind::Relevance, &state("原文"), &chain());
        let only_rule =
            DecisionCache::key(DecisionKind::Relevance, &state("原文"), &["rule".to_string()]);
        assert_ne!(base, only_rule, "用户开关模型后不该命中旧档位的结果");
    }

    #[test]
    fn key_is_stable_for_the_same_inputs_and_differs_across_kinds() {
        let first = DecisionCache::key(DecisionKind::Relevance, &state("原文"), &chain());
        let second = DecisionCache::key(DecisionKind::Relevance, &state("原文"), &chain());
        assert_eq!(first, second);
        assert_ne!(
            first,
            DecisionCache::key(DecisionKind::Utility, &state("原文"), &chain())
        );
    }
}
