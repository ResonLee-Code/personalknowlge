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

use std::collections::HashSet;

use rusqlite::types::Value;
use rusqlite::OptionalExtension;
use rusqlite::{params_from_iter, Connection};

use crate::core::error::Result;
use crate::core::text::cjk_search_tokens;
use crate::domain::search::SearchSort;

/// 超过这个命中数时不再对整个结果集计算 BM25 全排序。
///
/// 10 万篇库中的“查询优化”这类高频词会命中几乎全库；此时 BM25 对首页的区分度
/// 很低，却要求 SQLite 为全部命中算分并排序。保留小结果集的完整相关度排序，
/// 高频结果改为按 FTS rowid 流式分页，以稳定控制交互延迟。
const MAX_RANKED_MATCHES: i64 = 5_000;

/// 4 个稳定分片。按 `note_id` 的 FNV-1a 哈希路由，保证重建前后位置可复现。
pub const SEARCH_SHARDS: [&str; 4] = [
    "search_documents_fts",
    "search_documents_fts_1",
    "search_documents_fts_2",
    "search_documents_fts_3",
];

fn shard_of(note_id: &str) -> usize {
    let hash = note_id
        .as_bytes()
        .iter()
        .fold(2_166_136_261u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(16_777_619)
        });
    hash as usize % SEARCH_SHARDS.len()
}

fn shard_table(shard: usize) -> &'static str {
    SEARCH_SHARDS[shard.min(SEARCH_SHARDS.len() - 1)]
}

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
    /// 高亮词，用于在 SQL 中计算全局“标题命中优先”排序
    pub title_terms: &'a [String],
    pub category_id: Option<&'a str>,
    pub tag_id: Option<&'a str>,
    pub favorite_only: bool,
    pub updated_after: Option<&'a str>,
    pub updated_before: Option<&'a str>,
    /// 限定单篇笔记（笔记内查找）
    pub scope_note_id: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
    /// 用户选择的全局排序。分页之前就在 SQL 中应用，不能只排当前页。
    pub sort: SearchSort,
}

/// 搜索索引接口，便于未来替换实现（设计文档 6.5）。
pub trait SearchIndex {
    fn upsert(&self, conn: &Connection, document: &SearchDocument) -> Result<()>;
    fn remove(&self, conn: &Connection, note_id: &str) -> Result<()>;
    fn clear(&self, conn: &Connection) -> Result<()>;
    fn optimize(&self, conn: &Connection) -> Result<()>;
    fn count(&self, conn: &Connection) -> Result<i64>;
    fn search(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<(Vec<FtsHit>, i64)>;
    fn count_matches(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<i64>;
}

/// 基于 SQLite FTS5 的实现。
pub struct SqliteSearchIndex;

impl SqliteSearchIndex {
    fn has_note_filters(request: &FtsRequest<'_>) -> bool {
        request.category_id.is_some()
            || request.tag_id.is_some()
            || request.favorite_only
            || request.updated_after.is_some()
            || request.updated_before.is_some()
            || request.scope_note_id.is_some()
    }

    fn build_where(table: &str, request: &FtsRequest<'_>, params: &mut Vec<Value>) -> String {
        let mut conditions = vec![format!("{table} MATCH ?")];
        params.push(Value::Text(request.match_expression.to_string()));
        conditions.extend(Self::note_conditions(request, params));
        conditions.join(" AND ")
    }

    fn note_conditions(request: &FtsRequest<'_>, params: &mut Vec<Value>) -> Vec<String> {
        let mut conditions = vec!["n.status = 'active'".to_string()];

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

        conditions
    }

    fn order_clause(sort: SearchSort) -> &'static str {
        // 排序键可能同时相同（例如同一秒写入）；追加 id 保证每页的边界可复现。
        match sort {
            SearchSort::Relevance => {
                "title_hit DESC, score ASC, updated_at DESC, note_id DESC"
            }
            SearchSort::UpdatedDesc => "updated_at DESC, note_id DESC",
            SearchSort::CreatedDesc => "created_at DESC, note_id DESC",
            SearchSort::TitleAsc => "title COLLATE NOCASE ASC, note_id ASC",
        }
    }

    fn notes_order_clause(sort: SearchSort) -> &'static str {
        match sort {
            SearchSort::UpdatedDesc => "updated_at DESC, id DESC",
            SearchSort::CreatedDesc => "created_at DESC, id DESC",
            SearchSort::TitleAsc => "title COLLATE NOCASE ASC, id ASC",
            SearchSort::Relevance => unreachable!("相关度排序不使用 notes 排序索引"),
        }
    }

