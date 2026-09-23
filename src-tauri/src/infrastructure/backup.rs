//! 备份归档（设计文档 20）。
//!
//! 备份包结构：
//!
//! ```text
//! KnowledgeBackup_YYYYMMDD_HHmmss.zip
//! ├── manifest.json               备份清单（版本、数量、逐文件哈希）
//! ├── .knowledge/database.sqlite  数据库快照（VACUUM INTO）
//! ├── .knowledge/config.json      工作区配置
//! ├── 00_Inbox/xxx.md             全部 Markdown 文件（保持目录结构）
//! └── attachments/...             附件
//! ```
//!
//! 校验（`validate_archive`）会检查压缩包可打开、清单可解析、
//! 文件数量一致、数据库存在，并逐文件校验 SHA-256。

use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::hash::sha256_hex;
use crate::core::time::now_iso;
use crate::domain::backup::{BackupCheck, BackupValidation};

pub const MANIFEST_NAME: &str = "manifest.json";
pub const APP_NAME: &str = "Personal Knowledge OS";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 备份来源。
#[derive(Debug, Clone)]
pub enum BackupSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// 待写入压缩包的一个条目。
#[derive(Debug, Clone)]
pub struct BackupEntry {
    /// 压缩包内路径，始终使用 `/` 分隔
    pub name: String,
    pub source: BackupSource,
}

/// 清单中的文件记录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// 备份清单。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub app: String,
    pub app_version: String,
    pub schema_version: i64,
    pub workspace_id: String,
    pub workspace_name: String,
    pub created_at: String,
    pub note_count: i64,
    pub attachment_count: i64,
    pub includes_database: bool,
    pub files: Vec<ManifestFile>,
}

impl BackupManifest {
    pub fn new(workspace_id: &str, workspace_name: &str, schema_version: i64) -> Self {
        Self {
            app: APP_NAME.to_string(),
            app_version: APP_VERSION.to_string(),
            schema_version,
            workspace_id: workspace_id.to_string(),
            workspace_name: workspace_name.to_string(),
            created_at: now_iso(),
            note_count: 0,
            attachment_count: 0,
            includes_database: false,
            files: Vec::new(),
        }
    }
}

/// 写入压缩包，并在写入过程中填充清单（文件哈希、大小与数量统计）。
///
/// 采用流式写入 + 边写边算哈希的方式，避免把整个知识库读入内存。
pub fn write_archive(
    target: &Path,
    entries: &[BackupEntry],
    manifest: &mut BackupManifest,
) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(target)?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    manifest.files.clear();
    manifest.note_count = 0;
    manifest.attachment_count = 0;
    manifest.includes_database = false;

    for entry in entries {
        let name = normalize_entry_name(&entry.name);
        if name.is_empty() {
            continue;
        }
        if name.ends_with(".md") || name.ends_with(".markdown") {
            manifest.note_count += 1;
        }
        if name.starts_with("attachments/") {
            manifest.attachment_count += 1;
        }
        if name == ".knowledge/database.sqlite" {
            manifest.includes_database = true;
        }

        // 数据库快照本身已是高熵数据，直接存储更省时间
        let entry_options = if name.ends_with(".sqlite") { stored } else { options };
        writer
            .start_file(name.clone(), entry_options)
            .map_err(zip_error)?;

        let mut hashing = HashingWriter::new(&mut writer);
        match &entry.source {
            BackupSource::File(path) => {
                let mut source = fs::File::open(path)?;
                std::io::copy(&mut source, &mut hashing)?;
            }
            BackupSource::Bytes(bytes) => {
                hashing.write_all(bytes)?;
            }
        }
        let (sha256, size) = hashing.finish();
        manifest.files.push(ManifestFile { path: name, size, sha256 });
    }

    let manifest_json = serde_json::to_vec_pretty(manifest)?;
    writer
        .start_file(MANIFEST_NAME.to_string(), options)
        .map_err(zip_error)?;
    writer.write_all(&manifest_json)?;
    writer.finish().map_err(zip_error)?;
    Ok(())
}

