//! 设置仓储：`settings` 表的键值封装（设计文档 11.2）。

use rusqlite::{params, Connection, OptionalExtension};

use crate::core::error::Result;
use crate::core::time::now_iso;
use crate::domain::settings::AppSettings;

const SETTINGS_KEY: &str = "app.settings";
pub const INDEX_STATE_KEY: &str = "index.state";
pub const INDEX_REBUILT_AT_KEY: &str = "index.last_rebuilt_at";
pub const INDEX_ERROR_KEY: &str = "index.last_error";

pub trait SettingsRepository {
    fn load(&self, conn: &Connection) -> Result<AppSettings>;
    fn save(&self, conn: &Connection, settings: &AppSettings) -> Result<()>;
    fn get_raw(&self, conn: &Connection, key: &str) -> Result<Option<String>>;
    fn set_raw(&self, conn: &Connection, key: &str, value: &str) -> Result<()>;
    fn remove_raw(&self, conn: &Connection, key: &str) -> Result<()>;
}

pub struct SqliteSettingsRepository;

impl SettingsRepository for SqliteSettingsRepository {
    fn load(&self, conn: &Connection) -> Result<AppSettings> {
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![SETTINGS_KEY],
                |row| row.get(0),
            )
            .optional()?;
        match raw {
            Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            None => Ok(AppSettings::default()),
        }
    }

    fn save(&self, conn: &Connection, settings: &AppSettings) -> Result<()> {
        let value = serde_json::to_string(settings)?;
        self.set_raw(conn, SETTINGS_KEY, &value)
    }

    fn get_raw(&self, conn: &Connection, key: &str) -> Result<Option<String>> {
        let value = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(value)
    }

    fn set_raw(&self, conn: &Connection, key: &str, value: &str) -> Result<()> {
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now_iso()],
        )?;
        Ok(())
    }

    fn remove_raw(&self, conn: &Connection, key: &str) -> Result<()> {
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }
}
