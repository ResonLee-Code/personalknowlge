//! FTS5 全文搜索索引（设计文档 11.4 / 11.5 / 18）。
//!
//! 索引列：
//! - `title`：原始标题
//! - `content`：Markdown 正文抽取出的纯文本
//! - `keywords`：标签、分类路径，以及中日韩文字的单字与相邻双字扩展
//!
//! 中文策略：unicode61 分词器会把连续中文当成一个整词，
//! 因此额外写入「单字 + 双字」扩展，查询时把中文关键词转换成双字短语，
//! 从而既能支持单字查询，也能按连续短语精确匹配。

use rusqlite::OptionalExtension;
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};

use crate::core::error::Result;
use crate::core::text::cjk_search_tokens;

/// 写入索引的文档。
#[derive(Debug, Clone)]
pub struct SearchDocument {
    pub note_id: String,
    pub title: String,
    pub content: String,
    pub keywords: String,
}

impl SearchDocument {
    /// 由笔记内容构建索引文档。
    pub fn build(
        note_id: &str,
        title: &str,
        plain_text: &str,
        tags: &[String],
        category_path: &str,
    ) -> Self {
        let tags_text = tags.join(" ");
        let category_text = category_path.replace(['/', '\\'], " ");
        let expansion = cjk_search_tokens(&format!("{title} {plain_text} {tags_text}"));

        let mut keywords = String::new();
        keywords.push_str(&tags_text.to_lowercase());
        keywords.push(' ');
        keywords.push_str(&category_text);
        keywords.push(' ');
        keywords.push_str(&expansion);

        Self {
            note_id: note_id.to_string(),
            title: title.to_string(),
            content: plain_text.chars().take(200_000).collect(),
            keywords: keywords.trim().to_string(),
        }
    }
}

/// 一条 FTS 命中。
#[derive(Debug, Clone)]
pub struct FtsHit {
    pub note_id: String,
    /// BM25 分数（越小越相关，SQLite 返回负值）
    pub score: f64,
}

/// 查询请求。
#[derive(Debug, Clone, Default)]
pub struct FtsRequest<'a> {
    /// 已经过安全处理的 MATCH 表达式
    pub match_expression: &'a str,
    pub category_id: Option<&'a str>,
    pub tag_id: Option<&'a str>,
    pub favorite_only: bool,
    pub updated_after: Option<&'a str>,
    pub updated_before: Option<&'a str>,
    /// 限定单篇笔记（笔记内查找）
    pub scope_note_id: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
    /// true 时按相关度，否则按更新时间
    pub by_relevance: bool,
    pub sort_by_updated_desc: bool,
}

