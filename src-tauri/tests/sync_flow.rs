use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::writing::WritingService;
use personal_knowledge_os_lib::application::attachment_service::{
    AttachmentService, SaveAttachmentInput,
};
use personal_knowledge_os_lib::application::graph_service::GraphService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::sync::{SyncService, SyncTransport, WebDavTransport};
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::error::ErrorCode;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::graph::GraphQuery;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery, SaveNoteInput};
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::domain::sync::{
    ResolveSyncConflictInput, RestoreSyncHistoryInput, SyncRunInput, UpdateSyncSettingsInput,
};
use personal_knowledge_os_lib::domain::writing::{WritingMode, WritingRequest};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

const PASSPHRASE: &str = "correct horse battery staple";

#[derive(Clone)]
struct StoredObject {
    bytes: Vec<u8>,
    etag: String,
}

#[derive(Default)]
struct DavState {
    objects: HashMap<String, StoredObject>,
    sequence: u64,
    include_etags: bool,
}

struct LocalDav {
    endpoint: String,
    state: Arc<Mutex<DavState>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl LocalDav {
    fn start(auth: &str) -> Self {
        Self::start_with_etags(auth, true)
    }

    fn start_with_etags(auth: &str, include_etags: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("启动本地同步服务");
        listener.set_nonblocking(true).expect("设置非阻塞 listener");
        let address = listener.local_addr().expect("本地地址");
        let state = Arc::new(Mutex::new(DavState {
            include_etags,
            ..Default::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let state_for_thread = Arc::clone(&state);
        let stop_for_thread = Arc::clone(&stop);
        let expected_auth = auth.to_string();
        let thread = thread::spawn(move || {
            while !stop_for_thread.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(error) =
                            handle_connection(stream, &state_for_thread, &expected_auth)
                        {
                            eprintln!("local sync stub connection error: {error}");
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            endpoint: format!("http://{address}/dav"),
            state,
            stop,
            thread: Some(thread),
        }
    }

    fn contains_plaintext(&self, needle: &str) -> bool {
        self.state
            .lock()
            .expect("服务端状态")
            .objects
            .values()
            .any(|object| {
                String::from_utf8_lossy(&object.bytes).contains(needle)
                    || object
                        .bytes
                        .windows(needle.len())
                        .any(|window| window == needle.as_bytes())
            })
    }
}

impl Drop for LocalDav {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    state: &Arc<Mutex<DavState>>,
    expected_auth: &str,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut buffer = Vec::new();
    let mut block = [0u8; 4096];
    let header_end;
    loop {
        let read = stream.read(&mut block)?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&block[..read]);
        if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
    }
    let header = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = header.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default();
    let path = request_parts.next().unwrap_or_default().to_string();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let chunked = headers
        .get("transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"));
    if headers
        .get("expect")
        .is_some_and(|value| value.eq_ignore_ascii_case("100-continue"))
    {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
        stream.flush()?;
    }
    if chunked {
        while !find_bytes(&buffer[header_end..], b"0\r\n\r\n").is_some() {
            let read = stream.read(&mut block)?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&block[..read]);
        }
    } else {
        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut block)?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&block[..read]);
        }
    }
    let body = if chunked {
        decode_chunked(&buffer[header_end..])
    } else {
        buffer[header_end..header_end + content_length].to_vec()
    };

