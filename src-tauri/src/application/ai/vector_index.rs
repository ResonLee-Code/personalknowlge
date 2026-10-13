//! 向量索引：**增量构建** + 本地暴力检索（D3 / N7 第二步）。
//!
//! 取舍（写在这里免得以后再讨论一遍）：
//! - **向量存本地 SQLite**（`chunk_embeddings`），只有构建时把文本发出去一次；
//! - **暴力检索**（全量余弦）：不引入向量数据库依赖，10 万条以内够用；
//!   超过这个量级要重新评估（换 ANN 或分片）——这是**已知边界**，不是遗漏；
//! - **增量**：分块内容指纹没变就不重嵌（`content_hash`）；
//! - 任何一步出错都只是"向量通道不可用"，**检索照常用 BM25**。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use rusqlite::{params, Connection, OptionalExtension};

use crate::application::ai::embeddings::{EmbeddingProvider, MAX_INPUT_CHARS};
use crate::application::ai::policy::AiPolicyService;
use crate::application::ai::traits::{RetrievalOrigin, RetrievedChunk};
use crate::application::context::WorkspaceContext;
use crate::core::chunk::chunk_markdown;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::hash::sha256_hex_str;
use crate::core::time::now_iso;
use crate::domain::ai::{VectorIndexPreview, VectorIndexReport, VectorIndexStatus};
use crate::domain::note::NoteStatus;
use crate::repositories::settings_repository::{SqliteSettingsRepository, SettingsRepository};
use crate::repositories::{NoteRepository, SqliteNoteRepository};

/// 上次构建时间 / 构建用的模型（放设置表，随工作区走）。
const VECTOR_BUILT_AT_KEY: &str = "vector_index_built_at";
const VECTOR_MODEL_KEY: &str = "vector_index_model";

/// 一条待嵌入的分块。
struct PendingChunk {
    note_id: String,
    chunk_index: i64,
    text: String,
    content_hash: String,
}

/// 向量存储（都是派生数据，删了只是重算）。
pub struct VectorStore;

impl VectorStore {
    pub fn count(conn: &Connection) -> Result<i64> {
        Ok(conn.query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |row| row.get(0))?)
    }

    pub fn count_for_model(conn: &Connection, model: &str) -> Result<i64> {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM chunk_embeddings WHERE model = ?1",
            params![model],
            |row| row.get(0),
        )?)
    }

    pub fn clear(conn: &Connection) -> Result<usize> {
        Ok(conn.execute("DELETE FROM chunk_embeddings", [])?)
    }

    /// 该模型已有哪些分块的指纹（**增量构建**据此跳过没变的）。
    pub fn hashes(conn: &Connection, model: &str) -> Result<HashMap<(String, i64), String>> {
        let mut statement = conn.prepare(
            "SELECT note_id, chunk_index, content_hash FROM chunk_embeddings WHERE model = ?1",
        )?;
        let rows = statement.query_map(params![model], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, i64>(1)?),
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            map.insert(key, value);
        }
        Ok(map)
    }

    pub fn upsert(
        conn: &Connection,
        note_id: &str,
        chunk_index: i64,
        model: &str,
        vector: &[f32],
        content_hash: &str,
    ) -> Result<()> {
        let mut bytes = Vec::with_capacity(vector.len() * 4);
        for value in vector {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        conn.execute(
            "INSERT OR REPLACE INTO chunk_embeddings
                (note_id, chunk_index, model, dim, vector, content_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                note_id,
                chunk_index,
                model,
                vector.len() as i64,
                bytes,
                content_hash,
                now_iso()
            ],
        )?;
        Ok(())
    }

    /// 读出该模型的全部向量（暴力检索用）。
    pub fn load(conn: &Connection, model: &str) -> Result<Vec<(String, i64, Vec<f32>)>> {
        let mut statement = conn.prepare(
            "SELECT note_id, chunk_index, dim, vector FROM chunk_embeddings WHERE model = ?1",
        )?;
        let rows = statement.query_map(params![model], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (note_id, chunk_index, dim, bytes) = row?;
            let expected = (dim.max(0) as usize) * 4;
            if bytes.len() != expected || expected == 0 {
                // 坏数据（例如手工改过库）：跳过而不是整次失败
                continue;
            }
            let vector: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|slice| f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
                .collect();
            out.push((note_id, chunk_index, vector));
        }
        Ok(out)
    }
}

