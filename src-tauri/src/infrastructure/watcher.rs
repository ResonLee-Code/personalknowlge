//! 文件监听（设计文档 19）。
//!
//! 只监听当前工作区中的 Markdown 文件与附件目录，
//! 忽略内部目录、隐藏文件与临时文件，并对事件做防抖与合并。
//!
//! 两种后端：
//! - `Native`：使用 `notify` 的平台实现（macOS FSEvents、Windows ReadDirectoryChangesW、Linux inotify）
//! - `Polling`：`notify::PollWatcher`，在受限环境（例如沙箱、网络盘）无法拿到原生事件时兜底
//!
//! 事件可能只精确到目录（FSEvents 常见），因此批次中同时携带目录列表，
//! 由索引服务按目录重新比对。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Event, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::paths::{normalize_relative, WorkspacePaths};

/// 防抖窗口：文件系统通常会在短时间内产生多个重复事件。
const DEBOUNCE: Duration = Duration::from_millis(600);
const POLL_INTERVAL: Duration = Duration::from_millis(150);
/// 兜底轮询间隔。
pub const FALLBACK_POLL_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileChangeKind {
    Created,
    Modified,
    Removed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub kind: FileChangeKind,
}

/// 一次批量变化。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeBatch {
    pub changes: Vec<FileChange>,
    /// 只能确定到目录粒度的变化，需要按目录重新比对。
    #[serde(default)]
    pub directories: Vec<String>,
    pub attachments_changed: bool,
}

/// 监听后端。
pub enum WatchMode {
    Native,
    Polling,
}

/// 监听后端句柄：这里只持有对象，用于在停止监听时释放它。
enum Backend {
    #[allow(dead_code)]
    Native(RecommendedWatcher),
    #[allow(dead_code)]
    Polling(PollWatcher),
}

/// 监听器句柄。丢弃或调用 `stop` 都会停止监听。
pub struct WatcherHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    backend: Option<Backend>,
}

impl WatcherHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // 先丢弃后端，停止产生新事件
        self.backend.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 事件汇聚状态。
struct Collector {
    pending_files: Mutex<BTreeMap<String, FileChangeKind>>,
    pending_directories: Mutex<BTreeSet<String>>,
    attachments_changed: AtomicBool,
    last_event_at: Mutex<Instant>,
}

impl Collector {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            pending_files: Mutex::new(BTreeMap::new()),
            pending_directories: Mutex::new(BTreeSet::new()),
            attachments_changed: AtomicBool::new(false),
            last_event_at: Mutex::new(Instant::now()),
        })
    }

    /// 记录一次文件系统事件。
    fn record(&self, root: &Path, event: &Event) {
        if let Ok(mut last) = self.last_event_at.lock() {
            *last = Instant::now();
        }
        for path in &event.paths {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = normalize_relative(&relative.to_string_lossy());
            if relative.is_empty() || WorkspacePaths::is_ignored_relative(&relative) {
                continue;
            }
            if relative.starts_with("attachments/") {
                self.attachments_changed.store(true, Ordering::SeqCst);
                continue;
            }
            if !WorkspacePaths::is_markdown_file(path) {
                // 事件只精确到目录时记录下来，稍后按目录比对
                let directory = if path.is_dir() {
                    relative.clone()
                } else {
                    match relative.rsplit_once('/') {
                        Some((parent, _)) => parent.to_string(),
                        None => String::new(),
                    }
                };
                if !directory.is_empty() && !WorkspacePaths::is_ignored_relative(&directory) {
                    if let Ok(mut guard) = self.pending_directories.lock() {
                        guard.insert(directory);
                    }
                }
                continue;
            }
            let kind = if !path.exists() {
                FileChangeKind::Removed
            } else if event.kind.is_create() {
                FileChangeKind::Created
            } else {
                FileChangeKind::Modified
            };
            if let Ok(mut guard) = self.pending_files.lock() {
                guard.insert(relative, kind);
            }
        }
    }

    /// 防抖窗口结束后取出待处理变化。
    fn take_batch(&self) -> FileChangeBatch {
        let changes = match self.pending_files.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(_) => BTreeMap::new(),
        };
        let directories = match self.pending_directories.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard).into_iter().collect(),
            Err(_) => Vec::new(),
        };
        FileChangeBatch {
            changes: changes
                .into_iter()
                .map(|(path, kind)| FileChange { path, kind })
                .collect(),
            directories,
            attachments_changed: self.attachments_changed.swap(false, Ordering::SeqCst),
        }
    }

    fn is_empty(&self) -> bool {
        self.pending_files
            .lock()
            .map(|guard| guard.is_empty())
            .unwrap_or(true)
            && self
                .pending_directories
                .lock()
                .map(|guard| guard.is_empty())
                .unwrap_or(true)
            && !self.attachments_changed.load(Ordering::SeqCst)
    }

    fn quiet_for(&self) -> Duration {
        self.last_event_at
            .lock()
            .map(|last| last.elapsed())
            .unwrap_or(Duration::ZERO)
    }
}

