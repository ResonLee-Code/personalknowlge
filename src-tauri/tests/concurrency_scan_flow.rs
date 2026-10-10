//! 并发复现：**显式扫描**与**文件监听**同时采纳同一批文件（V1.2.1 阶段 B）。
//!
//! 历史缺陷（见 CHANGELOG V1.1.0「3000 篇基准」小节）：两侧都走「查不存在 → 插入」，
//! 并发时同一路径被采纳两次，触发
//! `UNIQUE constraint failed: notes.workspace_id, notes.file_path`，
//! 两侧扫描互相中止（3000 篇时 active 卡在 375）。
//!
//! 本测试先只做**复现与证据**：不修改实现，跑多轮并输出每一轮的结果。
//! 若稳定通过，说明批量采纳 / mtime 短路等后续改动已消除竞态，可据此关闭该限制项；
//! 若失败，则按「扫描单飞队列」或「插入冲突幂等」修复。

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::infrastructure::watcher::{
    FileChange, FileChangeBatch, FileChangeKind,
};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

/// 每轮写入的文件数：足够触发批量阈值（50）并让两个线程都「看得到」。
const FILES_PER_ROUND: usize = 400;
const ROUNDS: usize = 4;

struct Fixture {
    _data: TempDir,
    workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create(name, &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    /// 直接写文件（模拟外部编辑器 / 整目录拷入），返回相对路径列表。
    fn write_external_files(&self, round: usize) -> Vec<String> {
        let mut paths = Vec::with_capacity(FILES_PER_ROUND);
        for index in 0..FILES_PER_ROUND {
            let relative = format!("00_Inbox/并发-{round}-{index}.md");
            let absolute = self.workspace.path().join(&relative);
            if let Some(parent) = absolute.parent() {
                std::fs::create_dir_all(parent).expect("创建源目录");
            }
            std::fs::write(
                &absolute,
                format!(
                    "---\nid: note_conc_{round}_{index}\ntitle: 并发 {round}-{index}\n---\n\n正文 {index}\n"
                ),
            )
            .expect("写入文件");
            paths.push(relative);
        }
        paths
    }

    fn active_count(&self) -> i64 {
        self.app
            .context()
            .expect("工作区已打开")
            .db
            .with_conn(|conn| {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM notes WHERE status = 'active'",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count)
            })
            .expect("统计")
    }
}

#[test]
fn concurrent_scan_and_watch_adopt_the_same_batch() {
    let mut failures: Vec<String> = Vec::new();
    for round in 0..ROUNDS {
        let fixture = Fixture::new("并发测试");
        let context = fixture.app.context().expect("工作区已打开");
        let paths = fixture.write_external_files(round);
        let baseline = fixture.active_count();

        // 监听侧：把「整目录拷入」当成一次批量 Created 事件
        let batch = FileChangeBatch {
            changes: paths
                .iter()
                .map(|path| FileChange { path: path.clone(), kind: FileChangeKind::Created })
                .collect(),
            directories: Vec::new(),
            attachments_changed: false,
        };

        // 扫描侧与监听侧**同刻起跑**（屏障对齐，否则一侧会先跑完，测不出重叠窗口）
        let start_line = Arc::new(Barrier::new(3));
        let scan_context = Arc::clone(&context);
        let watch_context = Arc::clone(&context);
        let scan_line = Arc::clone(&start_line);
        let watch_line = Arc::clone(&start_line);
        let started = Instant::now();
        let scan = thread::spawn(move || {
            scan_line.wait();
            IndexService::new(scan_context).scan_workspace()
        });
        let watch = thread::spawn(move || {
            watch_line.wait();
            IndexService::new(watch_context).apply_file_changes(batch)
        });
        start_line.wait();
        let scan_result = scan.join().expect("扫描线程未 panic");
        let watch_result = watch.join().expect("监听线程未 panic");
        let elapsed = started.elapsed();

        let mut notes = Vec::new();
        if let Err(error) = &scan_result {
            notes.push(format!("扫描报错：{}", error.message));
        }
        if let Err(error) = &watch_result {
            notes.push(format!("监听处理报错：{}", error.message));
        }
        let active = fixture.active_count();
        if active < baseline + FILES_PER_ROUND as i64 {
            notes.push(format!(
                "采纳数量不足：active={active}，期望 >= {}",
                baseline + FILES_PER_ROUND as i64
            ));
        }
        // 逐篇核对：不应出现重复路径导致的唯一约束残留；两侧都不得报错
        let scanned = scan_result.as_ref().map(|report| report.added).unwrap_or(0);
        let watched = watch_result.as_ref().map(|report| report.added).unwrap_or(0);
        println!(
            "第 {round} 轮：scan added={scanned} / watch added={watched} / active={active} / 耗时 {:?} → {}",
            elapsed,
            if notes.is_empty() { "OK".to_string() } else { notes.join("；") }
        );
        failures.extend(notes.into_iter().map(|note| format!("第 {round} 轮：{note}")));
    }

    assert!(
        failures.is_empty(),
        "并发扫描与监听出现冲突（这正是阶段 B 要修的问题）：\n{}",
        failures.join("\n")
    );
}

/// 同一目录被两侧同时扫（显式扫描 + 轮询监听上报「目录变化」→ `scan_directory`）。
///
/// 这条路径比 `apply_file_changes` 更贴近历史缺陷的形态：两侧都要
/// 「列文件 → 查库 → 采纳」，且都走 `adopt_files_batched`。
#[test]
fn concurrent_directory_scans_of_the_same_directory() {
    let mut failures: Vec<String> = Vec::new();
    for round in 0..ROUNDS {
        let fixture = Fixture::new("目录并发");
        let context = fixture.app.context().expect("工作区已打开");
        let _ = fixture.write_external_files(round);
        let baseline = fixture.active_count();

        let start_line = Arc::new(Barrier::new(3));
        let first = {
            let line = Arc::clone(&start_line);
            let ctx = Arc::clone(&context);
            thread::spawn(move || {
                line.wait();
                IndexService::new(ctx).scan_directory("00_Inbox")
            })
        };
        let second = {
            let line = Arc::clone(&start_line);
            let ctx = Arc::clone(&context);
            thread::spawn(move || {
                line.wait();
                IndexService::new(ctx).scan_directory("00_Inbox")
            })
        };
        start_line.wait();
        let first_result = first.join().expect("线程未 panic");
        let second_result = second.join().expect("线程未 panic");

        let mut notes = Vec::new();
        for (label, result) in [("扫描 A", &first_result), ("扫描 B", &second_result)] {
            if let Err(error) = result {
                notes.push(format!("{label} 报错：{}", error.message));
            }
        }
        let active = fixture.active_count();
        if active < baseline + FILES_PER_ROUND as i64 {
            notes.push(format!(
                "采纳数量不足：active={active}，期望 >= {}",
                baseline + FILES_PER_ROUND as i64
            ));
        }
        let added_a = first_result.as_ref().map(|report| report.added).unwrap_or(0);
        let added_b = second_result.as_ref().map(|report| report.added).unwrap_or(0);
        println!(
            "目录并发第 {round} 轮：A added={added_a} / B added={added_b} / active={active} → {}",
            if notes.is_empty() { "OK".to_string() } else { notes.join("；") }
        );
        failures.extend(notes.into_iter().map(|note| format!("第 {round} 轮：{note}")));
    }
    assert!(
        failures.is_empty(),
        "同目录并发扫描出现冲突：\n{}",
        failures.join("\n")
    );
}

/// 用户「新建笔记」与「扫描」撞在同一文件上。
///
/// `NoteService::create` 的顺序是：写文件 → 按自己的 id 插入行；
/// 而扫描/监听是：读磁盘 → 采纳（会复用 frontmatter 里的 id）。
/// 若扫描趁 create 落库前先采纳了这篇文件，create 的插入就会撞主键
/// （`UNIQUE constraint failed: notes.id`）——用户看到「创建失败」，且库里已有一行。
#[test]
fn creating_notes_while_scanning_does_not_fail() {
    let fixture = Fixture::new("创建并发");
    let mut failures: Vec<String> = Vec::new();

    for round in 0..2 {
        let context = fixture.app.context().expect("工作区已打开");
        // 扫描线程：反复扫 00_Inbox（新建笔记默认落在这里）
        let scanning = {
            let ctx = Arc::clone(&context);
            let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = Arc::clone(&done);
            let handle = thread::spawn(move || {
                let service = IndexService::new(ctx);
                let mut scans = 0usize;
                while !flag.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = service.scan_directory("00_Inbox");
                    scans += 1;
                }
                scans
            });
            (handle, done)
        };

        // 主线程：连续新建 60 篇，模拟「边导入边新建」
        let notes = NoteService::new(Arc::clone(&context));
        let mut created_ids = Vec::new();
        for index in 0..60 {
            match notes.create(CreateNoteInput {
                title: format!("并发新建 {round}-{index}"),
                category_id: None,
                initial_content: Some(format!("正文 {index}")),
                tags: None,
                parent_id: None,
            }) {
                Ok(detail) => created_ids.push(detail.note.id),
                Err(error) => failures.push(format!("第 {round} 轮第 {index} 篇创建失败：{}", error.message)),
            }
        }

        let (handle, done) = scanning;
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        let scans = handle.join().expect("扫描线程未 panic");
        println!(
            "创建并发第 {round} 轮：新建 {} 篇 / 扫描 {scans} 轮 / 失败 {} 次",
            created_ids.len(),
            failures.len()
        );

        // 每篇都应恰好一行（按 id 只出现一次）
        let duplicate = context
            .db
            .with_conn(|conn| {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM (SELECT id FROM notes GROUP BY id HAVING COUNT(*) > 1)",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count)
            })
            .expect("重复 id 统计");
        if duplicate > 0 {
            failures.push(format!("第 {round} 轮出现 {duplicate} 个重复 id"));
        }
    }

    assert!(
        failures.is_empty(),
        "新建与扫描并发时出错：\n{}",
        failures.join("\n")
    );
}