/// 搜索索引接口，便于未来替换实现（设计文档 6.5）。
pub trait SearchIndex {
    fn upsert(&self, conn: &Connection, document: &SearchDocument) -> Result<()>;
    fn remove(&self, conn: &Connection, note_id: &str) -> Result<()>;
    fn clear(&self, conn: &Connection) -> Result<()>;
    fn optimize(&self, conn: &Connection) -> Result<()>;
    fn count(&self, conn: &Connection) -> Result<i64>;
    fn search(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<Vec<FtsHit>>;
    fn count_matches(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<i64>;
}

/// 基于 SQLite FTS5 的实现。
pub struct SqliteSearchIndex;

impl SqliteSearchIndex {
    fn build_where(request: &FtsRequest<'_>, params: &mut Vec<Value>) -> String {
        let mut conditions = vec!["search_documents_fts MATCH ?".to_string()];
        params.push(Value::Text(request.match_expression.to_string()));
        conditions.push("n.status = 'active'".to_string());

        if let Some(category_id) = request.category_id {
            conditions.push("n.category_id = ?".to_string());
            params.push(Value::Text(category_id.to_string()));
        }
        if request.favorite_only {
            conditions.push("n.favorite = 1".to_string());
        }
        if let Some(after) = request.updated_after {
            conditions.push("n.updated_at >= ?".to_string());
            params.push(Value::Text(after.to_string()));
        }
        if let Some(before) = request.updated_before {
            conditions.push("n.updated_at <= ?".to_string());
            params.push(Value::Text(before.to_string()));
        }
        if let Some(note_id) = request.scope_note_id {
            conditions.push("n.id = ?".to_string());
            params.push(Value::Text(note_id.to_string()));
        }
        if let Some(tag_id) = request.tag_id {
            conditions.push(
                "EXISTS (SELECT 1 FROM note_tags nt WHERE nt.note_id = n.id AND nt.tag_id = ?)"
                    .to_string(),
            );
            params.push(Value::Text(tag_id.to_string()));
        }

        conditions.join(" AND ")
    }

    fn order_clause(request: &FtsRequest<'_>) -> &'static str {
        // 与笔记列表一致：追加 id 作为次级键，保证分页在「同分同时间」时也是确定的
        if request.by_relevance {
            "score ASC, n.updated_at DESC, n.id DESC"
        } else if request.sort_by_updated_desc {
            "n.updated_at DESC, n.id DESC"
        } else {
            "n.updated_at ASC, n.id ASC"
        }
    }
}

impl SearchIndex for SqliteSearchIndex {
    fn upsert(&self, conn: &Connection, document: &SearchDocument) -> Result<()> {
        // note_id 在 FTS5 里是 UNINDEXED 列：按它 DELETE 会全表扫描（O(N)），
        // 逐篇保存/采纳于是退化成 O(N²)。这里用映射表拿到 rowid，删除改按主键。
        // 注意 rowid 的分配也必须 O(1)：早先版本的 `MAX(row_id)+1` 同样是全表扫描。
        let existing: Option<i64> = conn
            .query_row(
                "SELECT row_id FROM search_documents_map WHERE note_id = ?1",
                rusqlite::params![document.note_id],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(row_id) = existing {
            conn.execute(
                "DELETE FROM search_documents_fts WHERE rowid = ?1",
                rusqlite::params![row_id],
            )?;
        }

        // 不指定 rowid，交给 FTS5 自增分配，再用 last_insert_rowid 回填映射
        conn.execute(
            "INSERT INTO search_documents_fts (note_id, title, content, keywords)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                document.note_id,
                document.title,
                document.content,
                document.keywords
            ],
        )?;
        let row_id = conn.last_insert_rowid();

        if existing.is_some() {
            conn.execute(
                "UPDATE search_documents_map SET row_id = ?2 WHERE note_id = ?1",
                rusqlite::params![document.note_id, row_id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO search_documents_map (note_id, row_id) VALUES (?1, ?2)",
                rusqlite::params![document.note_id, row_id],
            )?;
        }
        Ok(())
    }

    fn remove(&self, conn: &Connection, note_id: &str) -> Result<()> {
        let existing: Option<i64> = conn
            .query_row(
                "SELECT row_id FROM search_documents_map WHERE note_id = ?1",
                rusqlite::params![note_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(row_id) = existing {
            conn.execute(
                "DELETE FROM search_documents_fts WHERE rowid = ?1",
                rusqlite::params![row_id],
            )?;
            conn.execute(
                "DELETE FROM search_documents_map WHERE note_id = ?1",
                rusqlite::params![note_id],
            )?;
        }
        Ok(())
    }

    fn clear(&self, conn: &Connection) -> Result<()> {
        conn.execute("DELETE FROM search_documents_fts", [])?;
        conn.execute("DELETE FROM search_documents_map", [])?;
        Ok(())
    }

    fn optimize(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO search_documents_fts (search_documents_fts) VALUES ('optimize')",
            [],
        )?;
        Ok(())
    }

    fn count(&self, conn: &Connection) -> Result<i64> {
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM search_documents_fts", [], |row| row.get(0))?;
        Ok(count)
    }

    fn search(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<Vec<FtsHit>> {
        let mut params: Vec<Value> = Vec::new();
        let where_clause = Self::build_where(request, &mut params);
        let sql = format!(
            "SELECT f.note_id AS note_id,
                    bm25(search_documents_fts, 0.0, 6.0, 1.0, 2.0) AS score
             FROM search_documents_fts f
             JOIN notes n ON n.id = f.note_id
             WHERE {where_clause}
             ORDER BY {}
             LIMIT ? OFFSET ?",
            Self::order_clause(request)
        );
        params.push(Value::Integer(request.limit));
        params.push(Value::Integer(request.offset));

        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(params.iter()), |row| {
            Ok(FtsHit { note_id: row.get(0)?, score: row.get(1)? })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok(hits)
    }

    fn count_matches(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<i64> {
        let mut params: Vec<Value> = Vec::new();
        let where_clause = Self::build_where(request, &mut params);
        let sql = format!(
            "SELECT COUNT(*)
             FROM search_documents_fts f
             JOIN notes n ON n.id = f.note_id
             WHERE {where_clause}"
        );
        let mut statement = conn.prepare(&sql)?;
        let count: i64 = statement.query_row(params_from_iter(params.iter()), |row| row.get(0))?;
        Ok(count)
    }
}