/// 启动原生文件监听。
pub fn start_watcher<F>(root: PathBuf, on_change: F) -> Result<WatcherHandle>
where
    F: Fn(FileChangeBatch) + Send + 'static,
{
    let collector = Collector::new();
    let collector_for_events = Arc::clone(&collector);
    let root_for_events = root.clone();

    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if let Ok(event) = result {
            collector_for_events.record(&root_for_events, &event);
        }
    })
    .map_err(|error| AppError::new(ErrorCode::IoError, format!("无法启动文件监听：{error}")))?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| AppError::new(ErrorCode::IoError, format!("监听目录失败：{error}")))?;

    Ok(build_handle(collector, Backend::Native(watcher), on_change))
}

/// 启动轮询式监听（兜底方案）。
pub fn start_polling_watcher<F>(root: PathBuf, on_change: F) -> Result<WatcherHandle>
where
    F: Fn(FileChangeBatch) + Send + 'static,
{
    let collector = Collector::new();
    let collector_for_events = Arc::clone(&collector);
    let root_for_events = root.clone();

    let config = notify::Config::default().with_poll_interval(FALLBACK_POLL_INTERVAL);
    let mut watcher = PollWatcher::new(
        move |result: notify::Result<Event>| {
            if let Ok(event) = result {
                collector_for_events.record(&root_for_events, &event);
            }
        },
        config,
    )
    .map_err(|error| AppError::new(ErrorCode::IoError, format!("无法启动轮询监听：{error}")))?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| AppError::new(ErrorCode::IoError, format!("监听目录失败：{error}")))?;

    Ok(build_handle(collector, Backend::Polling(watcher), on_change))
}

/// 自动选择可用的监听方式。
pub fn start_best_watcher<F>(root: PathBuf, on_change: F) -> Result<(WatcherHandle, WatchMode)>
where
    F: Fn(FileChangeBatch) + Send + 'static,
{
    if filesystem_events_available() {
        match start_watcher(root.clone(), on_change) {
            Ok(handle) => return Ok((handle, WatchMode::Native)),
            Err(error) => {
                tracing::warn!(target: "watcher", "原生文件监听不可用，改用轮询：{error}");
                return Err(error);
            }
        }
    }
    let handle = start_polling_watcher(root, on_change)?;
    Ok((handle, WatchMode::Polling))
}

fn build_handle<F>(collector: Arc<Collector>, backend: Backend, on_change: F) -> WatcherHandle
where
    F: Fn(FileChangeBatch) + Send + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);

    let thread = thread::Builder::new()
        .name("pkos-file-watcher".to_string())
        .spawn(move || loop {
            if stop_for_thread.load(Ordering::SeqCst) {
                break;
            }
            thread::sleep(POLL_INTERVAL);
            if collector.is_empty() || collector.quiet_for() < DEBOUNCE {
                continue;
            }
            let batch = collector.take_batch();
            if !batch.changes.is_empty()
                || !batch.directories.is_empty()
                || batch.attachments_changed
            {
                tracing::debug!(
                    target: "watcher",
                    files = batch.changes.len(),
                    directories = batch.directories.len(),
                    attachments = batch.attachments_changed,
                    "检测到外部文件变化"
                );
                on_change(batch);
            }
        })
        .expect("创建监听线程");

    WatcherHandle { stop, thread: Some(thread), backend: Some(backend) }
}