/// 边写边计算 SHA-256 的写入包装。
struct HashingWriter<W: Write> {
    inner: W,
    hasher: sha2::Sha256,
    size: u64,
}

impl<W: Write> HashingWriter<W> {
    fn new(inner: W) -> Self {
        use sha2::Digest;
        Self { inner, hasher: sha2::Sha256::new(), size: 0 }
    }

    fn finish(self) -> (String, u64) {
        use sha2::Digest;
        let digest = self.hasher.finalize();
        let mut hash = String::with_capacity(digest.len() * 2);
        for byte in digest {
            hash.push_str(&format!("{byte:02x}"));
        }
        (hash, self.size)
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use sha2::Digest;
        self.hasher.update(buf);
        self.size += buf.len() as u64;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// 读取清单。
pub fn read_manifest(archive: &Path) -> Result<BackupManifest> {
    let file = fs::File::open(archive)?;
    let mut zip = ZipArchive::new(file).map_err(zip_error)?;
    let mut entry = zip
        .by_name(MANIFEST_NAME)
        .map_err(|_| AppError::new(ErrorCode::BackupFailed, "备份缺少清单文件 manifest.json。"))?;
    let mut buffer = Vec::new();
    entry.read_to_end(&mut buffer)?;
    let manifest: BackupManifest = serde_json::from_slice(&buffer).map_err(|error| {
        AppError::new(ErrorCode::BackupFailed, format!("备份清单解析失败：{error}"))
    })?;
    Ok(manifest)
}

/// 归档中的全部条目名。
pub fn entry_names(archive: &Path) -> Result<Vec<String>> {
    let file = fs::File::open(archive)?;
    let mut zip = ZipArchive::new(file).map_err(zip_error)?;
    let mut names = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(zip_error)?;
        if !entry.is_dir() {
            names.push(entry.name().to_string());
        }
    }
    Ok(names)
}

/// 校验备份包（设计文档 20.4）。
pub fn validate_archive(archive: &Path) -> Result<BackupValidation> {
    let mut checks: Vec<BackupCheck> = Vec::new();

    let file = match fs::File::open(archive) {
        Ok(file) => file,
        Err(error) => {
            checks.push(check("压缩包可打开", false, &format!("无法打开：{error}")));
            return Ok(BackupValidation {
                valid: false,
                checks,
                note_count: 0,
                attachment_count: 0,
                created_at: None,
                workspace_name: None,
            });
        }
    };
    checks.push(check("压缩包可打开", true, "已成功读取归档"));

    let mut zip = match ZipArchive::new(file) {
        Ok(zip) => zip,
        Err(error) => {
            checks.push(check("ZIP 结构有效", false, &format!("解析失败：{error}")));
            return Ok(BackupValidation {
                valid: false,
                checks,
                note_count: 0,
                attachment_count: 0,
                created_at: None,
                workspace_name: None,
            });
        }
    };
    checks.push(check("ZIP 结构有效", true, &format!("共 {} 个条目", zip.len())));

    let manifest = match read_manifest_from(&mut zip) {
        Ok(manifest) => {
            checks.push(check("清单文件存在", true, "manifest.json 解析成功"));
            manifest
        }
        Err(error) => {
            checks.push(check("清单文件存在", false, &error.message));
            return Ok(BackupValidation {
                valid: false,
                checks,
                note_count: 0,
                attachment_count: 0,
                created_at: None,
                workspace_name: None,
            });
        }
    };

    let mut note_count = 0i64;
    let mut attachment_count = 0i64;
    let mut database_present = false;
    for name in entry_names_from(&mut zip)? {
        if name.ends_with(".md") || name.ends_with(".markdown") {
            note_count += 1;
        }
        if name.starts_with("attachments/") {
            attachment_count += 1;
        }
        if name == ".knowledge/database.sqlite" {
            database_present = true;
        }
    }

    checks.push(check(
        "数据库存在",
        database_present,
        if database_present { "包含 .knowledge/database.sqlite" } else { "缺少数据库文件" },
    ));
    checks.push(check(
        "Markdown 数量一致",
        note_count == manifest.note_count,
        &format!("清单 {} 个，实际 {} 个", manifest.note_count, note_count),
    ));
    checks.push(check(
        "附件数量一致",
        attachment_count == manifest.attachment_count,
        &format!("清单 {} 个，实际 {} 个", manifest.attachment_count, attachment_count),
    ));

    // 逐文件校验大小与哈希
    let mut mismatched: Vec<String> = Vec::new();
    for expected in &manifest.files {
        match read_entry_bytes(&mut zip, &expected.path) {
            Some(bytes) => {
                let hash = sha256_hex(&bytes);
                if hash != expected.sha256 || bytes.len() as u64 != expected.size {
                    mismatched.push(expected.path.clone());
                }
            }
            None => mismatched.push(expected.path.clone()),
        }
    }
    let integrity_detail = if mismatched.is_empty() {
        format!("已校验 {} 个文件", manifest.files.len())
    } else {
        format!("{} 个文件不一致：{}", mismatched.len(), mismatched.join(", "))
    };
    checks.push(check("文件完整性", mismatched.is_empty(), &integrity_detail));

    let valid = checks.iter().all(|item| item.passed);
    Ok(BackupValidation {
        valid,
        checks,
        note_count,
        attachment_count,
        created_at: Some(manifest.created_at),
        workspace_name: Some(manifest.workspace_name),
    })
}

/// 解压备份到目标目录。目标目录必须不存在或为空。
pub fn extract_archive(archive: &Path, target: &Path) -> Result<usize> {
    if target.exists() {
        let is_empty = fs::read_dir(target)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            return Err(AppError::new(
                ErrorCode::BackupFailed,
                "目标目录已存在且不为空，为避免覆盖数据，请换一个目录。",
            ));
        }
    }
    fs::create_dir_all(target)?;

