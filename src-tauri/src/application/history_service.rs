//! 版本历史（V1.2.1 P2，对标语雀「查看历史版本」）。
//!
//! 快照存放在工作区内：`.knowledge/history/<note_id>/<时间戳>-<内容哈希前 8 位>.md`，
//! 与 Markdown 一样属于**用户可见的普通文件**——删掉数据库不影响历史，删掉历史也不影响笔记。
//!
//! 策略：
//! - 每次覆盖写入前，把**写入前**的内容存一份；
//! - 同一篇笔记 60 秒内只留一份（自动保存会每秒写一次，不做节流会瞬间堆出上百份）；
//! - 每篇最多保留 50 份，超出后按时间删最旧的；
//! - 内容与最新快照完全相同时跳过。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::hash::sha256_hex_str;
use crate::infrastructure::frontmatter;

use super::context::WorkspaceContext;

/// 同一篇笔记两次快照的最小间隔（秒）。
const MIN_INTERVAL_SECS: i64 = 60;
/// 每篇笔记保留的最大快照数。
const MAX_SNAPSHOTS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// 快照 id（文件名去掉扩展名），读取 / 还原时使用
    pub entry_id: String,
    /// 该快照对应的时间（ISO 8601）
    pub saved_at: String,
    pub bytes: u64,
    /// 正文首个 `#` 标题，便于在列表里辨认
    pub title: String,
    /// 正文前 120 字预览
    pub preview: String,
    /// 用户命名的版本名（对标语雀的「保存为版本」）；未命名时为 None
    #[serde(default)]
    pub label: Option<String>,
}

/// 版本名在同一目录下的存储文件（entryId → label）。
const LABELS_FILE: &str = "labels.json";
/// 版本名长度上限。
const MAX_LABEL_CHARS: usize = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryContent {
    pub entry_id: String,
    /// 完整 Markdown（含 frontmatter）
    pub content: String,
    /// 仅正文（还原时使用，保留当前标题 / 标签等元数据）
    pub body: String,
}

pub struct HistoryService {
    ctx: Arc<WorkspaceContext>,
}

