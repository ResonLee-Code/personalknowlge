//! 类型化关系仓储（`note_relations` 表，N2）。
//!
//! 与 `note_links` 一样属于**派生数据**：事实来源是来源笔记 frontmatter 里的 `relations:`，
//! 重命名 / 删除 / 重建都会按正文重新生成。

use rusqlite::{params, Connection, Row};

use crate::core::error::Result;

/// 一行类型化关系。
#[derive(Debug, Clone)]
pub struct RelationRow {
    pub id: String,
    pub workspace_id: String,
    pub source_note_id: String,
    pub target_note_id: String,
    /// SUPERSEDES / CONTRADICTS / DERIVED_FROM / …
    pub relation_type: String,
    /// 用户写的说明（可选）
    pub note: Option<String>,
    pub position: i64,
    pub created_at: String,
}

pub trait RelationRepository {
    fn delete_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<()>;
    fn insert(
        &self,
        conn: &Connection,
        workspace_id: &str,
        source_note_id: &str,
        row: &RelationRow,
    ) -> Result<()>;
    fn list_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<Vec<RelationRow>>;
    fn list_for_target(&self, conn: &Connection, target_note_id: &str) -> Result<Vec<RelationRow>>;
    fn get(&self, conn: &Connection, id: &str) -> Result<Option<RelationRow>>;
    fn remove(&self, conn: &Connection, id: &str) -> Result<usize>;
    /// 目标笔记被删除/移入回收站：移除以它为目标的派生关系（frontmatter 仍保留声明）
    fn unbind_target(&self, conn: &Connection, target_note_id: &str) -> Result<usize>;
    /// 出现与标题同名的新笔记时，把未解析的声明重新绑定（当前按标题匹配重建即可）
    fn rebind_target(
        &self,
        conn: &Connection,
        workspace_id: &str,
        title: &str,
        note_id: &str,
    ) -> Result<usize>;
    fn clear_workspace(&self, conn: &Connection, workspace_id: &str) -> Result<()>;
}

pub struct SqliteRelationRepository;

const COLUMNS: &str =
    "id, workspace_id, source_note_id, target_note_id, relation_type, note, position, created_at";

fn map_row(row: &Row<'_>) -> rusqlite::Result<RelationRow> {
    Ok(RelationRow {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        source_note_id: row.get("source_note_id")?,
        target_note_id: row.get("target_note_id")?,
        relation_type: row.get("relation_type")?,
        note: row.get("note")?,
        position: row.get("position")?,
        created_at: row.get("created_at")?,
    })
}

impl RelationRepository for SqliteRelationRepository {
    fn delete_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM note_relations WHERE source_note_id = ?1",
            params![source_note_id],
        )?;
        Ok(())
    }

    fn insert(
        &self,
        conn: &Connection,
        workspace_id: &str,
        source_note_id: &str,
        row: &RelationRow,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO note_relations
                (id, workspace_id, source_note_id, target_note_id, relation_type, note, position, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.id,
                workspace_id,
                source_note_id,
                row.target_note_id,
                row.relation_type,
                row.note,
                row.position,
                row.created_at,
            ],
        )?;
        Ok(())
    }

    fn list_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<Vec<RelationRow>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM note_relations WHERE source_note_id = ?1 ORDER BY position ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![source_note_id], map_row)?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    fn list_for_target(&self, conn: &Connection, target_note_id: &str) -> Result<Vec<RelationRow>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM note_relations WHERE target_note_id = ?1 ORDER BY created_at ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![target_note_id], map_row)?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    fn get(&self, conn: &Connection, id: &str) -> Result<Option<RelationRow>> {
        let sql = format!("SELECT {COLUMNS} FROM note_relations WHERE id = ?1");
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query_map(params![id], map_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn remove(&self, conn: &Connection, id: &str) -> Result<usize> {
        Ok(conn.execute("DELETE FROM note_relations WHERE id = ?1", params![id])?)
    }

    fn unbind_target(&self, conn: &Connection, target_note_id: &str) -> Result<usize> {
        Ok(conn.execute(
            "DELETE FROM note_relations WHERE target_note_id = ?1",
            params![target_note_id],
        )?)
    }

    fn rebind_target(
        &self,
        _conn: &Connection,
        _workspace_id: &str,
        _title: &str,
        _note_id: &str,
    ) -> Result<usize> {
        // 关系按标题解析在写入路径完成；这里保留接口用于将来的「未解析关系」重绑。
        Ok(0)
    }

    fn clear_workspace(&self, conn: &Connection, workspace_id: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM note_relations WHERE workspace_id = ?1",
            params![workspace_id],
        )?;
        Ok(())
    }
}