    let file = fs::File::open(archive)?;
    let mut zip = ZipArchive::new(file).map_err(zip_error)?;
    let mut count = 0usize;

    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(zip_error)?;
        if entry.is_dir() {
            continue;
        }
        let name = normalize_entry_name(entry.name());
        if name.is_empty() || name == MANIFEST_NAME {
            continue;
        }
        if name.split('/').any(|part| part == "..") || Path::new(&name).is_absolute() {
            return Err(AppError::new(
                ErrorCode::PathInvalid,
                format!("备份中包含非法路径：{name}"),
            ));
        }
        let destination = target.join(&name);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&destination)?;
        std::io::copy(&mut entry, &mut output)?;
        count += 1;
    }
    Ok(count)
}

fn read_manifest_from<R: Read + Seek>(zip: &mut ZipArchive<R>) -> Result<BackupManifest> {
    let mut entry = zip
        .by_name(MANIFEST_NAME)
        .map_err(|_| AppError::new(ErrorCode::BackupFailed, "备份缺少清单文件 manifest.json。"))?;
    let mut buffer = Vec::new();
    entry.read_to_end(&mut buffer)?;
    serde_json::from_slice(&buffer).map_err(|error| {
        AppError::new(ErrorCode::BackupFailed, format!("备份清单解析失败：{error}"))
    })
}

fn entry_names_from<R: Read + Seek>(zip: &mut ZipArchive<R>) -> Result<Vec<String>> {
    let mut names = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(zip_error)?;
        if !entry.is_dir() {
            names.push(entry.name().to_string());
        }
    }
    Ok(names)
}

fn read_entry_bytes<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    let mut buffer = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buffer).ok()?;
    Some(buffer)
}

fn check(name: &str, passed: bool, detail: &str) -> BackupCheck {
    BackupCheck { name: name.to_string(), passed, detail: detail.to_string() }
}

fn normalize_entry_name(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches('/').to_string()
}

fn zip_error(error: zip::result::ZipError) -> AppError {
    AppError::new(ErrorCode::BackupFailed, format!("备份归档处理失败：{error}"))
}