impl HistoryService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    /// 写入前留档。`previous` 是**即将被覆盖**的完整 Markdown。
    ///
    /// 返回是否真的写了快照（便于测试与日志）。
    pub fn capture(&self, note_id: &str, previous: &str) -> Result<bool> {
        // 自动留档：遵守 60 秒节流，避免自动保存把历史刷爆
        Ok(self.capture_inner(note_id, previous, false, false)?.is_some())
    }

    /// 强制留档（还原、显式保存版本时使用，忽略 60 秒节流）。
    pub fn capture_forced(&self, note_id: &str, previous: &str) -> Result<bool> {
        Ok(self.capture_inner(note_id, previous, true, false)?.is_some())
    }

    /// 把当前内容存成**命名版本**（对标语雀的「保存为版本」）。
    ///
    /// 与自动留档的区别：忽略 60 秒节流，且**内容未变也会新建一份**（用户显式操作）。
    pub fn save_version(&self, note_id: &str, content: &str, label: &str) -> Result<HistoryEntry> {
        let trimmed = normalize_label(label)?;
        let entry_id = self
            .capture_inner(note_id, content, true, true)?
            .ok_or_else(|| AppError::new(ErrorCode::InternalError, "保存版本失败：未能写入快照。"))?;
        self.label(note_id, &entry_id, Some(&trimmed))?;
        self.list(note_id)?
            .into_iter()
            .find(|entry| entry.entry_id == entry_id)
            .ok_or_else(|| AppError::new(ErrorCode::InternalError, "保存版本失败：快照不可读。"))
    }

    /// 给已有快照命名；`label` 为空则清除命名。
    pub fn label(&self, note_id: &str, entry_id: &str, label: Option<&str>) -> Result<()> {
        let dir = self.note_dir(note_id)?;
        // 校验快照存在（不存在直接报错，避免写出悬空命名）
        let _ = self.entry_path(note_id, entry_id)?;
        let mut labels = self.read_labels(&dir);
        match label.map(str::trim).filter(|value| !value.is_empty()) {
            Some(value) => {
                labels.insert(entry_id.to_string(), normalize_label(value)?);
            }
            None => {
                labels.remove(entry_id);
            }
        }
        self.write_labels(&dir, &labels)
    }

    fn capture_inner(
        &self,
        note_id: &str,
        previous: &str,
        force: bool,
        force_new: bool,
    ) -> Result<Option<String>> {
        let dir = self.note_dir(note_id)?;
        let hash = sha256_hex_str(previous);
        let entries = self.list(note_id)?;

        if let Some(newest) = entries.first() {
            let newest_path = dir.join(format!("{}.md", newest.entry_id));
            if !force_new {
                if let Ok(text) = fs::read_to_string(&newest_path) {
                    if sha256_hex_str(&text) == hash {
                        return Ok(None); // 内容没变，不必重复留档
                    }
                }
            }
            if !force {
                if let Ok(meta) = fs::metadata(&newest_path) {
                    if let Ok(elapsed) = meta.modified().and_then(|time| {
                        time.elapsed()
                            .map_err(|error| std::io::Error::other(error.to_string()))
                    }) {
                        if elapsed.as_secs() < MIN_INTERVAL_SECS as u64 {
                            // 自动保存期间太密集：保留已有的那一份（它代表本次编辑开始前的状态）
                            return Ok(None);
                        }
                    }
                }
            }
        }

        fs::create_dir_all(&dir)?;
        let stamp = chrono::Local::now().format("%Y%m%dT%H%M%S%3f").to_string();
        let base_id = format!("{stamp}-{}", &hash[..8.min(hash.len())]);
        // 同名文件已存在（同毫秒 + 同内容再存一次）时追加序号，保证不覆盖
        let mut entry_id = base_id.clone();
        let mut suffix = 1;
        while dir.join(format!("{entry_id}.md")).exists() {
            suffix += 1;
            entry_id = format!("{base_id}-{suffix}");
        }
        self.ctx
            .storage
            .write_atomic_absolute(&dir.join(format!("{entry_id}.md")), previous.as_bytes())?;
        self.prune(note_id, &dir)?;
        Ok(Some(entry_id))
    }

    /// 读取版本名（容忍缺失与损坏，坏文件按「没有命名」处理）。
    fn read_labels(&self, dir: &Path) -> HashMap<String, String> {
        let path = dir.join(LABELS_FILE);
        let Ok(raw) = fs::read_to_string(&path) else {
            return HashMap::new();
        };
        serde_json::from_str::<HashMap<String, String>>(&raw).unwrap_or_default()
    }

    fn write_labels(&self, dir: &Path, labels: &HashMap<String, String>) -> Result<()> {
        let path = dir.join(LABELS_FILE);
        if labels.is_empty() {
            // 没有命名时不留空文件（保持目录干净）
            if path.exists() {
                fs::remove_file(&path)?;
            }
            return Ok(());
        }
        // 按 key 排序写出，保证文件内容稳定（便于人工查看与 diff）
        let mut sorted: Vec<(&String, &String)> = labels.iter().collect();
        sorted.sort();
        let map: serde_json::Map<String, serde_json::Value> = sorted
            .into_iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect();
        let json = serde_json::to_string_pretty(&serde_json::Value::Object(map))
            .unwrap_or_else(|_| "{}".to_string());
        self.ctx
            .storage
            .write_atomic_absolute(&path, format!("{json}\n").as_bytes())?;
        Ok(())
    }

    /// 列出快照（新的在前）。
    pub fn list(&self, note_id: &str) -> Result<Vec<HistoryEntry>> {
        let dir = self.note_dir(note_id)?;
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let labels = self.read_labels(&dir);
        let mut entries = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }

            let Some(entry_id) = path.file_stem().map(|stem| stem.to_string_lossy().to_string())
            else {
                continue;
            };
            let bytes = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            let saved_at = entry
                .metadata()
                .map(|meta| saved_at_from(&meta))
                .unwrap_or_default();
            let (title, preview) = match fs::read_to_string(&path) {
                Ok(text) => {
                    let parsed = frontmatter::parse(&text);
                    let title = parsed
                        .frontmatter
                        .title
                        .clone()
                        .or_else(|| {
                            parsed
                                .body
                                .lines()
                                .find_map(|line| line.trim().strip_prefix("# ").map(str::to_string))
                        })
                        .unwrap_or_default();
                    let preview: String = parsed
                        .body
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty() && !line.starts_with('#'))
                        .collect::<Vec<_>>()
                        .join(" ")
                        .chars()
                        .take(120)
                        .collect();
                    (title, preview)
                }
                Err(_) => (String::new(), String::new()),
            };
            let label = labels.get(&entry_id).cloned();
            entries.push(HistoryEntry { entry_id, saved_at, bytes, title, preview, label });
        }
        // 文件名以时间戳开头，字典序倒排即「新的在前」
        entries.sort_by(|left, right| right.entry_id.cmp(&left.entry_id));
        Ok(entries)
    }

    /// 读取某个快照。
    pub fn read(&self, note_id: &str, entry_id: &str) -> Result<HistoryContent> {
        let path = self.entry_path(note_id, entry_id)?;
        let content = fs::read_to_string(&path)?;
        let parsed = frontmatter::parse(&content);
        Ok(HistoryContent {
            entry_id: entry_id.to_string(),
            content,
            body: parsed.body,
        })
    }

    /// 删除某个快照。
    pub fn remove(&self, note_id: &str, entry_id: &str) -> Result<()> {
        let path = self.entry_path(note_id, entry_id)?;
        fs::remove_file(&path)?;
        Ok(())
    }

    /// 笔记被永久删除时清理它的历史目录。
    pub fn purge(&self, note_id: &str) -> Result<()> {
        let dir = self.note_dir(note_id)?;
        if dir.is_dir() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    fn prune(&self, note_id: &str, dir: &Path) -> Result<()> {
        let entries = self.list(note_id)?;
        for entry in entries.into_iter().skip(MAX_SNAPSHOTS) {
            let _ = fs::remove_file(dir.join(format!("{}.md", entry.entry_id)));
        }
        Ok(())
    }

    fn entry_path(&self, note_id: &str, entry_id: &str) -> Result<PathBuf> {
        let dir = self.note_dir(note_id)?;
        let trimmed = entry_id.trim();
        if trimmed.is_empty() || trimmed.contains(['/', '\\']) || trimmed.contains("..") {
            return Err(AppError::new(ErrorCode::InvalidInput, "版本 id 不合法。"));
        }
        let path = dir.join(format!("{}.md", trimmed.strip_suffix(".md").unwrap_or(trimmed)));
        if !path.is_file() {
            return Err(AppError::new(ErrorCode::NoteNotFound, "该历史版本已不存在。"));
        }
        Ok(path)
    }

    fn note_dir(&self, note_id: &str) -> Result<PathBuf> {
        let trimmed = note_id.trim();
        if trimmed.is_empty() || trimmed.contains(['/', '\\']) || trimmed.contains("..") {
            return Err(AppError::new(ErrorCode::InvalidInput, "笔记 id 不合法。"));
        }
        Ok(self.ctx.paths.note_history_dir(trimmed))
    }
}

/// 快照时间取文件的修改时间（写入即快照，避免再解析文件名）。
fn saved_at_from(metadata: &fs::Metadata) -> String {
    match metadata.modified() {
        Ok(time) => {
            let datetime: chrono::DateTime<chrono::Local> = time.into();
            datetime.to_rfc3339()
        }
        Err(_) => String::new(),
    }
}

/// 版本名规范化：去空白、限长、拒绝控制字符。
fn normalize_label(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput, "版本名不能为空。"));
    }
    let cleaned: String = trimmed.chars().filter(|ch| !ch.is_control()).collect();
    let limited: String = cleaned.chars().take(MAX_LABEL_CHARS).collect();
    let limited = limited.trim().to_string();
    if limited.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput, "版本名不合法。"));
    }
    Ok(limited)
}
