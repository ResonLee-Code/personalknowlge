//! 迁移表与迁移执行器。
//!
//! 迁移脚本以 SQL 文件形式保存在 `migrations/`，通过 `include_str!` 编译进二进制，
//! 保证发布版本不依赖外部文件。

use rusqlite::Connection;

use crate::core::error::Result;
use crate::core::time::now_iso;

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../../../migrations/001_init.sql"),
    },
    Migration {
        version: 2,
        name: "search_index",
        sql: include_str!("../../../migrations/002_search_index.sql"),
    },
    Migration {
        version: 3,
        name: "note_links",
        sql: include_str!("../../../migrations/003_note_links.sql"),
    },
    Migration {
        version: 4,
        name: "note_assets",
        sql: include_str!("../../../migrations/004_note_assets.sql"),
    },
    Migration {
        version: 5,
        name: "note_file_stamp",
        sql: include_str!("../../../migrations/005_note_file_stamp.sql"),
    },
    Migration {
        version: 6,
        name: "search_rowid_map",
        sql: include_str!("../../../migrations/006_search_rowid_map.sql"),
    },
    Migration {
        version: 7,
        name: "note_parent",
        sql: include_str!("../../../migrations/007_note_parent.sql"),
    },
];

pub const SCHEMA_VERSION: i64 = 7;

/// 创建迁移记录表。
fn ensure_migration_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

/// 已应用的迁移版本。
pub fn applied_migrations(conn: &Connection) -> Result<Vec<i64>> {
    ensure_migration_table(conn)?;
    let mut statement = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}

/// 依次执行未应用的迁移。返回本次应用的版本号。
pub fn run_migrations(conn: &Connection) -> Result<Vec<i64>> {
    ensure_migration_table(conn)?;
    let applied = applied_migrations(conn)?;
    let mut newly_applied = Vec::new();

    for migration in MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }
        tracing::info!(
            target: "db",
            version = migration.version,
            name = migration.name,
            "应用数据库迁移"
        );
        conn.execute_batch(migration.sql)?;
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![migration.version, migration.name, now_iso()],
        )?;
        newly_applied.push(migration.version);
    }

    let version = applied.iter().chain(newly_applied.iter()).copied().max().unwrap_or(0);
    conn.pragma_update(None, "user_version", version)?;
    Ok(newly_applied)
}
