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
use std::collections::BTreeMap;

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

// ---------------------------------------------------------------- 参数名契约

/// 从 `commands/*.rs` 里取每个命令声明的参数名（跳过 `state`）。
fn declared_parameters() -> BTreeMap<String, BTreeSet<String>> {
    let dir = repo_root().join("src-tauri/src/commands");
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in fs::read_dir(&dir).expect("读 commands 目录") {
        let path = entry.expect("目录项").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("读命令文件");
        for block in split_command_signatures(&text) {
            let Some((name, params)) = block.split_once('(') else {
                continue;
            };
            let name = name.trim();
            let Some(params) = params.split(')').next() else {
                continue;
            };
            let mut names = BTreeSet::new();
            for part in params.split(',') {
                let part = part.trim();
                if part.is_empty() || part.starts_with("state:") {
                    continue;
                }
                let Some((key, _)) = part.split_once(':') else {
                    continue;
                };
                let key = key.trim();
                if !key.is_empty()
                    && key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                {
                    names.insert(to_snake(key));
                }
            }
            map.insert(name.to_string(), names);
        }
    }
    map
}

/// 取出所有 `pub async fn name(...)` 之后到 `->` 之前的片段（含函数名与参数表）。
fn split_command_signatures(source: &str) -> Vec<String> {
    const MARKER: &str = "pub async fn ";
    let mut result = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find(MARKER) {
        let tail = &rest[index + MARKER.len()..];
        let end = tail.find("->").map(|value| value + 2).unwrap_or(tail.len());
        result.push(tail[..end].to_string());
        rest = &tail[end.min(tail.len())..];
    }
    result
}

/// camelCase → snake_case（Tauri 两种写法都认，比较前先归一）。
fn to_snake(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// 从对象字面量里取**顶层**键（跳过嵌套对象 / 数组，支持简写属性）。
fn top_level_keys(object: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut depth = 0usize;
    let mut segment = String::new();
    for ch in object.chars() {
        match ch {
            '{' | '[' | '(' => {
                depth += 1;
                if depth == 1 {
                    segment.clear();
                }
            }
            '}' | ']' | ')' => {
                if depth == 1 {
                    record_key(&segment, &mut keys);
                    segment.clear();
                }
                depth = depth.saturating_sub(1);
            }
            ':' if depth == 1 => {
                record_key(&segment, &mut keys);
                segment.clear();
            }
            ',' if depth == 1 => {
                record_key(&segment, &mut keys);
                segment.clear();
            }
            _ => {
                if depth == 1 {
                    segment.push(ch);
                }
            }
        }
    }
    keys
}

fn record_key(segment: &str, keys: &mut BTreeSet<String>) {
    let token = segment.trim();
    let looks_like_identifier = token
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    // 值片段（例如 `null` / `'abc'` / `targetDir ?? null`）不会是合法标识符，会被这里过滤掉
    if looks_like_identifier {
        keys.insert(token.to_string());
    }
}

/// 前端 `call('name', { ... })` 的 (命令名, 顶层键)。
fn frontend_payloads() -> Vec<(String, String, BTreeSet<String>)> {
    let dir = repo_root().join("src/services");
    let mut result = Vec::new();
    for entry in fs::read_dir(&dir).expect("读 src/services") {
        let path = entry.expect("目录项").path();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        if !file_name.ends_with(".ts") || file_name.ends_with(".test.ts") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("读服务文件");
        let bytes: Vec<char> = text.chars().collect();
        let mut index = 0usize;
        while index < bytes.len() {
            // 找 `call`
            if !matches_at(&bytes, index, "call") {
                index += 1;
                continue;
            }
            let mut cursor = index + 4;
            // 可选泛型 `<...>`
            if bytes.get(cursor) == Some(&'<') {
                let mut depth = 0;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        '<' => depth += 1,
                        '>' => {
                            depth -= 1;
                            if depth == 0 {
                                cursor += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    cursor += 1;
                }
            }
            if bytes.get(cursor) != Some(&'(') {
                index += 1;
                continue;
            }
            cursor += 1;
            while bytes.get(cursor).is_some_and(|ch| ch.is_whitespace() || *ch == ',') {
                cursor += 1;
            }
            if bytes.get(cursor) != Some(&'\'') {
                index += 1;
                continue;
            }
            cursor += 1;
            let name_start = cursor;
            while bytes.get(cursor).is_some_and(|ch| *ch != '\'') {
                cursor += 1;
            }
            let name: String = bytes[name_start..cursor].iter().collect();
            cursor += 1; // 跳过结尾引号
            while bytes.get(cursor).is_some_and(|ch| ch.is_whitespace() || *ch == ',') {
                cursor += 1;
            }
            if bytes.get(cursor) != Some(&'{') {
                index = cursor.max(index + 1);
                continue;
            }
            let mut depth = 0usize;
            let object_start = cursor;
            while cursor < bytes.len() {
                match bytes[cursor] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            cursor += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                cursor += 1;
            }
            let object: String = bytes[object_start..cursor.min(bytes.len())].iter().collect();
            result.push((file_name.clone(), name, top_level_keys(&object)));
            index = cursor.max(index + 1);
        }
    }
    result
}

fn matches_at(chars: &[char], index: usize, needle: &str) -> bool {
    let needle: Vec<char> = needle.chars().collect();
    index + needle.len() <= chars.len() && chars[index..index + needle.len()] == needle[..]
}

#[test]
fn frontend_command_payloads_only_use_declared_parameters() {
    let declared = declared_parameters();
    let payloads = frontend_payloads();

    assert!(
        declared.len() >= 100,
        "命令签名解析异常：只找到 {} 个",
        declared.len()
    );
    assert!(
        payloads.len() >= 70,
        "前端调用解析异常：只找到 {} 处带参数的调用",
        payloads.len()
    );

    let mut problems: Vec<String> = Vec::new();
    for (file, command, keys) in &payloads {
        let Some(params) = declared.get(command) else {
            problems.push(format!("{file}: 调用了未声明的命令 {command}"));
            continue;
        };
        for key in keys {
            let normalized = to_snake(key);
            if !params.contains(&normalized) {
                problems.push(format!(
                    "{file}: `{command}` 发送了未声明的参数 `{key}`（Rust 参数：{params:?}）"
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "前端载荷与命令参数不一致（浏览器预览会忽略多余键，只有桌面端会报 invalid args）：\n{}",
        problems.join("\n")
    );
}