    if method != "MKCOL" {
        let auth = headers
            .get("authorization")
            .map(String::as_str)
            .unwrap_or("");
        let authorized = if expected_auth == "s3" {
            auth.starts_with("AWS4-HMAC-SHA256 Credential=")
        } else {
            auth == expected_auth
        };
        if !authorized {
            return respond(&mut stream, 401, &[], b"unauthorized", None);
        }
    }
    if method == "MKCOL" {
        return respond(&mut stream, 201, &[], b"", None);
    }
    let mut guard = state.lock().expect("服务端状态");
    match method {
        "GET" | "HEAD" => match guard.objects.get(&path).cloned() {
            Some(object) => {
                let headers: Vec<(&str, &str)> = if guard.include_etags {
                    vec![("ETag", object.etag.as_str())]
                } else {
                    Vec::new()
                };
                respond(
                    &mut stream,
                    200,
                    &headers,
                    if method == "HEAD" { b"" } else { &object.bytes },
                    None,
                )
            }
            None => respond(&mut stream, 404, &[], b"", None),
        },
        "PUT" => {
            if headers
                .get("if-none-match")
                .is_some_and(|value| value == "*")
                && guard.objects.contains_key(&path)
            {
                return respond(&mut stream, 412, &[], b"", None);
            }
            if let Some(required) = headers.get("if-match") {
                if guard.objects.get(&path).map(|item| item.etag.as_str()) != Some(required) {
                    return respond(&mut stream, 412, &[], b"", None);
                }
            }
            guard.sequence += 1;
            let etag = format!("\"v{}\"", guard.sequence);
            guard.objects.insert(
                path,
                StoredObject {
                    bytes: body,
                    etag: etag.clone(),
                },
            );
            let headers: Vec<(&str, &str)> = if guard.include_etags {
                vec![("ETag", etag.as_str())]
            } else {
                Vec::new()
            };
            respond(&mut stream, 201, &headers, b"", None)
        }
        "DELETE" => {
            guard.objects.remove(&path);
            respond(&mut stream, 204, &[], b"", None)
        }
        _ => respond(&mut stream, 405, &[], b"", None),
    }
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
    _unused: Option<()>,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        412 => "Precondition Failed",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn decode_chunked(mut bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    while let Some(line_end) = find_bytes(bytes, b"\r\n") {
        let size_text = String::from_utf8_lossy(&bytes[..line_end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("0").trim(), 16)
            .unwrap_or(0);
        bytes = &bytes[line_end + 2..];
        if size == 0 {
            break;
        }
        if bytes.len() < size + 2 {
            break;
        }
        output.extend_from_slice(&bytes[..size]);
        bytes = &bytes[size + 2..];
    }
    output
}

struct Device {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
}

impl Device {
    fn new(name: &str) -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create(name, &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self {
            _data: data,
            _workspace: workspace,
            app,
        }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn sync(&self) -> SyncService {
        SyncService::new(self.app.context().expect("工作区已打开"))
    }

    fn clear_initial_notes(&self) {
        let notes = self.notes();
        let items = notes
            .list(NoteListQuery {
                page_size: Some(100),
                ..Default::default()
            })
            .expect("列出初始笔记")
            .items;
        for item in items {
            notes.delete(&item.id).expect("移出初始笔记");
        }
    }

    fn note(&self, title: &str, body: &str) -> String {
        let detail = self
            .notes()
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(body.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记");
        detail.note.id
    }

    fn configure_webdav(&self, endpoint: &str) {
        self.sync()
            .update_settings(UpdateSyncSettingsInput {
                enabled: Some(true),
                provider: Some("webdav".to_string()),
                endpoint: Some(endpoint.to_string()),
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                sync_notes: Some(true),
                sync_attachments: Some(true),
                sync_index: Some(false),
                ..Default::default()
            })
            .expect("保存 WebDAV 配置");
    }

    fn configure_webdav_with_index(&self, endpoint: &str) {
        self.sync()
            .update_settings(UpdateSyncSettingsInput {
                enabled: Some(true),
                provider: Some("webdav".to_string()),
                endpoint: Some(endpoint.to_string()),
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                sync_notes: Some(true),
                sync_attachments: Some(true),
                sync_index: Some(true),
                ..Default::default()
            })
            .expect("保存含索引的 WebDAV 配置");
    }

    fn workspace_root(&self) -> &std::path::Path {
        self._workspace.path()
    }

    fn data_root(&self) -> &std::path::Path {
        self._data.path()
    }

    fn run_sync(&self) -> personal_knowledge_os_lib::domain::sync::SyncRunReport {
        self.sync().preview(PASSPHRASE).expect("同步 dry-run");
        self.sync()
            .run(SyncRunInput {
                passphrase: PASSPHRASE.to_string(),
                confirm: true,
            })
            .expect("执行同步")
    }

    fn find_note(&self, title: &str) -> String {
        self.notes()
            .list(NoteListQuery {
                page_size: Some(100),
                ..Default::default()
            })
            .expect("列出笔记")
            .items
            .into_iter()
            .find(|item| item.title == title)
            .unwrap_or_else(|| panic!("找不到笔记：{title}"))
            .id
    }

    fn save_note(&self, id: &str, body: &str) {
        self.notes()
            .save(SaveNoteInput {
                id: id.to_string(),
                content: body.to_string(),
                base_content_hash: None,
                title: None,
                category_id: None,
                tags: None,
            })
            .expect("保存笔记");
    }
}

#[test]
fn webdav_two_device_add_modify_delete_conflict_and_merge() {
    let remote = LocalDav::start("Basic dXNlcjpwYXNz");
    let device_a = Device::new("设备 A");
    let device_b = Device::new("设备 B");
    device_b.clear_initial_notes();
    device_a.configure_webdav(&remote.endpoint);
    device_b.configure_webdav(&remote.endpoint);

    device_a.note(
        "同步机密笔记",
        "# 同步机密笔记\n\n这段正文必须只以密文形式出现在远端。",
    );
    device_a.run_sync();
    device_b.run_sync();

    let b_note = device_b.find_note("同步机密笔记");
    assert!(device_b
        .notes()
        .get(&b_note)
        .expect("设备 B 读取")
        .content
        .contains("密文形式"));
    assert!(
        !remote.contains_plaintext("同步机密笔记") && !remote.contains_plaintext("密文形式"),
        "远端对象与 manifest 都不得出现正文或标题明文"
    );

    device_b.save_note(&b_note, "# 同步机密笔记\n\n设备 B 修改了正文。");
    device_b.run_sync();
    device_a.run_sync();
    assert!(device_a
        .notes()
        .get(&device_a.find_note("同步机密笔记"))
        .expect("设备 A 读取更新")
        .content
        .contains("设备 B"));

    device_a
        .notes()
        .delete(&device_a.find_note("同步机密笔记"))
        .expect("设备 A 删除");
    let a_delete_report = device_a.run_sync();
    assert_eq!(a_delete_report.remote_deleted, 1, "设备 A 应上传删除墓碑");
    let delete_report = device_b.run_sync();
    assert_eq!(delete_report.local_deleted, 1, "设备 B 应下载删除墓碑");
    assert!(
        device_b
            .notes()
            .list(NoteListQuery {
                page_size: Some(100),
                ..Default::default()
            })
            .expect("设备 B 列表")
            .items
            .iter()
            .all(|item| item.title != "同步机密笔记"),
        "远端墓碑应在另一设备传播删除"
    );

    device_a.note(
        "三方冲突",
        "# 三方冲突\n\n第一行旧值\n\n第二行旧值\n\n第三行旧值\n",
    );
    device_a.run_sync();
    device_b.run_sync();
    let a_conflict = device_a.find_note("三方冲突");
    let b_conflict = device_b.find_note("三方冲突");
    device_a.save_note(
        &a_conflict,
        "# 三方冲突\n\n第一行 A 修改\n\n第二行旧值\n\n第三行旧值\n",
    );
    device_b.save_note(
        &b_conflict,
        "# 三方冲突\n\n第一行旧值\n\n第二行旧值\n\n第三行 B 修改\n",
    );
    device_b.run_sync();
    let report = device_a
        .sync()
        .run(SyncRunInput {
            passphrase: PASSPHRASE.to_string(),
            confirm: true,
        })
        .expect("设备 A 检测冲突");
    assert_eq!(report.conflicts_created, 1);
    assert!(
        device_a
            .notes()
            .get(&a_conflict)
            .expect("冲突本地版本")
            .content
            .contains("第一行 A 修改"),
        "冲突不得自动覆盖本地版本"
    );
    let conflicts = device_a.sync().conflicts().expect("冲突列表");
    assert_eq!(conflicts.len(), 1);
    let detail = device_a
        .sync()
        .conflict_detail(&conflicts[0].id)
        .expect("冲突详情");
    assert!(detail
        .remote_content
        .as_deref()
        .is_some_and(|text| text.contains("第三行 B 修改")));
    let merged = device_a
        .sync()
        .merge_preview(&conflicts[0].id)
        .expect("三方合并预览");
    let merged_content = merged.merged_content.expect("不重叠修改应自动合并");
    assert!(merged_content.contains("第一行 A 修改"));
    assert!(merged_content.contains("第三行 B 修改"));
    device_a
        .sync()
        .resolve_conflict(ResolveSyncConflictInput {
            conflict_id: conflicts[0].id.clone(),
            resolution: "merged".to_string(),
            merged_content: Some(merged_content),
        })
        .expect("保存合并结果");
    device_a.run_sync();
    device_b.run_sync();
    let final_content = device_b
        .notes()
        .get(&device_b.find_note("三方冲突"))
        .expect("设备 B 读取合并结果")
        .content;
    assert!(final_content.contains("第一行 A 修改"));
    assert!(final_content.contains("第三行 B 修改"));

    device_b
        .sync()
        .update_settings(UpdateSyncSettingsInput {
            enabled: Some(false),
            ..Default::default()
        })
        .expect("关闭同步");
    assert!(device_b
        .notes()
        .get(&device_b.find_note("三方冲突"))
        .expect("关闭同步后仍可本地读取")
        .content
        .contains("第二行旧值"));

    drop(remote);
    let context = device_b.app.context().expect("关闭同步后的本地上下文");
    let search = SearchService::new(Arc::clone(&context))
        .search(SearchQuery {
            keyword: "三方冲突".to_string(),
            ..Default::default()
        })
        .expect("关闭同步后检索仍可用");
    assert!(
        search.hits.iter().any(|hit| hit.note.title == "三方冲突"),
        "关闭同步并断开远端后，检索不应受影响"
    );
    let graph = GraphService::new(Arc::clone(&context))
        .graph(GraphQuery::default())
        .expect("关闭同步后图谱仍可用");
    assert!(
        graph.nodes.iter().any(|node| node.title == "三方冲突"),
        "关闭同步后图谱应继续读取本地图"
    );
    AiPolicyService::new(Arc::clone(&context))
        .update(UpdateAiPolicyInput {
            enabled: Some(true),
            ..Default::default()
        })
        .expect("开启本地 AI 降级");
    let writing = WritingService::new(Arc::clone(&context))
        .draft(WritingRequest {
            note_id: device_b.find_note("三方冲突"),
            mode: Some(WritingMode::Shorten),
            instruction: "保持本地可用".to_string(),
            selection: Some("第二行旧值".to_string()),
            selection_start: Some(0),
            selection_end: Some(8),
            confirm_outbound: false,
        })
        .expect("关闭同步后写作模板仍可用");
    assert!(!writing.segments.is_empty());
}

#[test]
fn s3_compatible_transport_runs_against_local_signature_aware_stub() {
    let remote = LocalDav::start("s3");
    let device = Device::new("S3 设备");
    device.clear_initial_notes();
    device
        .sync()
        .update_settings(UpdateSyncSettingsInput {
            enabled: Some(true),
            provider: Some("s3".to_string()),
            endpoint: Some(remote.endpoint.trim_end_matches("/dav").to_string()),
            bucket: Some("pkos".to_string()),
            region: Some("us-east-1".to_string()),
            access_key: Some("AKID".to_string()),
            secret_key: Some("SECRET".to_string()),
            sync_notes: Some(true),
            sync_attachments: Some(true),
            sync_index: Some(false),
            ..Default::default()
        })
        .expect("保存 S3 配置");
    device.note("S3 同步", "# S3 同步\n\n通过 S3 兼容签名上传。");
    let preview = device.sync().preview(PASSPHRASE).expect("S3 dry-run");
    assert_eq!(preview.uploads.len(), 1);
    let report = device
        .sync()
        .run(SyncRunInput {
            passphrase: PASSPHRASE.to_string(),
            confirm: true,
        })
        .expect("S3 同步");
    assert_eq!(report.uploaded, 1);
    assert!(!remote.contains_plaintext("通过 S3 兼容签名上传"));
}

#[test]
fn wrong_passphrase_and_tampered_ciphertext_are_rejected() {
    let remote = LocalDav::start("Basic dXNlcjpwYXNz");
    let device = Device::new("篡改测试");
    device.clear_initial_notes();
    device.configure_webdav(&remote.endpoint);
    device.note("待保护", "# 待保护\n\n不能被篡改。");
    device.run_sync();

    let wrong = device
        .sync()
        .preview("this passphrase is definitely different")
        .expect_err("错误口令必须失败");
    assert_eq!(wrong.code, ErrorCode::InvalidInput);

    let manifest_key = remote
        .state
        .lock()
        .expect("服务端状态")
        .objects
        .keys()
        .find(|key| key.ends_with("/manifest.enc"))
        .cloned()
        .expect("远端 manifest");
    {
        let mut guard = remote.state.lock().expect("服务端状态");
        guard
            .objects
            .get_mut(&manifest_key)
            .expect("manifest 存在")
            .bytes[12] ^= 1;
    }
    let tampered = device
        .sync()
        .preview(PASSPHRASE)
        .expect_err("篡改 manifest 必须失败");
    assert_eq!(tampered.code, ErrorCode::InvalidInput);
}

#[test]
fn missing_manifest_etag_refuses_to_overwrite_remote_state() {
    let remote = LocalDav::start_with_etags("Basic dXNlcjpwYXNz", false);
    let device = Device::new("ETag 缺失");
    device.clear_initial_notes();
    device.configure_webdav(&remote.endpoint);
    let note = device.note("无 ETag", "# 无 ETag\n\n初始正文。");
    device.run_sync();
    device.save_note(&note, "# 无 ETag\n\n本地第二版。");

    let error = device
        .sync()
        .run(SyncRunInput {
            passphrase: PASSPHRASE.to_string(),
            confirm: true,
        })
        .expect_err("已有 manifest 但服务端无 ETag 时必须拒绝覆盖");
    assert_eq!(error.code, ErrorCode::SyncFailed);
    assert!(
        error.message.contains("ETag"),
        "错误需要明确说明并发保护缺什么：{}",
        error.message
    );
    assert!(device
        .notes()
        .get(&note)
        .expect("本地版本")
        .content
        .contains("本地第二版"));
}

#[test]
fn conditional_create_prevents_two_first_syncs_from_overwriting_manifest() {
    let remote = LocalDav::start("Basic dXNlcjpwYXNz");
    let transport = WebDavTransport::new(&remote.endpoint, "user", "pass");
    let key = "pkos-sync/v2/library/test-create.json";
    transport
        .put(key, b"first", None, true)
        .expect("首次条件创建");
    let conflict = transport
        .put(key, b"second", None, true)
        .expect_err("对象已存在时 If-None-Match 必须失败");
    assert_eq!(conflict.code, ErrorCode::SyncConflict);
    let object = transport.get(key).expect("读取远端").expect("对象存在");
    assert_eq!(object.bytes, b"first", "第二个创建者不能覆盖第一个");
}

#[test]
fn non_loopback_http_endpoint_is_rejected_before_credentials_cross_the_network() {
    let device = Device::new("明文 endpoint");
    for (endpoint, expected) in [
        ("http://dav.example.test/pkos", "HTTPS"),
        ("http://127.evil.com/pkos", "HTTPS"),
        ("http://127.0.0.1.evil.com/pkos", "HTTPS"),
        ("https://user:secret@dav.example.test/pkos", "userinfo"),
        ("https://dav.example.test/pkos?token=secret", "查询串"),
        ("https:///missing-host", "主机名"),
        ("https://dav.example.test/has space", "空白"),
    ] {
        let error = device
            .sync()
            .update_settings(UpdateSyncSettingsInput {
                enabled: Some(true),
                provider: Some("webdav".to_string()),
                endpoint: Some(endpoint.to_string()),
                username: Some("user".to_string()),
                password: Some("secret".to_string()),
                ..Default::default()
            })
            .expect_err("不安全 endpoint 必须拒绝");
        assert_eq!(error.code, ErrorCode::InvalidInput, "{endpoint}");
        assert!(
            error.message.contains(expected),
            "{endpoint}: {}",
            error.message
        );
    }
    assert!(
        !device.data_root().join("sync").exists(),
        "被拒绝的配置不应落盘"
    );
    let local = device
        .sync()
        .update_settings(UpdateSyncSettingsInput {
            enabled: Some(true),
            provider: Some("webdav".to_string()),
            endpoint: Some("http://127.0.0.1:9/pkos".to_string()),
            username: Some("user".to_string()),
            password: Some("secret".to_string()),
            ..Default::default()
        })
        .expect("回环 HTTP 允许用于本机验收");
    assert_eq!(local.endpoint.as_deref(), Some("http://127.0.0.1:9/pkos"));

    let config_path = std::fs::read_dir(device.data_root().join("sync"))
        .expect("同步目录")
        .flatten()
        .map(|entry| entry.path().join("config.json"))
        .find(|path| path.is_file())
        .expect("同步配置");
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&config_path).expect("读取配置")).expect("解析配置");
    config["endpoint"] = serde_json::Value::String("http://old.example.test/pkos".to_string());
    std::fs::write(
        &config_path,
        serde_json::to_vec_pretty(&config).expect("序列化配置"),
    )
    .expect("模拟旧版不安全配置");
    let runtime_error = device
        .sync()
        .preview(PASSPHRASE)
        .expect_err("运行期也必须复核旧配置");
    assert_eq!(runtime_error.code, ErrorCode::InvalidInput);
    assert!(runtime_error.message.contains("HTTPS"));
}

#[test]
fn offline_changes_are_queued_and_local_editing_stays_available() {
    let remote = LocalDav::start("Basic dXNlcjpwYXNz");
    let device = Device::new("离线队列");
    device.clear_initial_notes();
    device.configure_webdav(&remote.endpoint);
    let credential_file = std::fs::read_dir(device.data_root().join("sync"))
        .expect("同步状态目录")
        .flatten()
        .map(|entry| entry.path().join("credentials.json"))
        .find(|path| path.is_file())
        .expect("应用数据目录应保存同步凭据");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&credential_file)
                .expect("凭据文件元数据")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "同步凭据必须仅当前用户可读写"
        );
    }
    let note = device.note("离线编辑", "# 离线编辑\n\n初始正文。");
    device.run_sync();

    drop(remote);
    device.save_note(&note, "# 离线编辑\n\n离线修改后仍可继续编辑。");
    let error = device
        .sync()
        .run(SyncRunInput {
            passphrase: PASSPHRASE.to_string(),
            confirm: true,
        })
        .expect_err("服务停止时同步应失败");
    assert_eq!(error.code, ErrorCode::SyncFailed);
    assert!(
        device.sync().settings().pending_queue >= 1,
        "网络失败后本地修改必须进入持久重试队列"
    );
    let root = device.workspace_root().to_path_buf();
    WorkspaceService::new(Arc::clone(&device.app))
        .close()
        .expect("关闭离线队列工作区");
    WorkspaceService::new(Arc::clone(&device.app))
        .open(&root.to_string_lossy())
        .expect("重开离线队列工作区");
    assert!(
        device.sync().settings().pending_queue >= 1,
        "离线队列必须跨应用重启保留"
    );
    assert!(device
        .notes()
        .get(&note)
        .expect("关闭远端后本地仍可读取")
        .content
        .contains("离线修改"));
}

