//! 分类仓储。

use rusqlite::{params, Connection, Row};

use crate::core::error::Result;
use crate::domain::category::Category;

pub trait CategoryRepository {
    fn insert(&self, conn: &Connection, category: &Category) -> Result<()>;
    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Category>>;
    fn list(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<Category>>;
    fn update(
        &self,
        conn: &Connection,
        id: &str,
        name: &str,
        dir_name: &str,
        parent_id: Option<&str>,
        sort_order: i64,
    ) -> Result<()>;
    fn update_sort_order(&self, conn: &Connection, id: &str, sort_order: i64) -> Result<()>;
    fn delete(&self, conn: &Connection, id: &str) -> Result<()>;
    fn next_sort_order(
        &self,
        conn: &Connection,
        workspace_id: &str,
        parent_id: Option<&str>,
    ) -> Result<i64>;
    fn name_exists(
        &self,
        conn: &Connection,
        workspace_id: &str,
        parent_id: Option<&str>,
        name: &str,
        exclude_id: Option<&str>,
    ) -> Result<bool>;
    fn descendants(&self, conn: &Connection, id: &str) -> Result<Vec<String>>;
    fn count(&self, conn: &Connection, workspace_id: &str) -> Result<i64>;
}

pub struct SqliteCategoryRepository;

const COLUMNS: &str =
    "id, workspace_id, parent_id, name, sort_order, COALESCE(dir_name, '') AS dir_name, created_at";

fn map_category(row: &Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        parent_id: row.get("parent_id")?,
        name: row.get("name")?,
        sort_order: row.get("sort_order")?,
        dir_name: row.get("dir_name")?,
        created_at: row.get("created_at")?,
        note_count: 0,
    })
}

impl CategoryRepository for SqliteCategoryRepository {
    fn insert(&self, conn: &Connection, category: &Category) -> Result<()> {
        conn.execute(
            "INSERT INTO categories (id, workspace_id, parent_id, name, sort_order, dir_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                category.id,
                category.workspace_id,
                category.parent_id,
                category.name,
                category.sort_order,
                category.dir_name,
                category.created_at
            ],
        )?;
        Ok(())
    }

    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Category>> {
        let sql = format!("SELECT {COLUMNS} FROM categories WHERE id = ?1");
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query_map(params![id], map_category)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<Category>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM categories WHERE workspace_id = ?1
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![workspace_id], map_category)?;
        let mut categories = Vec::new();
        for row in rows {
            categories.push(row?);
        }
        Ok(categories)
    }

    fn update(
        &self,
        conn: &Connection,
        id: &str,
        name: &str,
        dir_name: &str,
        parent_id: Option<&str>,
        sort_order: i64,
    ) -> Result<()> {
        conn.execute(
            "UPDATE categories
             SET name = ?2, dir_name = ?3, parent_id = ?4, sort_order = ?5
             WHERE id = ?1",
            params![id, name, dir_name, parent_id, sort_order],
        )?;
        Ok(())
    }

    fn update_sort_order(&self, conn: &Connection, id: &str, sort_order: i64) -> Result<()> {
        conn.execute(
            "UPDATE categories SET sort_order = ?2 WHERE id = ?1",
            params![id, sort_order],
        )?;
        Ok(())
    }

    fn delete(&self, conn: &Connection, id: &str) -> Result<()> {
        conn.execute("DELETE FROM categories WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn next_sort_order(
        &self,
        conn: &Connection,
        workspace_id: &str,
        parent_id: Option<&str>,
    ) -> Result<i64> {
        let order: i64 = match parent_id {
            Some(parent) => conn.query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM categories
                 WHERE workspace_id = ?1 AND parent_id = ?2",
                params![workspace_id, parent],
                |row| row.get(0),
            )?,
            None => conn.query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM categories
                 WHERE workspace_id = ?1 AND parent_id IS NULL",
                params![workspace_id],
                |row| row.get(0),
            )?,
        };
        Ok(order)
    }

    fn name_exists(
        &self,
        conn: &Connection,
        workspace_id: &str,
        parent_id: Option<&str>,
        name: &str,
        exclude_id: Option<&str>,
    ) -> Result<bool> {
        let mut sql = String::from(
            "SELECT COUNT(*) FROM categories
             WHERE workspace_id = ?1 AND lower(name) = lower(?2)
               AND IFNULL(parent_id, '') = IFNULL(?3, '')",
        );
        if exclude_id.is_some() {
            sql.push_str(" AND id <> ?4");
        }
        let mut statement = conn.prepare(&sql)?;
        let count: i64 = match exclude_id {
            Some(exclude) => {
                statement.query_row(params![workspace_id, name, parent_id, exclude], |row| row.get(0))?
            }
            None => statement.query_row(params![workspace_id, name, parent_id], |row| row.get(0))?,
        };
        Ok(count > 0)
    }

    fn descendants(&self, conn: &Connection, id: &str) -> Result<Vec<String>> {
        let mut statement = conn.prepare(
            "WITH RECURSIVE sub(id) AS (
                SELECT id FROM categories WHERE parent_id = ?1
                UNION ALL
                SELECT c.id FROM categories c JOIN sub ON c.parent_id = sub.id
             )
             SELECT id FROM sub",
        )?;
        let rows = statement.query_map(params![id], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    fn count(&self, conn: &Connection, workspace_id: &str) -> Result<i64> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}
