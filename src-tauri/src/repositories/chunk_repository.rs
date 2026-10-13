//! 分块仓储（`note_chunks` 表，N5）。
//!
//! 与 `note_links` / `note_relations` 一样属于**派生数据**：事实来源是 Markdown 正文，
//! 表里只存"分块在哪里"（标题路径 + 正文字符区间），删库重扫可完整重建。

use rusqlite::{params, Connection};

use crate::core::chunk::TextChunk;
use crate::core::error::Result;

/// 库里的一行分块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChunk {
    pub index: i64,
    pub heading: String,
    pub char_start: i64,
    pub char_end: i64,
}

pub trait ChunkRepository {
    /// 用新算出来的分块整体替换某篇笔记的分块（先删后插，同一事务内）。
    fn replace(&self, conn: &Connection, note_id: &str, chunks: &[TextChunk]) -> Result<()>;
    fn delete(&self, conn: &Connection, note_id: &str) -> Result<()>;
    fn list(&self, conn: &Connection, note_id: &str) -> Result<Vec<StoredChunk>>;
    /// 重建索引前先清空（与 FTS 同一个节奏）。
    fn clear(&self, conn: &Connection) -> Result<()>;
    fn count(&self, conn: &Connection) -> Result<i64>;
}

pub struct SqliteChunkRepository;

impl ChunkRepository for SqliteChunkRepository {
    fn replace(&self, conn: &Connection, note_id: &str, chunks: &[TextChunk]) -> Result<()> {
        conn.execute("DELETE FROM note_chunks WHERE note_id = ?1", params![note_id])?;
        if chunks.is_empty() {
            return Ok(());
        }
        let mut statement = conn.prepare(
            "INSERT OR REPLACE INTO note_chunks
                (note_id, chunk_index, heading, char_start, char_end)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for chunk in chunks {
            statement.execute(params![
                note_id,
                chunk.index as i64,
                chunk.heading,
                chunk.char_start as i64,
                chunk.char_end as i64,
            ])?;
        }
        Ok(())
    }

    fn delete(&self, conn: &Connection, note_id: &str) -> Result<()> {
        conn.execute("DELETE FROM note_chunks WHERE note_id = ?1", params![note_id])?;
        Ok(())
    }

    fn list(&self, conn: &Connection, note_id: &str) -> Result<Vec<StoredChunk>> {
        let mut statement = conn.prepare(
            "SELECT chunk_index, heading, char_start, char_end
               FROM note_chunks WHERE note_id = ?1 ORDER BY chunk_index ASC",
        )?;
        let rows = statement.query_map(params![note_id], |row| {
            Ok(StoredChunk {
                index: row.get(0)?,
                heading: row.get(1)?,
                char_start: row.get(2)?,
                char_end: row.get(3)?,
            })
        })?;
        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row?);
        }
        Ok(chunks)
    }

    fn clear(&self, conn: &Connection) -> Result<()> {
        conn.execute("DELETE FROM note_chunks", [])?;
        Ok(())
    }

    fn count(&self, conn: &Connection) -> Result<i64> {
        Ok(conn.query_row("SELECT COUNT(*) FROM note_chunks", [], |row| row.get(0))?)
    }
}