#[test]
fn attachments_and_optional_index_cache_sync_as_ciphertext() {
    let remote = LocalDav::start("Basic dXNlcjpwYXNz");
    let device_a = Device::new("附件设备 A");
    let device_b = Device::new("附件设备 B");
    device_b.clear_initial_notes();
    device_a.configure_webdav_with_index(&remote.endpoint);
    device_b.configure_webdav_with_index(&remote.endpoint);
    device_a.note("附件来源", "# 附件来源\n\n包含一个附件。\n");
    let saved = AttachmentService::new(device_a.app.context().expect("设备 A 上下文"))
        .save(SaveAttachmentInput {
            note_id: None,
            file_name: Some("sync-test.txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            data_base64: "aGVsbG8gYXR0YWNobWVudA==".to_string(),
        })
        .expect("保存附件");
    device_a.run_sync();
    device_b.run_sync();

    assert_eq!(
        std::fs::read(device_b.workspace_root().join(&saved.relative_path)).expect("设备 B 附件"),
        b"hello attachment"
    );
    assert!(
        device_b
            .workspace_root()
            .join(".knowledge/sync/index-cache.json")
            .is_file(),
        "可选索引缓存也应到达另一设备"
    );
    assert!(
        !remote.contains_plaintext("hello attachment") && !remote.contains_plaintext("index-cache"),
        "附件与索引缓存不得以明文上传"
    );
}

#[test]
fn downloaded_overwrite_is_archived_and_can_be_restored_after_confirmation() {
    let remote = LocalDav::start("Basic dXNlcjpwYXNz");
    let device_a = Device::new("历史设备 A");
    let device_b = Device::new("历史设备 B");
    device_b.clear_initial_notes();
    device_a.configure_webdav(&remote.endpoint);
    device_b.configure_webdav(&remote.endpoint);
    let note = device_a.note("同步历史", "# 同步历史\n\n第一版正文。");
    device_a.run_sync();
    device_b.run_sync();
    let b_note = device_b.find_note("同步历史");
    device_b.save_note(&b_note, "# 同步历史\n\n第二版正文。");
    device_b.run_sync();
    device_a.run_sync();
    assert!(device_a
        .notes()
        .get(&note)
        .expect("设备 A 下载第二版")
        .content
        .contains("第二版"));

    let history = device_a.sync().history().expect("同步历史列表");
    let archived = history
        .iter()
        .find(|entry| entry.category == "before-write" && entry.original_path.ends_with(".md"))
        .expect("下载覆盖前必须留档旧版本");
    assert!(archived.size_bytes > 0);
    let rejected = device_a
        .sync()
        .restore_history(RestoreSyncHistoryInput {
            relative_path: archived.relative_path.clone(),
            confirm: false,
        })
        .expect_err("未确认时不能还原");
    assert_eq!(rejected.code, ErrorCode::InvalidInput);
    let restored_path = device_a
        .sync()
        .restore_history(RestoreSyncHistoryInput {
            relative_path: archived.relative_path.clone(),
            confirm: true,
        })
        .expect("确认后还原历史");
    assert!(restored_path.ends_with(".md"));
    assert!(device_a
        .notes()
        .get(&note)
        .expect("还原后读取")
        .content
        .contains("第一版"));
    assert!(
        device_a
            .sync()
            .history()
            .expect("还原后历史")
            .iter()
            .any(|entry| entry.category == "before-restore"),
        "还原前也要先保存当前版本"
    );
}

#[test]
fn propagated_delete_is_archived_and_can_be_restored_into_the_note_index() {
    let remote = LocalDav::start("Basic dXNlcjpwYXNz");
    let device_a = Device::new("删除历史 A");
    let device_b = Device::new("删除历史 B");
    device_b.clear_initial_notes();
    device_a.configure_webdav(&remote.endpoint);
    device_b.configure_webdav(&remote.endpoint);
    device_a.note(
        "删除后可恢复",
        "# 删除后可恢复\n\n这版必须能从同步历史找回。",
    );
    device_a.run_sync();
    device_b.run_sync();
    let b_note = device_b.find_note("删除后可恢复");

    device_a
        .notes()
        .delete(&device_a.find_note("删除后可恢复"))
        .expect("设备 A 删除");
    device_a.run_sync();
    device_b.run_sync();
    assert!(device_b
        .notes()
        .list(NoteListQuery {
            page_size: Some(100),
            ..Default::default()
        })
        .expect("删除后的笔记列表")
        .items
        .iter()
        .all(|item| item.id != b_note));

    let archived = device_b
        .sync()
        .history()
        .expect("删除历史")
        .into_iter()
        .find(|entry| entry.category == "before-delete" && entry.original_path.ends_with(".md"))
        .expect("删除前必须留档");
    device_b
        .sync()
        .restore_history(RestoreSyncHistoryInput {
            relative_path: archived.relative_path,
            confirm: true,
        })
        .expect("从同步历史恢复删除文件");

    let restored = device_b
        .notes()
        .get(&b_note)
        .expect("恢复文件后原笔记记录也必须恢复");
    assert!(restored.content.contains("同步历史找回"));
    let searched = SearchService::new(device_b.app.context().expect("恢复后的上下文"))
        .search(SearchQuery {
            keyword: "同步历史找回".to_string(),
            ..Default::default()
        })
        .expect("恢复后的检索");
    assert!(
        searched.hits.iter().any(|hit| hit.note.id == b_note),
        "恢复文件后必须重新进入搜索索引"
    );
}
