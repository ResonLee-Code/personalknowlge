//! SQLite 连接管理、迁移与事务。
//!
//! 设计文档 8.2：
//! - UI 不直接操作 SQLite
//! - 数据库通过迁移管理
//! - 数据库可重建（索引可重建，元数据可由 Markdown 重新扫描）

mod migrations;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, Transaction};

use crate::core::error::{AppError, ErrorCode, Result};

pub use migrations::{applied_migrations, run_migrations, MIGRATIONS, SCHEMA_VERSION};

/// 数据库句柄。
///
/// 使用单个连接 + `Mutex` 串行化访问：桌面应用是单用户场景，
/// 这样可以避免多连接写冲突，也便于保证保存流程的原子性。
pub struct Db {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
}

impl Db {
    /// 打开（或创建）数据库文件。
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        let db = Self { conn: Mutex::new(conn), path: Some(path.to_path_buf()) };
        db.migrate()?;
        Ok(db)
    }

    /// 内存数据库，供测试使用。
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        let db = Self { conn: Mutex::new(conn), path: None };
        db.migrate()?;
        Ok(db)
    }

    fn configure(conn: &Connection) -> Result<()> {
        // WAL 提升并发读性能；busy_timeout 避免瞬时锁冲突直接失败
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// 执行迁移。返回本次新应用的迁移版本列表。
    pub fn migrate(&self) -> Result<Vec<i64>> {
        self.with_conn(|conn| run_migrations(conn))
    }

    /// 当前 schema 版本。
    pub fn schema_version(&self) -> Result<i64> {
        self.with_conn(|conn| {
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap_or(0);
            Ok(version)
        })
    }

    /// 借用连接执行一段操作。**不要**在回调内部再调用 `with_conn`
    /// （同一个 `Mutex` 会死锁）。
    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| AppError::new(ErrorCode::DatabaseError, "数据库连接已损坏。"))?;
        f(&guard)
    }

    /// 在事务中执行一段操作。回调返回 `Err` 时自动回滚。
    pub fn with_transaction<T>(
        &self,
        f: impl FnOnce(&Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| AppError::new(ErrorCode::DatabaseError, "数据库连接已损坏。"))?;
        let tx = guard.transaction()?;
        let value = f(&tx)?;
        tx.commit()?;
        Ok(value)
    }

    /// 使用 `VACUUM INTO` 生成一致的数据库快照（用于备份）。
    pub fn snapshot_to(&self, target: &Path) -> Result<()> {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if target.exists() {
            std::fs::remove_file(target)?;
        }
        let target_str = target.to_string_lossy().to_string();
        self.with_conn(|conn| {
            conn.execute("VACUUM INTO ?1", rusqlite::params![target_str])?;
            Ok(())
        })
    }

    /// 检查数据库完整性。用于「数据库损坏后可重建」的提示。
    pub fn integrity_check(&self) -> Result<String> {
        self.with_conn(|conn| {
            let result: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            Ok(result)
        })
    }

    /// 清空所有业务数据（用于索引重建前的完全重置）。
    pub fn reset_content(&self) -> Result<()> {
        self.with_transaction(|tx| {
            tx.execute_batch(
                "DELETE FROM note_tags;
                 DELETE FROM notes;
                 DELETE FROM categories;
                 DELETE FROM tags;",
            )?;
            Ok(())
        })
    }
}
