//! 命令契约守卫（0 信任 / 防静默失效）。
//!
//! 前端通过字符串调用 Tauri 命令（`call('note_create', …)`）。这类调用**只有跑起来才知道**对不对：
//! - 编译期管不到字符串；
//! - E2E 走的是浏览器预览的内存后端（mock），名字写错也会被 mock 兜住；
//! - 于是「命令名不一致 / 漏注册 / mock 漏实现」就会变成桌面端才暴露的静默失败。
//!
//! 这个测试把三份清单对齐：`commands/*.rs` 的注册表 ↔ 前端 `services/*.ts` 的调用 ↔
//! mock 的 `case` 分支。任何一处漂移都直接失败，并打印出具体名字。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("仓库根目录")
        .to_path_buf()
}

/// 从 `lib.rs` 的 `invoke_handler` 里取已注册的命令名。
fn registered_commands() -> BTreeSet<String> {
    let source = fs::read_to_string(repo_root().join("src-tauri/src/lib.rs")).expect("读 lib.rs");
    let start = source
        .find("invoke_handler")
        .expect("lib.rs 里应当有 invoke_handler");
    let tail = &source[start..];
    let end = tail.find("])").map(|index| index + 2).unwrap_or(tail.len());
    let block = &tail[..end];
    let mut names = BTreeSet::new();
    for line in block.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if let Some(rest) = trimmed.strip_prefix("commands::") {
            if let Some((_module, function)) = rest.split_once("::") {
                let name = function.trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

/// 从前端服务层里取所有被调用的命令名。
fn frontend_commands() -> BTreeSet<String> {
    let dir = repo_root().join("src/services");
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(&dir).expect("读 src/services") {
        let path = entry.expect("目录项").path();
        let is_service = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value == "ts")
            && !path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.ends_with(".test.ts"));
        if !is_service {
            continue;
        }
        let text = fs::read_to_string(&path).expect("读服务文件");
        for (index, _) in text.match_indices("call<") {
            if let Some(name) = extract_name(&text[index..]) {
                names.insert(name);
            }
        }
        for (index, _) in text.match_indices("call(") {
            if let Some(name) = extract_name(&text[index..]) {
                names.insert(name);
            }
        }
    }
    names
}

/// 从 `call...(` 的余下文本里取第一个字符串字面量（即命令名）。
fn extract_name(from_index: &str) -> Option<String> {
    let call_at = from_index.find('(')?;
    let rest = &from_index[call_at + 1..];
    let trimmed = rest.trim_start();
    // 泛型参数（`call<Type>(`）在调用括号之后就不会再出现，直接找第一个引号
    let quote = trimmed.find('\'')?;
    let after = &trimmed[quote + 1..];
    let end = after.find('\'')?;
    let name = &after[..end];
    if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_' || ch.is_ascii_digit()) {
        return None;
    }
    Some(name.to_string())
}

/// 从 mock（浏览器预览后端）里取所有 `case '<name>':` 分支。
fn mock_commands() -> BTreeSet<String> {
    let text = fs::read_to_string(repo_root().join("src/services/mock.ts")).expect("读 mock.ts");
    let mut names = BTreeSet::new();
    for (index, _) in text.match_indices("case '") {
        let rest = &text[index + 6..];
        if let Some(end) = rest.find('\'') {
            names.insert(rest[..end].to_string());
        }
    }
    names
}

#[test]
fn every_frontend_command_is_registered_and_mocked() {
    let registered = registered_commands();
    let frontend = frontend_commands();
    let mocked = mock_commands();

    // 解析本身要有效：数量级不对说明正则/结构变了，测试会变成「永远通过」
    assert!(
        registered.len() >= 100,
        "注册表解析异常（只找到 {} 个命令），请检查 lib.rs 的结构",
        registered.len()
    );
    assert!(
        frontend.len() >= 90,
        "前端调用解析异常（只找到 {} 个命令），请检查 services 的写法",
        frontend.len()
    );
    assert!(
        mocked.len() >= 90,
        "mock 分支解析异常（只找到 {} 个命令）",
        mocked.len()
    );

    let missing_backend: Vec<&String> = frontend.difference(&registered).collect();
    assert!(
        missing_backend.is_empty(),
        "前端调用了未在 lib.rs 注册的命令（桌面端会直接失败）：{missing_backend:?}"
    );

    let missing_mock: Vec<&String> = frontend.difference(&mocked).collect();
    assert!(
        missing_mock.is_empty(),
        "前端调用了 mock 未实现的命令（浏览器预览会失败）：{missing_mock:?}"
    );
}

#[test]
fn registered_commands_live_in_the_commands_module() {
    // 注册表里出现的每个函数名都应当能在 commands/ 下找到定义，避免「注册了不存在的函数」这类
    // 只在特定 feature/条件编译下暴露的问题（编译期通常能拦住，这里再兜一层）。
    let registered = registered_commands();
    let mut defined: BTreeSet<String> = BTreeSet::new();
    let dir = repo_root().join("src-tauri/src/commands");
    for entry in fs::read_dir(&dir).expect("读 commands 目录") {
        let path = entry.expect("目录项").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("读命令文件");
        for (index, _) in text.match_indices("pub async fn ") {
            let rest = &text[index + "pub async fn ".len()..];
            let name: String = rest
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .collect();
            if !name.is_empty() {
                defined.insert(name);
            }
        }
    }
    assert!(defined.len() >= 100, "命令定义解析异常：{}", defined.len());
    let unknown: Vec<&String> = registered.difference(&defined).collect();
    assert!(
        unknown.is_empty(),
        "lib.rs 注册了 commands/ 里不存在的函数：{unknown:?}"
    );
}
