//! 日志（设计文档 21）。
//!
//! 规则：记录时间、模块、操作、错误码与简短上下文；
//! 不记录用户笔记正文与敏感信息。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const LOG_FILE_NAME: &str = "app.log";

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Clone)]
struct SharedFileWriter {
    file: Arc<Mutex<std::fs::File>>,
}

struct FileWriterGuard(Arc<Mutex<std::fs::File>>);

impl Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut file) => file.write(buf),
            Err(_) => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.0.lock() {
            Ok(mut file) => file.flush(),
            Err(_) => Ok(()),
        }
    }
}

impl<'a> MakeWriter<'a> for SharedFileWriter {
    type Writer = FileWriterGuard;

    fn make_writer(&'a self) -> Self::Writer {
        FileWriterGuard(self.file.clone())
    }
}

/// 初始化日志。重复调用只会生效一次。
pub fn init(log_dir: Option<&Path>) {
    let filter = EnvFilter::try_from_env("PKOS_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,personal_knowledge_os_lib=debug"));

    let file_writer = log_dir.and_then(open_log_file);

    match file_writer {
        Some(writer) => {
            let layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_writer(writer);
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(layer)
                .with(tracing_subscriber::fmt::layer().with_target(false))
                .try_init();
        }
        None => {
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_target(false))
                .try_init();
        }
    }

    tracing::info!(target: "app", "日志系统已初始化");
}

/// 把日志目录切换到工作区日志目录（幂等）。
pub fn set_log_directory(dir: &Path) {
    if let Some(writer) = open_log_file(dir) {
        LOG_PATH.get_or_init(|| dir.join(LOG_FILE_NAME));
        tracing::debug!(target: "app", "工作区日志目录：{}", dir.display());
        drop(writer);
    }
}

/// 当前日志文件路径（用于「打开日志」功能）。
pub fn log_file_path() -> Option<PathBuf> {
    LOG_PATH.get().cloned()
}

fn open_log_file(dir: &Path) -> Option<SharedFileWriter> {
    if std::fs::create_dir_all(dir).is_err() {
        return None;
    }
    let path = dir.join(LOG_FILE_NAME);
    rotate_if_needed(&path);
    let file = OpenOptions::new().create(true).append(true).open(&path).ok()?;
    Some(SharedFileWriter { file: Arc::new(Mutex::new(file)) })
}

fn rotate_if_needed(path: &Path) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.len() < MAX_LOG_BYTES {
        return;
    }
    let rotated = path.with_extension("log.1");
    let _ = std::fs::remove_file(&rotated);
    let _ = std::fs::rename(path, rotated);
}
