use std::path::{Path, PathBuf};

use personal_knowledge_os_lib::infrastructure::backup::{
    extract_archive, validate_archive, write_archive, BackupEntry, BackupManifest, BackupSource,
};

fn unsafe_archive(root: &Path) -> PathBuf {
    let archive = root.join("unsafe-backup.zip");
    let entries = vec![
        BackupEntry {
            name: "safe.md".to_string(),
            source: BackupSource::Bytes(b"# safe".to_vec()),
        },
        BackupEntry {
            name: "../escape.md".to_string(),
            source: BackupSource::Bytes(b"# escape".to_vec()),
        },
        BackupEntry {
            name: ".knowledge/database.sqlite".to_string(),
            source: BackupSource::Bytes(b"database".to_vec()),
        },
    ];
    let mut manifest = BackupManifest::new("workspace", "unsafe", 16);
    write_archive(&archive, &entries, &mut manifest).expect("构造恶意测试归档");
    archive
}

#[test]
fn validation_reports_unsafe_entry_paths() {
    let root = tempfile::tempdir().expect("临时目录");
    let archive = unsafe_archive(root.path());

    let validation = validate_archive(&archive).expect("校验归档");

    assert!(!validation.valid, "包含 ../ 的归档不能标为有效");
    assert!(
        validation.checks.iter().any(|check| {
            !check.passed && check.name.contains("路径")
        }),
        "失败原因必须明确指向危险路径：{:?}",
        validation.checks
    );
}

#[test]
fn extraction_rejects_all_paths_before_writing_any_file() {
    let root = tempfile::tempdir().expect("临时目录");
    let archive = unsafe_archive(root.path());
    let target = root.path().join("restored");

    extract_archive(&archive, &target).expect_err("路径穿越必须拒绝");

    if target.exists() {
        let entries = std::fs::read_dir(&target)
            .expect("读取目标目录")
            .collect::<Result<Vec<_>, _>>()
            .expect("收集目标目录");
        assert!(entries.is_empty(), "失败恢复不能留下半成品：{entries:?}");
    }
    assert!(!root.path().join("escape.md").exists());
}
