//! 工作区仓储。

use rusqlite::{params, Connection, Row};

use crate::core::error::Result;
use crate::domain::workspace::{Workspace, WorkspaceStats};

pub trait WorkspaceRepository {
    fn upsert(&self, conn: &Connection, workspace: &Workspace) -> Result<()>;
    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Workspace>>;
    fn get_by_root(&self, conn: &Connection, root_path: &str) -> Result<Option<Workspace>>;
    fn list(&self, conn: &Connection) -> Result<Vec<Workspace>>;
    fn delete(&self, conn: &Connection, id: &str) -> Result<()>;
    fn touch(&self, conn: &Connection, id: &str, updated_at: &str) -> Result<()>;
    fn stats(&self, conn: &Connection, workspace_id: &str) -> Result<WorkspaceStats>;
}

pub struct SqliteWorkspaceRepository;

fn map_workspace(row: &Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: row.get("id")?,
        name: row.get("name")?,
        root_path: row.get("root_path")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl WorkspaceRepository for SqliteWorkspaceRepository {
    fn upsert(&self, conn: &Connection, workspace: &Workspace) -> Result<()> {
        conn.execute(
            "INSERT INTO workspaces (id, name, root_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 root_path = excluded.root_path,
                 updated_at = excluded.updated_at",
            params![
                workspace.id,
                workspace.name,
                workspace.root_path,
                workspace.created_at,
                workspace.updated_at
            ],
        )?;
        Ok(())
    }

    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Workspace>> {
        let mut statement = conn.prepare(
            "SELECT id, name, root_path, created_at, updated_at FROM workspaces WHERE id = ?1",
        )?;
        let mut rows = statement.query_map(params![id], map_workspace)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn get_by_root(&self, conn: &Connection, root_path: &str) -> Result<Option<Workspace>> {
        let mut statement = conn.prepare(
            "SELECT id, name, root_path, created_at, updated_at
             FROM workspaces WHERE root_path = ?1",
        )?;
        let mut rows = statement.query_map(params![root_path], map_workspace)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list(&self, conn: &Connection) -> Result<Vec<Workspace>> {
        let mut statement = conn.prepare(
            "SELECT id, name, root_path, created_at, updated_at
             FROM workspaces ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], map_workspace)?;
        let mut workspaces = Vec::new();
        for row in rows {
            workspaces.push(row?);
        }
        Ok(workspaces)
    }

    fn delete(&self, conn: &Connection, id: &str) -> Result<()> {
        conn.execute("DELETE FROM workspaces WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn touch(&self, conn: &Connection, id: &str, updated_at: &str) -> Result<()> {
        conn.execute(
            "UPDATE workspaces SET updated_at = ?2 WHERE id = ?1",
            params![id, updated_at],
        )?;
        Ok(())
    }

    fn stats(&self, conn: &Connection, workspace_id: &str) -> Result<WorkspaceStats> {
        let note_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE workspace_id = ?1 AND status = 'active'",
            params![workspace_id],
            |row| row.get(0),
        )?;
        let inbox_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notes
             WHERE workspace_id = ?1 AND status = 'active' AND category_id IS NULL",
            params![workspace_id],
            |row| row.get(0),
        )?;
        let trash_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE workspace_id = ?1 AND status = 'deleted'",
            params![workspace_id],
            |row| row.get(0),
        )?;
        let category_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )?;
        let tag_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tags WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )?;
        // 统计明细（对标语雀「统计」页）：字数、近 7/30 天更新、置顶数。
        // 时间比较交给 SQLite 的 datetime()，与 ISO 8601 存储格式一致。
        let (total_words, updated_last_7d, updated_last_30d, pinned_count) = conn.query_row(
            "SELECT COALESCE(SUM(word_count), 0),
                    COALESCE(SUM(CASE WHEN updated_at >= datetime('now', '-7 days') THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN updated_at >= datetime('now', '-30 days') THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN pinned = 1 THEN 1 ELSE 0 END), 0)
             FROM notes WHERE workspace_id = ?1 AND status = 'active'",
            params![workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        Ok(WorkspaceStats {
            note_count,
            inbox_count,
            trash_count,
            category_count,
            tag_count,
            attachment_count: 0,
            total_words,
            updated_last_7d,
            updated_last_30d,
            pinned_count,
        })
    }
}