/// 余弦相似度（向量未归一化也能用；有一侧为零向量则返回 0）。
pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for (a, b) in left.iter().zip(right.iter()) {
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return 0.0;
    }
    dot / (left_norm.sqrt() * right_norm.sqrt())
}

/// 分块要嵌入的文本：原文切片，截断到上限并去掉首尾空白。
fn chunk_text(body: &str, start: usize, end: usize) -> String {
    let slice: String = body
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    crate::core::text::truncate_chars(slice.trim(), MAX_INPUT_CHARS)
}

pub struct VectorIndexService {
    ctx: Arc<WorkspaceContext>,
}

impl VectorIndexService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(Arc::clone(&self.ctx))
    }

    /// 当前状态：配置齐不齐、建了多少、什么时候建的。
    ///
    /// `provider_id` 由调用方给出**真正会用的**嵌入 Provider 标识
    /// （由它构造得出，而不是从策略里猜）——否则会出现"明明建好了却说不可用"。
    pub fn status(&self, provider_id: Option<&str>) -> Result<VectorIndexStatus> {
        let policy = self.ctx.settings().ai;
        let model = provider_id.map(str::to_string);
        let built_chunks = match &model {
            Some(model) => self
                .ctx
                .db
                .with_conn(|conn| VectorStore::count_for_model(conn, model))
                .unwrap_or(0),
            None => 0,
        };
        let total_chunks = self
            .ctx
            .db
            .with_conn(|conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM note_chunks", [], |row| row.get(0))?)
            })
            .unwrap_or(0);
        let built_at = self
            .ctx
            .db
            .with_conn(|conn| SqliteSettingsRepository.get_raw(conn, VECTOR_BUILT_AT_KEY))
            .ok()
            .flatten();
        let configured = policy.vector_enabled
            && model.is_some()
            && self.policy().remote_model().is_some()
            && policy.vector_authorized_at.is_some();
        let available = configured && built_chunks > 0;
        let note = if !policy.vector_enabled {
            "未开启向量检索（默认关闭；开启后只影响召回，不影响检索可用性）".to_string()
        } else if model.is_none() {
            "未配置嵌入模型（或在线模型未配置完整 / 未授权）".to_string()
        } else if !configured {
            "在线模型未配置完整或未授权（嵌入与在线模型共用地址与 Key）".to_string()
        } else if built_chunks == 0 {
            "尚未构建向量索引".to_string()
        } else {
            format!("已构建 {built_chunks} 条向量（模型 {model:?}）")
        };
        Ok(VectorIndexStatus {
            available,
            built_chunks,
            total_chunks,
            model,
            built_at,
            note,
        })
    }

    /// 构建**之前**的预览：要发多少、发给谁（给确认框用）。
    pub fn preview(&self, provider_id: Option<&str>) -> Result<VectorIndexPreview> {
        let policy = self.ctx.settings().ai;
        let blocked = |reason: &str| VectorIndexPreview {
            will_send: false,
            host: None,
            notes: 0,
            chunks_total: 0,
            chunks_to_embed: 0,
            reason: reason.to_string(),
        };
        if !policy.enabled {
            return Ok(blocked("AI 总开关关闭：不会外发任何内容。"));
        }
        if !policy.vector_enabled {
            return Ok(blocked("未开启向量检索。"));
        }
        let Some(model) = provider_id else {
            return Ok(blocked(
                "嵌入未就绪：未填嵌入模型名，或在线模型未配置完整 / 未授权。",
            ));
        };
        let model = model.to_string();
        if policy.vector_authorized_at.is_none() {
            return Ok(blocked("向量检索尚未授权：查询词会发给嵌入端点，需要在「设置 → AI」里确认一次。"));
        }

        let pending = self.pending_chunks()?;
        let existing = self
            .ctx
            .db
            .with_conn(|conn| VectorStore::hashes(conn, &model))?;
        let to_embed = pending
            .iter()
            .filter(|chunk| {
                existing.get(&(chunk.note_id.clone(), chunk.chunk_index)) != Some(&chunk.content_hash)
            })
            .count() as i64;
        let notes = pending
            .iter()
            .map(|chunk| chunk.note_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len() as i64;
        Ok(VectorIndexPreview {
            will_send: to_embed > 0,
            host: self
                .policy()
                .remote_diagnosis()
                .0
                .and_then(|(base, _)| super::host::host_of(&base)),
            notes,
            chunks_total: pending.len() as i64,
            chunks_to_embed: to_embed,
            reason: if to_embed > 0 {
                format!("本次会把 {to_embed} 个片段的文本发给嵌入端点（增量：内容没变的 {skipped} 个不会重发）", skipped = pending.len() as i64 - to_embed)
            } else {
                "所有分块的向量都是最新的，本次不需要外发。".to_string()
            },
        })
    }

    /// 构建（增量）。**必须显式确认**——这是把整库内容发出去的一次性动作。
    pub fn build(&self, provider: &dyn EmbeddingProvider, confirm: bool) -> Result<VectorIndexReport> {
        let started = Instant::now();
        if !confirm {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "构建向量索引会把笔记片段发到嵌入端点，需要显式确认后再执行。",
            ));
        }
        let policy = self.ctx.settings().ai;
        if !policy.enabled || !policy.vector_enabled {
            return Err(AppError::new(ErrorCode::AiDisabled, "向量检索未开启。"));
        }
        if policy.vector_authorized_at.is_none() {
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                "向量检索尚未授权：查询词与片段会发给嵌入端点，需先在「设置 → AI」里确认一次。",
            ));
        }
        if self.policy().remote_model().is_none() {
            return Err(AppError::new(
                ErrorCode::AiDisabled,
                "在线模型未配置完整（嵌入与在线模型共用地址与 Key）。",
            ));
        }

        let model = provider.id();
        let pending = self.pending_chunks()?;
        let total_chunks = pending.len() as i64;
        let existing = self
            .ctx
            .db
            .with_conn(|conn| VectorStore::hashes(conn, &model))?;
        let to_embed: Vec<&PendingChunk> = pending
            .iter()
            .filter(|chunk| {
                existing.get(&(chunk.note_id.clone(), chunk.chunk_index)) != Some(&chunk.content_hash)
            })
            .collect();
        let skipped = total_chunks - to_embed.len() as i64;

        let mut embedded = 0i64;
        let batch = crate::application::ai::embeddings::MAX_BATCH;
        for group in to_embed.chunks(batch) {
            let texts: Vec<String> = group.iter().map(|chunk| chunk.text.clone()).collect();
            let vectors = provider.embed(&texts)?;
            if vectors.len() != group.len() {
                return Err(AppError::internal("嵌入返回条数与请求不一致。"));
            }
            self.ctx.db.with_transaction(|tx| {
                for (chunk, vector) in group.iter().zip(vectors.iter()) {
                    VectorStore::upsert(
                        tx,
                        &chunk.note_id,
                        chunk.chunk_index,
                        &model,
                        vector,
                        &chunk.content_hash,
                    )?;
                }
                Ok(())
            })?;
            embedded += group.len() as i64;
            self.emit_progress(embedded, to_embed.len() as i64, skipped);
        }

        if embedded == 0 {
            self.emit_progress(0, 0, skipped);
        }
        self.ctx.db.with_conn(|conn| {
            SqliteSettingsRepository.set_raw(conn, VECTOR_BUILT_AT_KEY, &now_iso())?;
            SqliteSettingsRepository.set_raw(conn, VECTOR_MODEL_KEY, &model)
        })?;
        self.policy().audit(
            "vector_index_build",
            true,
            "ok",
            started.elapsed().as_millis() as u64,
            Some(&format!("embedded={embedded} skipped={skipped}")),
        );
        Ok(VectorIndexReport {
            model,
            total_chunks,
            embedded,
            skipped,
            took_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub fn clear(&self) -> Result<usize> {
        let removed = self.ctx.db.with_conn(|conn| VectorStore::clear(conn))?;
        self.ctx.db.with_conn(|conn| {
            SqliteSettingsRepository.set_raw(conn, VECTOR_BUILT_AT_KEY, "")?;
            SqliteSettingsRepository.set_raw(conn, VECTOR_MODEL_KEY, "")
        })?;
        Ok(removed)
    }

    /// 向量检索：把查询词嵌入 → 全量余弦 → top-k（再补上分块元信息）。
    pub fn search(
        &self,
        provider: &dyn EmbeddingProvider,
        query: &str,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let model = provider.id();
        let stored = self.ctx.db.with_conn(|conn| VectorStore::load(conn, &model))?;
        if stored.is_empty() {
            return Ok(Vec::new());
        }
        let query_vector = provider
            .embed(&[query.to_string()])?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::internal("嵌入端点没有返回查询向量。"))?;
        let mut scored: Vec<(f32, String, i64)> = stored
            .into_iter()
            .map(|(note_id, chunk_index, vector)| {
                (cosine_similarity(&query_vector, &vector), note_id, chunk_index)
            })
            .collect();
        scored.sort_by(|left, right| {
            right.0.partial_cmp(&left.0).unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit.max(1));

        let mut chunks = Vec::with_capacity(scored.len());
        for (score, note_id, chunk_index) in scored {
            if score <= 0.0 {
                continue;
            }
            let Some(row) = self.chunk_meta(&note_id, chunk_index)? else {
                continue;
            };
            let body = self.note_body(&row.file_path).unwrap_or_default();
            chunks.push(RetrievedChunk {
                note_id: note_id.clone(),
                chunk_index,
                heading_path: row
                    .heading
                    .split(" > ")
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect(),
                snippet: crate::core::text::truncate_chars(
                    &chunk_text(&body, row.char_start.max(0) as usize, row.char_end.max(0) as usize),
                    180,
                ),
                char_start: row.char_start,
                char_end: row.char_end,
                score,
                origin: RetrievalOrigin::Vector,
            });
        }
        Ok(chunks)
    }

    fn note_body(&self, file_path: &str) -> Option<String> {
        self.ctx.storage.read_note(file_path).ok().map(|stored| stored.body)
    }

    fn chunk_meta(&self, note_id: &str, chunk_index: i64) -> Result<Option<ChunkMeta>> {
        let meta = self.ctx.db.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT c.heading, c.char_start, c.char_end, n.file_path
                       FROM note_chunks c JOIN notes n ON n.id = c.note_id
                      WHERE c.note_id = ?1 AND c.chunk_index = ?2",
                    params![note_id, chunk_index],
                    |row| {
                        Ok(ChunkMeta {
                            heading: row.get(0)?,
                            char_start: row.get(1)?,
                            char_end: row.get(2)?,
                            file_path: row.get(3)?,
                        })
                    },
                )
                .optional()?)
        })?;
        Ok(meta)
    }

    /// 当前所有活跃笔记的分块（读取文件 + 分块 + 指纹）。
    fn pending_chunks(&self) -> Result<Vec<PendingChunk>> {
        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, &self.ctx.workspace.id, NoteStatus::Active)
        })?;
        let mut pending = Vec::new();
        for note in notes {
            let Ok(stored) = self.ctx.storage.read_note(&note.file_path) else {
                continue; // 读不到就跳过（与索引重建同口径：不因单篇失败而中断）
            };
            let body = stored.body;
            for chunk in chunk_markdown(&body) {
                let text = chunk_text(&body, chunk.char_start, chunk.char_end);
                if text.trim().is_empty() {
                    continue;
                }
                pending.push(PendingChunk {
                    note_id: note.id.clone(),
                    chunk_index: chunk.index as i64,
                    content_hash: sha256_hex_str(&text),
                    text,
                });
            }
        }
        Ok(pending)
    }

    fn emit_progress(&self, done: i64, total: i64, skipped: i64) {
        self.ctx.events.emit(
            event_names::VECTOR_INDEX_PROGRESS,
            serde_json::json!({ "done": done, "total": total, "skipped": skipped }),
        );
    }
}

struct ChunkMeta {
    heading: String,
    char_start: i64,
    char_end: i64,
    file_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_handles_edges() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
        assert!((cosine_similarity(&[2.0, 0.0], &[3.0, 0.0]) - 1.0).abs() < 1e-6, "只看方向");
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), 0.0, "零向量不算相似");
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0, "维度不同直接 0");
    }

    #[test]
    fn chunk_text_is_truncated_and_trimmed() {
        let body = "  abc  ";
        assert_eq!(chunk_text(body, 0, body.chars().count()), "abc");
        let long = "字".repeat(MAX_INPUT_CHARS + 50);
        assert!(chunk_text(&long, 0, long.chars().count()).chars().count() <= MAX_INPUT_CHARS + 1);
    }
}