    fn sort_index(sort: SearchSort) -> &'static str {
        match sort {
            SearchSort::UpdatedDesc => "idx_notes_search_updated",
            SearchSort::CreatedDesc => "idx_notes_search_created",
            SearchSort::TitleAsc => "idx_notes_search_title",
            SearchSort::Relevance => unreachable!("相关度排序不使用 notes 排序索引"),
        }
    }

    /// 显式排序：内存命中集合 + 沿覆盖索引按目标顺序扫描当前页。
    ///
    /// 直接对四个分片的全部命中做 UNION + 全排序需要约 300 ms（10 万篇实测）；
    /// 命中集合很大时，按用户顺序扫描 `notes` 并提前停止，避免把 10 万个字符串
    /// 物化到 SQLite 临时 B-tree，也避免对全部命中做跨分片排序。
    fn search_explicit_sorted(
        &self,
        conn: &Connection,
        request: &FtsRequest<'_>,
    ) -> Result<(Vec<FtsHit>, i64)> {
        let mut params: Vec<Value> = Vec::new();
        let mut selects = Vec::new();
        for table in SEARCH_SHARDS {
            let where_clause = Self::build_where(table, request, &mut params);
            selects.push(format!(
                "SELECT f.note_id
                 FROM {table} f
                 JOIN notes n ON n.id = f.note_id
                 WHERE {where_clause}"
            ));
        }
        let hit_sql = format!(
            "SELECT note_id FROM ({})",
            selects.join(" UNION ALL ")
        );
        let mut matched: HashSet<String> = HashSet::new();
        {
            let mut statement = conn.prepare(&hit_sql)?;
            let rows = statement.query_map(params_from_iter(params.iter()), |row| {
                row.get::<_, String>(0)
            })?;
            for row in rows {
                matched.insert(row?);
            }
        }
        let total = matched.len() as i64;

        if total <= MAX_RANKED_MATCHES {
            let ids: Vec<String> = matched.into_iter().collect();
            let placeholders = vec!["?"; ids.len()].join(", ");
            let sql = format!(
                "SELECT id, title, created_at, updated_at FROM notes WHERE id IN ({placeholders})"
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(ids.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            let mut sorted = Vec::new();
            for row in rows {
                sorted.push(row?);
            }
            match request.sort {
                SearchSort::UpdatedDesc => sorted.sort_by(|left, right| {
                    right.3.cmp(&left.3).then(right.0.cmp(&left.0))
                }),
                SearchSort::CreatedDesc => sorted.sort_by(|left, right| {
                    right.2.cmp(&left.2).then(right.0.cmp(&left.0))
                }),
                SearchSort::TitleAsc => sorted.sort_by(|left, right| {
                    left.1
                        .to_ascii_lowercase()
                        .cmp(&right.1.to_ascii_lowercase())
                        .then(left.0.cmp(&right.0))
                }),
                SearchSort::Relevance => unreachable!("显式排序不应收到相关度"),
            }
            let hits = sorted
                .into_iter()
                .skip(request.offset.max(0) as usize)
                .take(request.limit.max(0) as usize)
                .map(|(note_id, _, _, _)| FtsHit { note_id, score: 0.0 })
                .collect();
            return Ok((hits, total));
        }

        let mut note_params: Vec<Value> = Vec::new();
        let note_where = Self::note_conditions(request, &mut note_params).join(" AND ");
        let sql = format!(
            "SELECT n.id
             FROM notes n INDEXED BY {}
             WHERE {note_where}
             ORDER BY {}",
            Self::sort_index(request.sort),
            Self::notes_order_clause(request.sort),
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(note_params.iter()), |row| {
            row.get::<_, String>(0)
        })?;
        let mut skipped = 0i64;
        let mut hits = Vec::new();
        for row in rows {
            let note_id = row?;
            if !matched.contains(&note_id) {
                continue;
            }
            if skipped < request.offset {
                skipped += 1;
                continue;
            }
            hits.push(FtsHit { note_id, score: 0.0 });
            if hits.len() as i64 >= request.limit {
                break;
            }
        }
        Ok((hits, total))
    }

    fn count_matches_fast(conn: &Connection, request: &FtsRequest<'_>) -> Result<i64> {
        if !Self::has_note_filters(request) {
            let counts = SEARCH_SHARDS
                .iter()
                .enumerate()
                .map(|(index, table)| {
                    format!(
                        "(SELECT COUNT(*) FROM {table} WHERE {table} MATCH ?{index})",
                        index = index + 1
                    )
                })
                .collect::<Vec<_>>()
                .join(" + ");
            let params =
                vec![Value::Text(request.match_expression.to_string()); SEARCH_SHARDS.len()];
            let total: i64 = conn.query_row(
                &format!("SELECT {counts}"),
                params_from_iter(params),
                |row| row.get(0),
            )?;
            return Ok(total);
        }
        let mut total = 0i64;
        for table in SEARCH_SHARDS {
            let mut params: Vec<Value> = Vec::new();
            let where_clause = Self::build_where(table, request, &mut params);
            let sql = format!(
                "SELECT COUNT(*)
                 FROM {table} f
                 JOIN notes n ON n.id = f.note_id
                 WHERE {where_clause}"
            );
            let mut statement = conn.prepare(&sql)?;
            let count: i64 =
                statement.query_row(params_from_iter(params.iter()), |row| row.get(0))?;
            total = total.saturating_add(count);
        }
        Ok(total)
    }

    fn search_by_rowid(
        &self,
        conn: &Connection,
        request: &FtsRequest<'_>,
        total: i64,
    ) -> Result<(Vec<FtsHit>, i64)> {
        let mut params: Vec<Value> = Vec::new();
        let mut selects = Vec::new();
        for table in SEARCH_SHARDS {
            if !Self::has_note_filters(request) {
                params.push(Value::Text(request.match_expression.to_string()));
                selects.push(format!(
                    "SELECT note_id, 0.0 AS score
                     FROM {table}
                     WHERE {table} MATCH ?"
                ));
            } else {
                let where_clause = Self::build_where(table, request, &mut params);
                selects.push(format!(
                    "SELECT f.note_id AS note_id, 0.0 AS score
                     FROM {table} f
                     JOIN notes n ON n.id = f.note_id
                     WHERE {where_clause}"
                ));
            }
        }
        let sql = format!(
            "SELECT note_id, score FROM ({}) ORDER BY note_id ASC LIMIT ? OFFSET ?",
            selects.join(" UNION ALL ")
        );
        params.push(Value::Integer(request.limit));
        params.push(Value::Integer(request.offset));
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(params.iter()), |row| {
            Ok(FtsHit {
                note_id: row.get(0)?,
                score: row.get(1)?,
            })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok((hits, total))
    }
}

impl SearchIndex for SqliteSearchIndex {
    fn upsert(&self, conn: &Connection, document: &SearchDocument) -> Result<()> {
        // note_id 在 FTS5 里是 UNINDEXED 列：按它 DELETE 会全表扫描（O(N)），
        // 逐篇保存/采纳于是退化成 O(N²)。这里用映射表拿到 rowid，删除改按主键。
        // 注意 rowid 的分配也必须 O(1)：早先版本的 `MAX(row_id)+1` 同样是全表扫描。
        let existing: Option<(i64, usize)> = conn
            .query_row(
                "SELECT row_id, shard FROM search_documents_map WHERE note_id = ?1",
                rusqlite::params![document.note_id],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)?.max(0) as usize)),
            )
            .optional()?;
        let target_shard = shard_of(&document.note_id);
        let target_table = shard_table(target_shard);

        if let Some((row_id, shard)) = existing {
            conn.execute(
                &format!("DELETE FROM {} WHERE rowid = ?1", shard_table(shard)),
                rusqlite::params![row_id],
            )?;
        }

        // 不指定 rowid，交给 FTS5 自增分配，再用 last_insert_rowid 回填映射
        conn.execute(
            &format!(
                "INSERT INTO {target_table} (note_id, title, content, keywords)
                 VALUES (?1, ?2, ?3, ?4)"
            ),
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
                "UPDATE search_documents_map SET row_id = ?2, shard = ?3 WHERE note_id = ?1",
                rusqlite::params![document.note_id, row_id, target_shard as i64],
            )?;
        } else {
            conn.execute(
                "INSERT INTO search_documents_map (note_id, row_id, shard) VALUES (?1, ?2, ?3)",
                rusqlite::params![document.note_id, row_id, target_shard as i64],
            )?;
        }
        Ok(())
    }

    fn remove(&self, conn: &Connection, note_id: &str) -> Result<()> {
        let existing: Option<(i64, usize)> = conn
            .query_row(
                "SELECT row_id, shard FROM search_documents_map WHERE note_id = ?1",
                rusqlite::params![note_id],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)?.max(0) as usize)),
            )
            .optional()?;
        if let Some((row_id, shard)) = existing {
            conn.execute(
                &format!("DELETE FROM {} WHERE rowid = ?1", shard_table(shard)),
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
        for table in SEARCH_SHARDS {
            conn.execute(&format!("DELETE FROM {table}"), [])?;
        }
        conn.execute("DELETE FROM search_documents_map", [])?;
        Ok(())
    }

    fn optimize(&self, conn: &Connection) -> Result<()> {
        for table in SEARCH_SHARDS {
            conn.execute(
                &format!("INSERT INTO {table} ({table}) VALUES ('optimize')"),
                [],
            )?;
        }
        Ok(())
    }

    fn count(&self, conn: &Connection) -> Result<i64> {
        let mut total = 0i64;
        for table in SEARCH_SHARDS {
            let count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
            total = total.saturating_add(count);
        }
        Ok(total)
    }

    fn search(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<(Vec<FtsHit>, i64)> {
        if request.sort != SearchSort::Relevance {
            return self.search_explicit_sorted(conn, request);
        }
        let total = Self::count_matches_fast(conn, request)?;
        // 超高频相关度搜索仍然保留原有的流式分页降级，避免对全库计算 BM25。
        if total > MAX_RANKED_MATCHES {
            return self.search_by_rowid(conn, request, total);
        }
        let mut params: Vec<Value> = Vec::new();
        let mut selects = Vec::new();
        let order_by = Self::order_clause(request.sort);
        // 每个分片只保留足以组成全局页面的前缀；再对最多 4 个短列表做全局
        // 排序，避免高频命中时把 4×N 条记录全部送进外层次级排序。
        let per_shard_limit = request.offset.saturating_add(request.limit);
        for table in SEARCH_SHARDS {
            let title_hit = if request.sort == SearchSort::Relevance
                && !request.title_terms.is_empty()
            {
                let conditions = request
                    .title_terms
                    .iter()
                    .map(|_| "instr(lower(n.title), lower(?)) > 0")
                    .collect::<Vec<_>>()
                    .join(" OR ");
                params.extend(
                    request
                        .title_terms
                        .iter()
                        .cloned()
                        .map(Value::Text),
                );
                format!("CASE WHEN ({conditions}) THEN 1 ELSE 0 END")
            } else {
                "0".to_string()
            };
            let where_clause = Self::build_where(table, request, &mut params);
            let score = if request.sort == SearchSort::Relevance {
                format!("bm25({table}, 0.0, 6.0, 1.0, 2.0)")
            } else {
                "0.0".to_string()
            };
            selects.push(format!(
                "SELECT * FROM (
                     SELECT f.note_id AS note_id,
                            {score} AS score,
                            {title_hit} AS title_hit,
                            n.updated_at AS updated_at,
                            n.created_at AS created_at,
                            n.title AS title
                     FROM {table} f
                     JOIN notes n ON n.id = f.note_id
                     WHERE {where_clause}
                     ORDER BY {order_by}
                     LIMIT ?
                 )"
            ));
            params.push(Value::Integer(per_shard_limit));
        }
        let sql = format!(
            "SELECT note_id, score
             FROM ({})
             ORDER BY {}
             LIMIT ? OFFSET ?",
            selects.join(" UNION ALL "),
            order_by,
        );
        params.push(Value::Integer(request.limit));
        params.push(Value::Integer(request.offset));

        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(params.iter()), |row| {
            Ok(FtsHit {
                note_id: row.get(0)?,
                score: row.get(1)?,
            })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok((hits, total))
    }

    fn count_matches(&self, conn: &Connection, request: &FtsRequest<'_>) -> Result<i64> {
        Self::count_matches_fast(conn, request)
    }
}