/// 检测当前环境是否能收到原生文件系统事件。
///
/// 某些受限环境（沙箱、部分网络文件系统）会静默丢弃事件，
/// 因此这里做一次写文件探测，失败则改用轮询。
pub fn filesystem_events_available() -> bool {
    let probe_dir = std::env::temp_dir().join(format!("pkos-watch-probe-{}", crate::core::ids::ulid()));
    if std::fs::create_dir_all(&probe_dir).is_err() {
        return false;
    }
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    let watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if result.is_ok() {
            let _ = sender.send(());
        }
    });
    let available = match watcher {
        Ok(mut watcher) => {
            let watching = watcher
                .watch(&probe_dir, RecursiveMode::NonRecursive)
                .is_ok();
            if watching {
                let probe_file = probe_dir.join("probe.md");
                let _ = std::fs::write(&probe_file, b"probe");
                let received = receiver.recv_timeout(Duration::from_millis(1500)).is_ok();
                let _ = std::fs::remove_file(&probe_file);
                received
            } else {
                false
            }
        }
        Err(_) => false,
    };
    let _ = std::fs::remove_dir_all(&probe_dir);
    if !available {
        tracing::warn!(target: "watcher", "当前环境无法接收文件系统事件，将使用轮询方式同步外部修改");
    }
    available
}

/// 递归扫描工作区，返回 Markdown 文件的（相对路径, 大小, 修改时间）。
///
/// 用于工作区打开时的比对，以及监听不可用时的兜底轮询。
pub fn scan_markdown_files(root: &Path) -> Vec<(String, u64, Option<String>)> {
    let paths = WorkspacePaths::new(root.to_path_buf());
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            !(entry.file_type().is_dir()
                && (name == crate::core::paths::KNOWLEDGE_DIR
                    || name == crate::core::paths::TRASH_DIR))
                && !(entry.file_type().is_file()
                    && crate::core::paths::is_temporary_file_name(&name))
        })
        .flatten()
    {
        if !entry.file_type().is_file() || !WorkspacePaths::is_markdown_file(entry.path()) {
            continue;
        }
        let Ok(relative) = paths.to_relative(entry.path()) else {
            continue;
        };
        if WorkspacePaths::is_ignored_relative(&relative) {
            continue;
        }
        let metadata = entry.metadata().ok();
        let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
        let modified = metadata
            .and_then(|meta| meta.modified().ok())
            .map(crate::core::time::system_time_to_iso);
        files.push((relative, size, modified));
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 诊断用：查看当前环境是否能收到原生事件。
    /// 默认忽略，需要时执行 `cargo test -- --ignored --nocapture`。
    #[test]
    #[ignore = "环境探测：验证文件系统事件支持"]
    fn raw_notify_events_arrive() {
        println!("原生文件系统事件可用：{}", filesystem_events_available());
    }

    #[test]
    fn scan_finds_markdown_and_ignores_internal_paths() {
        let dir = tempfile::tempdir().expect("临时目录");
        let root = dir.path();
        std::fs::create_dir_all(root.join("01_技术")).unwrap();
        std::fs::create_dir_all(root.join(".knowledge")).unwrap();
        std::fs::write(root.join("01_技术/a.md"), "a").unwrap();
        std::fs::write(root.join("b.markdown"), "b").unwrap();
        std::fs::write(root.join(".knowledge/c.md"), "c").unwrap();
        std::fs::write(root.join("01_技术/.pkos-tmp-x.tmp"), "temp").unwrap();
        std::fs::write(root.join("notes.txt"), "text").unwrap();

        let files = scan_markdown_files(root);
        let paths: Vec<String> = files.into_iter().map(|(path, _, _)| path).collect();
        assert_eq!(paths, vec!["01_技术/a.md".to_string(), "b.markdown".to_string()]);
    }
}
