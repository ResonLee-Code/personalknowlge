//! 内容发现：公开网页 → 链接候选 → 勾选深抓 → 内容收件箱 → 确认写入普通笔记。
//!
//! P0 只处理静态 HTML/文本，不执行 JavaScript、不递归调用浏览器 Agent。网页正文写入
//! `.knowledge/content-discovery/` 临时缓存，SQLite 只保存状态和元数据；正式笔记仍走
//! `NoteService`，因此历史版本、索引、标签、链接和关系更新保持单一写入路径。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use url::Url;

use crate::application::ai::decision_engine::DecisionEngine;
use crate::application::ai::local_model::OllamaModel;
use crate::application::ai::policy::AiPolicyService;
use crate::application::ai::remote_model::RemoteModel;
use crate::application::ai::rule_provider::RuleDecisionProvider;
use crate::application::ai::source_grade::grade_web;
use crate::application::ai::system_one::SystemOneDecisionModel;
use crate::application::ai::tools::AiTools;
use crate::application::ai::traits::{
    DecisionCandidate, DecisionChoice, DecisionKind, DecisionModel, DecisionState,
};
use crate::application::clip_service::{decode_html, extract_page_links, html_to_markdown};
use crate::application::context::WorkspaceContext;
use crate::application::note_service::NoteService;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::hash::{sha256_hex_str, short_hash};
use crate::core::ids::ulid;
use crate::core::text::{count_words, markdown_to_plain_text, truncate_chars};
use crate::core::time::{date_offset_days, now_iso};
use crate::domain::content_discovery::{
    ContentCapture, ContentCaptureInput, ContentCaptureSaveInput, ContentCaptureSaveResult,
    ContentCaptureStatus, ContentDiscoveryCandidate, ContentDiscoveryOptions, ContentDiscoveryPage,
    ContentDiscoveryRunInput, ContentSourceKind,
};
use crate::domain::note::{CreateNoteInput, SetSourceInput};
use crate::repositories::content_discovery_repository::{
    ContentDiscoveryRepository, SqliteContentDiscoveryRepository,
};

const MAX_BYTES: u64 = 3 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const MAX_CANDIDATES: usize = 200;
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; PersonalKnowledgeOS/1.3; +local-first content discovery)";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const REVIEW_PERIOD_DAYS: i64 = 180;

struct FetchedHttp {
    final_url: String,
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

struct FetchedPage {
    requested_url: String,
    final_url: String,
    title: String,
    site: Option<String>,
    markdown: String,
    links: Vec<crate::application::clip_service::ExtractedPageLink>,
    word_count: i64,
    bytes: usize,
    content_hash: String,
}

pub struct ContentDiscoveryService {
    ctx: Arc<WorkspaceContext>,
    repo: SqliteContentDiscoveryRepository,
    /// 仅供本机集成测试；release 构建始终拒绝 loopback/私网。
    allow_loopback: bool,
    decision_override: Option<Arc<dyn DecisionModel>>,
}

impl ContentDiscoveryService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self {
            ctx,
            repo: SqliteContentDiscoveryRepository,
            allow_loopback: false,
            decision_override: None,
        }
    }

    #[cfg(debug_assertions)]
    pub fn with_loopback_for_tests(ctx: Arc<WorkspaceContext>) -> Self {
        Self {
            ctx,
            repo: SqliteContentDiscoveryRepository,
            allow_loopback: true,
            decision_override: None,
        }
    }

    #[cfg(debug_assertions)]
    pub fn with_decision_model_for_tests(
        ctx: Arc<WorkspaceContext>,
        decision: Arc<dyn DecisionModel>,
    ) -> Self {
        Self {
            ctx,
            repo: SqliteContentDiscoveryRepository,
            allow_loopback: true,
            decision_override: Some(decision),
        }
    }

    /// 创建单页内容记录，并发现根页面中的候选链接。
    pub fn create(&self, input: ContentCaptureInput) -> Result<ContentCapture> {
        let url = self.request_url(&input)?;
        let policy = self.policy();
        policy.ensure_external_fetch_allowed("content_capture_create", &url, input.confirmed)?;
        let proxy = policy.outbound_proxy();
        let page = self.fetch_source(&url, proxy.as_deref(), "content_capture_create")?;
        let canonical_url = canonicalize_url(&page.final_url)?;
        if let Some(existing) = self.ctx.db.with_conn(|conn| {
            self.repo
                .find_capture_by_canonical(conn, self.ctx.workspace_id(), &canonical_url)
        })? {
            return self.get(&existing);
        }

        let id = format!("capture_{}", ulid());
        let cache_dir = format!(".knowledge/{}/{}", "content-discovery", id);
        let cache_path = self.cache_page(&cache_dir, &canonical_url, &page.markdown)?;
        let source_kind = detect_source_kind(&page.final_url);
        let options = ContentDiscoveryOptions::default();
        let mut candidates = self.build_candidates(&page, &options);
        let decision_provider = self
            .score_link_candidates(
                &page.title,
                input.user_note.as_deref(),
                &mut candidates,
                input.confirmed,
            )
            .unwrap_or_else(|_| "rule".to_string());
        let related = AiTools::new(Arc::clone(&self.ctx))
            .kb_search(&page.title, 3)
            .unwrap_or_default();
        let suggested_category_id = self.suggest_category(&related);
        let (summary, key_points) = deterministic_digest(&page.markdown, &page.title);
        let now = now_iso();
        let capture = ContentCapture {
            id: id.clone(),
            workspace_id: self.ctx.workspace_id().to_string(),
            requested_url: url,
            final_url: page.final_url.clone(),
            canonical_url: canonical_url.clone(),
            source_kind,
            title: page.title.clone(),
            author: None,
            site: page.site.clone(),
            published_at: None,
            captured_at: now.clone(),
            user_note: input
                .user_note
                .or(input.shared_text)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            status: ContentCaptureStatus::AwaitingSelection,
            summary: Some(summary),
            key_points,
            tags: vec![source_kind_tag(source_kind).to_string()],
            related_note_ids: related.iter().map(|hit| hit.note_id.clone()).collect(),
            related_notes: Vec::new(),
            suggested_category_id,
            decision_provider: Some(decision_provider),
            keep_full_text: options.keep_full_text,
            page_count: 1,
            word_count: page.word_count,
            cache_dir,
            error: None,
            note_id: None,
            duplicate_of: None,
            created_at: now.clone(),
            updated_at: now,
            saved_at: None,
            source_changed: false,
            last_refreshed_at: None,
            candidates,
            pages: Vec::new(),
            markdown: None,
        };
        let root_page = self.page_record(&page, None, 0, cache_path);
        self.ctx.db.with_conn(|conn| {
            conn.execute("BEGIN IMMEDIATE", [])?;
            if let Err(error) = self.repo.insert_capture(conn, &capture) {
                let _ = conn.execute("ROLLBACK", []);
                return Err(error);
            }
            if let Err(error) = self.repo.replace_candidates(conn, &id, &capture.candidates) {
                let _ = conn.execute("ROLLBACK", []);
                return Err(error);
            }
            if let Err(error) = self.repo.upsert_page(conn, &id, &root_page) {
                let _ = conn.execute("ROLLBACK", []);
                return Err(error);
            }
            conn.execute("COMMIT", [])?;
            Ok(())
        })?;
        self.get(&id)
    }

    pub fn list(&self, status: Option<&str>, limit: usize) -> Result<Vec<ContentCapture>> {
        self.ctx.db.with_conn(|conn| {
            self.repo
                .list_captures(conn, self.ctx.workspace_id(), status, limit)
        })
    }

    pub fn get(&self, capture_id: &str) -> Result<ContentCapture> {
        let mut capture = self.ctx.db.with_conn(|conn| {
            self.repo
                .get_capture(conn, self.ctx.workspace_id(), capture_id)?
                .ok_or_else(|| AppError::invalid_input("内容记录不存在。"))
        })?;
        let (candidates, pages) = self.ctx.db.with_conn(|conn| {
            Ok((
                self.repo.list_candidates(conn, capture_id)?,
                self.repo.list_pages(conn, capture_id)?,
            ))
        })?;
        capture.candidates = candidates;
        capture.pages = pages;
        let notes = NoteService::new(Arc::clone(&self.ctx));
        capture.related_notes = capture
            .related_note_ids
            .iter()
            .filter_map(|note_id| {
                notes.get(note_id).ok().map(|detail| {
                    crate::domain::content_discovery::ContentRelatedNote {
                        note_id: note_id.clone(),
                        title: detail.note.title,
                    }
                })
            })
            .collect();
        capture.markdown = Some(self.read_capture_markdown(&capture)?);
        Ok(capture)
    }

    /// 检查来源是否变化并刷新收件箱预览；不会覆盖已经保存的正式笔记。
    pub fn refresh(&self, capture_id: &str, confirmed: bool) -> Result<ContentCapture> {
        let mut capture = self.get(capture_id)?;
        if capture.status == ContentCaptureStatus::Ignored {
            return Err(AppError::invalid_input("已忽略的内容不能刷新。"));
        }
        let policy = self.policy();
        policy.ensure_external_fetch_allowed(
            "content_capture_refresh",
            &capture.final_url,
            confirmed,
        )?;
        let proxy = policy.outbound_proxy();
        let page = self.fetch_source(
            &capture.final_url,
            proxy.as_deref(),
            "content_capture_refresh",
        )?;
        let canonical_url = canonicalize_url(&page.final_url)?;
        let previous_hash = capture
            .pages
            .iter()
            .find(|item| item.depth == 0)
            .map(|item| item.content_hash.as_str());
        let source_changed = previous_hash != Some(page.content_hash.as_str());
        let root_cache_path = capture
            .pages
            .iter()
            .find(|item| item.depth == 0)
            .map(|item| item.cache_path.clone())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| format!("{}/{}.md", capture.cache_dir, short_hash(&canonical_url)));
        let cache_path = self.cache_page(&capture.cache_dir, &canonical_url, &page.markdown)?;
        let options = ContentDiscoveryOptions::default();
        let mut candidates = self.build_candidates(&page, &options);
        let decision_provider = self
            .score_link_candidates(
                &page.title,
                capture.user_note.as_deref(),
                &mut candidates,
                confirmed,
            )
            .unwrap_or_else(|_| "rule".to_string());
        let related = AiTools::new(Arc::clone(&self.ctx))
            .kb_search(&page.title, 3)
            .unwrap_or_default();
        let (summary, key_points) = deterministic_digest(&page.markdown, &page.title);
        let root_page = self.page_record(
            &page,
            None,
            0,
            if cache_path.is_empty() {
                root_cache_path
            } else {
                cache_path
            },
        );
        let now = now_iso();
        let mut pages = capture.pages.clone();
        pages.retain(|item| item.depth != 0);
        pages.insert(0, root_page.clone());
        capture.final_url = page.final_url;
        capture.canonical_url = canonical_url;
        capture.source_kind = detect_source_kind(&capture.final_url);
        capture.title = page.title;
        capture.site = page.site;
        capture.summary = Some(summary);
        capture.key_points = key_points;
        capture.related_note_ids = related.iter().map(|hit| hit.note_id.clone()).collect();
        capture.suggested_category_id = self.suggest_category(&related);
        capture.decision_provider = Some(decision_provider);
        capture.page_count = pages.iter().filter(|item| item.status == "ready").count();
        capture.word_count = pages.iter().map(|item| item.word_count).sum();
        capture.error = None;
        capture.source_changed = source_changed;
        capture.last_refreshed_at = Some(now.clone());
        capture.updated_at = now;
        capture.status = if capture.note_id.is_some() {
            ContentCaptureStatus::Saved
        } else if pages
            .iter()
            .any(|item| item.depth > 0 && item.status == "ready")
        {
            ContentCaptureStatus::Ready
        } else {
            ContentCaptureStatus::AwaitingSelection
        };
        self.ctx.db.with_conn(|conn| {
            conn.execute("BEGIN IMMEDIATE", [])?;
            if let Err(error) = self.repo.update_capture(conn, &capture) {
                let _ = conn.execute("ROLLBACK", []);
                return Err(error);
            }
            if let Err(error) = self.repo.replace_candidates(conn, &capture.id, &candidates) {
                let _ = conn.execute("ROLLBACK", []);
                return Err(error);
            }
            if let Err(error) = self.repo.upsert_page(conn, &capture.id, &root_page) {
                let _ = conn.execute("ROLLBACK", []);
                return Err(error);
            }
            conn.execute("COMMIT", [])?;
            Ok(())
        })?;
        self.get(&capture.id)
    }

    /// 抓取用户勾选的候选链接。P0 只完成一层；后端仍保留 `maxDepth` 以兼容后续扩展。
    pub fn crawl_selected(&self, input: ContentDiscoveryRunInput) -> Result<ContentCapture> {
        let mut capture = self.get(&input.capture_id)?;
        if matches!(
            capture.status,
            ContentCaptureStatus::Saved | ContentCaptureStatus::Ignored
        ) {
            return Err(AppError::invalid_input("已保存或忽略的内容不能再抓取。"));
        }
        let options = input.options.clamped();
        capture.keep_full_text = options.keep_full_text;
        let policy = self.policy();
        let proxy = policy.outbound_proxy();
        let known: HashMap<String, &ContentDiscoveryCandidate> = capture
            .candidates
            .iter()
            .map(|candidate| (candidate.canonical_url.clone(), candidate))
            .collect();
        let existing_ready: HashSet<String> = capture
            .pages
            .iter()
            .filter(|page| page.status == "ready")
            .map(|page| page.canonical_url.clone())
            .collect();
        let existing_failed: HashSet<String> = capture
            .pages
            .iter()
            .filter(|page| page.status == "failed")
            .map(|page| page.canonical_url.clone())
            .collect();
        let max_pages = options.max_pages as usize;
        let mut planned_page_count = capture.pages.len();
        let mut selected = Vec::new();
        let mut seen = HashSet::new();
        for raw in input.selected_urls {
            let canonical = canonicalize_url(&raw)?;
            if !seen.insert(canonical.clone()) {
                continue;
            }
            if let Some(candidate) = known.get(&canonical) {
                if !candidate_allowed(candidate, &capture.final_url, &options) {
                    continue;
                }
            } else {
                continue;
            }
            if existing_ready.contains(&canonical) {
                continue;
            }
            if !existing_failed.contains(&canonical) {
                if planned_page_count >= max_pages {
                    continue;
                }
                planned_page_count += 1;
            }
            selected.push(canonical);
        }
        let total = selected.len();
        capture.status = ContentCaptureStatus::Fetching;
        capture.updated_at = now_iso();
        self.ctx
            .db
            .with_conn(|conn| self.repo.update_capture(conn, &capture))?;

        let mut pages = capture.pages.clone();
        let mut hashes: HashSet<String> = pages
            .iter()
            .filter(|page| page.status == "ready")
            .map(|page| page.content_hash.clone())
            .collect();
        let mut completed = 0usize;
        self.emit_progress(&capture.id, "fetching", completed, total, None);
        for canonical in selected.into_iter().take(total) {
            let Some(candidate) = known.get(&canonical) else {
                continue;
            };
            let fetched = policy
                .ensure_external_fetch_allowed(
                    "content_discovery_crawl",
                    &candidate.url,
                    input.confirmed,
                )
                .and_then(|_| {
                    self.fetch_page(&candidate.url, proxy.as_deref(), "content_discovery_crawl")
                });
            match fetched {
                Ok(fetched) => {
                    if hashes.contains(&fetched.content_hash) {
                        completed += 1;
                        self.emit_progress(
                            &capture.id,
                            "fetching",
                            completed,
                            total,
                            Some(&canonical),
                        );
                        continue;
                    }
                    hashes.insert(fetched.content_hash.clone());
                    let cache_path =
                        self.cache_page(&capture.cache_dir, &canonical, &fetched.markdown)?;
                    let page =
                        self.page_record(&fetched, Some(capture.final_url.clone()), 1, cache_path);
                    self.ctx
                        .db
                        .with_conn(|conn| self.repo.upsert_page(conn, &capture.id, &page))?;
                    pages.retain(|item| item.canonical_url != canonical);
                    pages.push(page);
                }
                Err(error) => {
                    let failed_page = ContentDiscoveryPage {
                        url: candidate.url.clone(),
                        canonical_url: canonical.clone(),
                        parent_url: Some(capture.final_url.clone()),
                        depth: 1,
                        title: candidate.title.clone(),
                        site: None,
                        content_hash: sha256_hex_str(&canonical),
                        word_count: 0,
                        bytes: 0,
                        cache_path: String::new(),
                        status: "failed".to_string(),
                        fetched_at: now_iso(),
                        error: Some(error.message),
                    };
                    self.ctx
                        .db
                        .with_conn(|conn| self.repo.upsert_page(conn, &capture.id, &failed_page))?;
                    pages.retain(|item| item.canonical_url != canonical);
                    pages.push(failed_page);
                }
            }
            completed += 1;
            self.emit_progress(&capture.id, "fetching", completed, total, Some(&canonical));
        }
        capture.pages = pages;
        capture.page_count = capture
            .pages
            .iter()
            .filter(|page| page.status == "ready")
            .count();
        capture.word_count = capture.pages.iter().map(|page| page.word_count).sum();
        capture.summary = Some(self.aggregate_summary(&capture)?);
        capture.status = ContentCaptureStatus::Ready;
        capture.updated_at = now_iso();
        self.ctx
            .db
            .with_conn(|conn| self.repo.update_capture(conn, &capture))?;
        self.emit_progress(&capture.id, "ready", completed, total, None);
        self.get(&capture.id)
    }

    pub fn save(&self, input: ContentCaptureSaveInput) -> Result<ContentCaptureSaveResult> {
        let mut capture = self.get(&input.capture_id)?;
        if let Some(note_id) = capture.note_id.clone() {
            return Ok(ContentCaptureSaveResult {
                capture_id: capture.id,
                note_title: capture.title,
                note_id,
            });
        }
        if !matches!(
            capture.status,
            ContentCaptureStatus::Ready
                | ContentCaptureStatus::AwaitingSelection
                | ContentCaptureStatus::Duplicate
        ) {
            return Err(AppError::invalid_input("内容尚未准备好，不能写入知识库。"));
        }
        if let Some(category_id) = input.category_id.as_deref() {
            self.ctx.ensure_category_exists(category_id)?;
        }
        let title = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&capture.title)
            .to_string();
        if let Some(keep_full_text) = input.keep_full_text {
            capture.keep_full_text = keep_full_text;
        }
        let tags = input.tags.unwrap_or_else(|| capture.tags.clone());
        let body = self.build_note_markdown(&capture)?;
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let detail = notes.create(CreateNoteInput {
            title: title.clone(),
            category_id: input.category_id,
            initial_content: Some(body),
            tags: Some(tags.clone()),
            parent_id: None,
        })?;
        notes.set_source(SetSourceInput {
            id: detail.note.id.clone(),
            source_type: Some(source_kind_tag(capture.source_kind).to_string()),
            source_ref: Some(capture.final_url.clone()),
            source_reliability: Some(source_reliability(capture.source_kind)),
            // 内容发现默认进入复核周期，避免“收进来以后永不消化”。
            valid_until: Some(date_offset_days(REVIEW_PERIOD_DAYS)),
        })?;
        let now = now_iso();
        capture.status = ContentCaptureStatus::Saved;
        capture.tags = tags;
        capture.note_id = Some(detail.note.id.clone());
        capture.saved_at = Some(now.clone());
        capture.updated_at = now;
        self.ctx
            .db
            .with_conn(|conn| self.repo.update_capture(conn, &capture))?;
        Ok(ContentCaptureSaveResult {
            capture_id: capture.id,
            note_id: detail.note.id,
            note_title: title,
        })
    }

    pub fn ignore(&self, capture_id: &str) -> Result<ContentCapture> {
        let mut capture = self.get(capture_id)?;
        if capture.note_id.is_some() {
            return Err(AppError::invalid_input(
                "已写入知识库的内容不能标记为忽略。",
            ));
        }
        capture.status = ContentCaptureStatus::Ignored;
        capture.updated_at = now_iso();
        self.ctx
            .db
            .with_conn(|conn| self.repo.update_capture(conn, &capture))?;
        Ok(capture)
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(Arc::clone(&self.ctx))
    }

    fn request_url(&self, input: &ContentCaptureInput) -> Result<String> {
        let raw = if input.url.trim().is_empty() {
            input.shared_text.as_deref().unwrap_or_default()
        } else {
            input.url.as_str()
        };
        extract_first_url(raw)
            .ok_or_else(|| AppError::invalid_input("分享内容里没有找到 http/https URL。"))
    }

    fn fetch_page(
        &self,
        raw_url: &str,
        proxy: Option<&str>,
        capability: &str,
    ) -> Result<FetchedPage> {
        let url = normalize_url(raw_url)?;
        self.ensure_public_url(&url)?;
        let response = self.fetch_http(&url, proxy, capability)?;
        if response.status >= 400 {
            return Err(AppError::new(
                ErrorCode::ExportFailed,
                format!(
                    "抓取失败：HTTP {}（{}）",
                    response.status, response.final_url
                ),
            ));
        }
        let content_type = response.content_type.to_ascii_lowercase();
        let is_html = content_type.is_empty() || content_type.contains("html");
        let is_text = content_type.starts_with("text/");
        if !is_html && !is_text {
            return Err(AppError::invalid_input(format!(
                "该地址不是可抽取的网页（Content-Type: {content_type}）。"
            )));
        }
        let text = decode_html(&response.body, &content_type);
        let (title, markdown, site, links) = if is_html {
            let (title, markdown, site) = html_to_markdown(&text, &response.final_url)?;
            let links = extract_page_links(&text, &response.final_url);
            (title, markdown, site, links)
        } else {
            let title = response
                .final_url
                .split("://")
                .nth(1)
                .and_then(|rest| rest.split('/').next())
                .unwrap_or("网页内容")
                .to_string();
            (title, text, None, Vec::new())
        };
        if markdown.trim().is_empty() {
            return Err(AppError::invalid_input(
                "没有抽取到正文（页面可能要求登录或完全依赖 JavaScript）。",
            ));
        }
        let plain = markdown_to_plain_text(&markdown);
        Ok(FetchedPage {
            requested_url: raw_url.trim().to_string(),
            final_url: response.final_url,
            title,
            site,
            word_count: count_words(&plain),
            bytes: response.body.len(),
            content_hash: sha256_hex_str(&markdown),
            markdown,
            links,
        })
    }

    fn fetch_source(
        &self,
        url: &str,
        proxy: Option<&str>,
        capability: &str,
    ) -> Result<FetchedPage> {
        if let Some(repo) = git_readme_candidate(url) {
            if let Ok(readme) = self.fetch_text(&repo.raw_url, proxy, capability) {
                if !readme.trim().is_empty() {
                    let links = crate::core::text::parse_markdown_links(&readme)
                        .into_iter()
                        .filter_map(|item| {
                            let parsed = Url::parse(&item.target)
                                .ok()
                                .or_else(|| Url::parse(url).ok()?.join(&item.target).ok())?;
                            matches!(parsed.scheme(), "http" | "https").then(|| {
                                crate::application::clip_service::ExtractedPageLink {
                                    url: parsed.to_string(),
                                    title: item.text.unwrap_or_else(|| parsed.to_string()),
                                    context: item.context,
                                    hidden: false,
                                }
                            })
                        })
                        .collect();
                    return Ok(FetchedPage {
                        requested_url: url.to_string(),
                        final_url: url.to_string(),
                        title: repo.title,
                        site: Some(repo.site.to_string()),
                        word_count: count_words(&markdown_to_plain_text(&readme)),
                        bytes: readme.len(),
                        content_hash: sha256_hex_str(&readme),
                        markdown: readme,
                        links,
                    });
                }
            }
        }
        self.fetch_page(url, proxy, capability)
    }

    fn fetch_text(&self, url: &str, proxy: Option<&str>, capability: &str) -> Result<String> {
        let response = self.fetch_http(url, proxy, capability)?;
        if response.status >= 400 {
            return Err(AppError::new(
                ErrorCode::ExportFailed,
                format!("读取 Git README 失败：HTTP {}", response.status),
            ));
        }
        Ok(decode_html(&response.body, &response.content_type))
    }

    fn fetch_http(
        &self,
        raw_url: &str,
        proxy: Option<&str>,
        capability: &str,
    ) -> Result<FetchedHttp> {
        self.fetch_http_with_robots(raw_url, proxy, capability, true)
    }

    fn fetch_http_with_robots(
        &self,
        raw_url: &str,
        proxy: Option<&str>,
        capability: &str,
        respect_robots: bool,
    ) -> Result<FetchedHttp> {
        let mut current = normalize_url(raw_url)?;
        for _ in 0..=MAX_REDIRECTS {
            self.ensure_public_url(&current)?;
            self.policy()
                .ensure_external_fetch_allowed(capability, &current, true)?;
            if respect_robots {
                // 每个重定向目标都重新检查；robots.txt 自身只做 URL / 策略校验，避免递归。
                self.ensure_robots_allowed(&current, proxy, capability)?;
            }
            let mut config = ureq::Agent::config_builder()
                .timeout_global(Some(REQUEST_TIMEOUT))
                .http_status_as_error(false)
                .max_redirects(0);
            if let Some(raw_proxy) = crate::core::net::resolve_proxy(&current, proxy) {
                if let Ok(proxy) = ureq::Proxy::new(&raw_proxy) {
                    config = config.proxy(Some(proxy));
                }
            }
            let agent = ureq::Agent::new_with_config(config.build());
            let response = agent
                .get(&current)
                .header("User-Agent", USER_AGENT)
                .header(
                    "Accept",
                    "text/html,text/plain,application/xhtml+xml;q=0.9,*/*;q=0.5",
                )
                .call()
                .map_err(|error| {
                    AppError::new(
                        ErrorCode::ExportFailed,
                        format!("联网抓取失败：{error}（请检查网络、代理或 URL）"),
                    )
                })?;
            let status = response.status().as_u16();
            if (300..400).contains(&status) {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| AppError::invalid_input("重定向缺少 Location。"))?;
                current = Url::parse(&current)
                    .and_then(|base| base.join(location))
                    .map_err(|error| AppError::invalid_input(format!("重定向地址无效：{error}")))?
                    .to_string();
                continue;
            }
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_string();
            let mut response = response;
            let body = response
                .body_mut()
                .with_config()
                .limit(MAX_BYTES)
                .read_to_vec()
                .map_err(|error| {
                    AppError::new(
                        ErrorCode::ExportFailed,
                        format!("读取网页失败：{error}（单页上限 3 MB）"),
                    )
                })?;
            return Ok(FetchedHttp {
                final_url: current,
                status,
                content_type,
                body,
            });
        }
        Err(AppError::invalid_input("重定向次数过多。"))
    }

    fn ensure_robots_allowed(
        &self,
        raw_url: &str,
        proxy: Option<&str>,
        capability: &str,
    ) -> Result<()> {
        let url = Url::parse(raw_url)
            .map_err(|error| AppError::invalid_input(format!("URL 无效：{error}")))?;
        let robots = format!("{}/robots.txt", url.origin().ascii_serialization());
        let Ok(response) = self.fetch_http_with_robots(&robots, proxy, capability, false) else {
            return Ok(());
        };
        if !(200..300).contains(&response.status) {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&response.body);
        if robots_disallows(&text, url.path()) {
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                format!("robots.txt 不允许抓取：{}", url.path()),
            ));
        }
        Ok(())
    }

    fn ensure_public_url(&self, raw_url: &str) -> Result<()> {
        let url = Url::parse(raw_url)
            .map_err(|error| AppError::invalid_input(format!("URL 无效：{error}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::invalid_input("只支持 http/https URL。"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(AppError::invalid_input("URL 不能包含用户名或密码。"));
        }
        let host = url
            .host_str()
            .ok_or_else(|| AppError::invalid_input("URL 缺少主机名。"))?;
        let port = url.port_or_known_default().unwrap_or(443);
        let addresses = (host, port)
            .to_socket_addrs()
            .map_err(|error| AppError::invalid_input(format!("无法解析主机 {host}：{error}")))?
            .map(|address| address.ip())
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            return Err(AppError::invalid_input(format!(
                "主机 {host} 没有可用地址。"
            )));
        }
        for address in addresses {
            if !self.allow_loopback && !is_public_ip(address) {
                return Err(AppError::new(
                    ErrorCode::NetworkDisabled,
                    format!("拒绝抓取私网、回环或保留地址：{host}"),
                ));
            }
        }
        Ok(())
    }

    fn build_candidates(
        &self,
        page: &FetchedPage,
        options: &ContentDiscoveryOptions,
    ) -> Vec<ContentDiscoveryCandidate> {
        let root = Url::parse(&page.final_url).ok();
        let base_path = root.as_ref().map(root_base_path).unwrap_or_default();
        let mut seen = HashSet::new();
        page.links
            .iter()
            .take(MAX_CANDIDATES)
            .filter_map(|link| {
                let canonical = canonicalize_url(&link.url).ok()?;
                if !seen.insert(canonical.clone()) {
                    return None;
                }
                let parsed = Url::parse(&canonical).ok()?;
                let same_origin = root.as_ref().is_some_and(|root| {
                    root.scheme() == parsed.scheme()
                        && root.host_str() == parsed.host_str()
                        && root.port_or_known_default() == parsed.port_or_known_default()
                });
                let same_site = same_site(&page.final_url, &canonical);
                let same_path = same_origin && parsed.path().starts_with(&base_path);
                let blocked_reason = if link.hidden && !options.include_hidden {
                    Some("hidden".to_string())
                } else if !same_origin
                    && !options.include_external
                    && !(options.include_subdomains && same_site)
                {
                    Some("external".to_string())
                } else if options.same_path && !same_path && !options.include_external {
                    Some("outsidePath".to_string())
                } else {
                    None
                };
                Some(ContentDiscoveryCandidate {
                    url: link.url.clone(),
                    canonical_url: canonical,
                    parent_url: page.final_url.clone(),
                    depth: 1,
                    title: if link.title.trim().is_empty() {
                        link.url.clone()
                    } else {
                        link.title.clone()
                    },
                    anchor_text: Some(link.title.clone()).filter(|value| !value.is_empty()),
                    context: link.context.clone(),
                    hidden: link.hidden,
                    same_origin,
                    same_site,
                    eligible: blocked_reason.is_none(),
                    blocked_reason,
                    relevance_score: None,
                    relevance_reason: None,
                    selected: false,
                })
            })
            .collect()
    }

    /// P1：Decision Model 在用户勾选前批量筛选候选链接。
    ///
    /// 规则档仍给出分数用于排序，但不自动删除或勾选，避免离线关键词误伤。
    fn score_link_candidates(
        &self,
        page_title: &str,
        user_note: Option<&str>,
        candidates: &mut [ContentDiscoveryCandidate],
        confirm_outbound: bool,
    ) -> Result<String> {
        if candidates.is_empty() {
            return Ok("rule".to_string());
        }
        let policy = self.policy();
        let raw_policy = policy.policy();
        let mut chain: Vec<Arc<dyn DecisionModel>> = Vec::new();
        if let Some(decision) = &self.decision_override {
            chain.push(Arc::clone(decision));
        } else {
            if let Some(config) = policy.decision_config() {
                if policy.outbound_confirmed(confirm_outbound) {
                    chain.push(Arc::new(SystemOneDecisionModel::new(&config)));
                }
            }
            if raw_policy.remote_model_score
                && policy
                    .ensure_remote_model_allowed("content_discovery_candidates")
                    .is_ok()
                && policy.outbound_confirmed(confirm_outbound)
            {
                if let Some((base, model, key)) = policy.remote_model() {
                    chain.push(Arc::new(
                        RemoteModel::new(&base, &model, &key).with_proxy(policy.outbound_proxy()),
                    ));
                }
            }
            if raw_policy.local_model_score {
                if let Some((endpoint, model)) = policy.local_model() {
                    chain.push(Arc::new(OllamaModel::new(&endpoint, &model)));
                }
            }
        }
        chain.push(Arc::new(RuleDecisionProvider::default()));
        let query = match user_note.map(str::trim).filter(|value| !value.is_empty()) {
            Some(note) => format!("{page_title}；保存意图：{note}"),
            None => page_title.to_string(),
        };
        let state = DecisionState {
            query,
            candidates: candidates
                .iter()
                .map(|candidate| {
                    let grade = grade_web(&candidate.url, &candidate.title, &candidate.context);
                    DecisionCandidate {
                        note_id: candidate.canonical_url.clone(),
                        title: candidate.title.clone(),
                        snippet: candidate.context.clone(),
                        updated_at: now_iso(),
                        source_reliability: Some(grade.level.reliability()),
                        valid_until: None,
                    }
                })
                .collect(),
        };
        let outcome =
            DecisionEngine::new().decide(&self.ctx.db, DecisionKind::Relevance, &state, &chain)?;
        let provider = if outcome.provider.is_empty() {
            "rule".to_string()
        } else {
            outcome.provider.clone()
        };
        let by_id: HashMap<String, _> = outcome
            .results
            .iter()
            .map(|result| (result.note_id.clone(), result))
            .collect();
        let model_active = provider != "rule";
        for candidate in candidates.iter_mut() {
            if let Some(result) = by_id.get(&candidate.canonical_url) {
                candidate.relevance_score = Some(result.score);
                candidate.relevance_reason = Some(result.reason.clone());
                if model_active && result.choice == DecisionChoice::Drop && result.score < 0.25 {
                    candidate.eligible = false;
                    candidate.blocked_reason = Some("decisionLowRelevance".to_string());
                    candidate.selected = false;
                } else if model_active && result.score >= 0.66 && candidate.eligible {
                    candidate.selected = true;
                }
            }
        }
        if let Some(note) = outcome
            .trace
            .attempts
            .iter()
            .find(|attempt| !attempt.ok)
            .map(|attempt| format!("{} 不可用：{}", attempt.provider, attempt.detail))
        {
            policy.audit(
                "content_discovery_candidates",
                provider != "rule",
                "error",
                outcome.trace.took_ms,
                Some(&note),
            );
        }
        Ok(provider)
    }

    fn cache_page(&self, cache_dir: &str, canonical_url: &str, markdown: &str) -> Result<String> {
        let relative = format!("{cache_dir}/{}.md", short_hash(canonical_url));
        let absolute = self.ctx.paths.resolve_relative(&relative)?;
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(absolute, markdown)?;
        Ok(relative)
    }

    fn page_record(
        &self,
        page: &FetchedPage,
        parent_url: Option<String>,
        depth: u32,
        cache_path: String,
    ) -> ContentDiscoveryPage {
        ContentDiscoveryPage {
            url: page.requested_url.clone(),
            canonical_url: canonicalize_url(&page.final_url)
                .unwrap_or_else(|_| page.final_url.clone()),
            parent_url,
            depth,
            title: page.title.clone(),
            site: page.site.clone(),
            content_hash: page.content_hash.clone(),
            word_count: page.word_count,
            bytes: page.bytes,
            cache_path,
            status: "ready".to_string(),
            fetched_at: now_iso(),
            error: None,
        }
    }

    fn read_capture_markdown(&self, capture: &ContentCapture) -> Result<String> {
        let mut combined = String::new();
        for page in capture.pages.iter().filter(|page| page.status == "ready") {
            let absolute = self.ctx.paths.resolve_relative(&page.cache_path)?;
            let content = fs::read_to_string(absolute).unwrap_or_default();
            if content.trim().is_empty() {
                continue;
            }
            if !combined.is_empty() {
                combined.push_str("\n\n---\n\n");
            }
            if page.depth > 0 {
                combined.push_str(&format!("## {}\n\n", page.title));
            }
            combined.push_str(&content);
        }
        Ok(combined)
    }

    fn aggregate_summary(&self, capture: &ContentCapture) -> Result<String> {
        let markdown = self.read_capture_markdown(capture)?;
        Ok(deterministic_digest(&markdown, &capture.title).0)
    }

    fn build_note_markdown(&self, capture: &ContentCapture) -> Result<String> {
        let markdown = self.read_capture_markdown(capture)?;
        if markdown.trim().is_empty() {
            return Err(AppError::invalid_input("页面正文为空，不能写入笔记。"));
        }
        let mut out = format!(
            "# {}\n\n> 来源：[{}]({})\n> 抓取时间：{}\n",
            capture.title,
            capture.site.as_deref().unwrap_or(&capture.title),
            capture.final_url,
            capture.captured_at
        );
        if let Some(note) = capture
            .user_note
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            out.push_str(&format!("> 保存备注：{}\n", note.trim()));
        }
        if let Some(summary) = capture.summary.as_deref().filter(|value| !value.is_empty()) {
            out.push_str("\n## 摘要\n\n");
            out.push_str(summary);
            out.push('\n');
        }
        if !capture.key_points.is_empty() {
            out.push_str("\n## 关键观点\n\n");
            for point in &capture.key_points {
                out.push_str(&format!("- {point}\n"));
            }
        }
        if capture.keep_full_text {
            out.push_str("\n## 原文\n\n");
            out.push_str(&markdown);
        } else {
            out.push_str("\n## 内容摘录\n\n");
            out.push_str(&truncate_chars(&markdown, 1800));
            out.push_str("\n\n> 完整原文未长期保存；可从来源链接重新阅读。\n");
        }
        Ok(out)
    }

    fn emit_progress(
        &self,
        capture_id: &str,
        status: &str,
        completed: usize,
        total: usize,
        current_url: Option<&str>,
    ) {
        self.ctx.events.emit(
            event_names::CONTENT_DISCOVERY_PROGRESS,
            json!({
                "captureId": capture_id,
                "status": status,
                "completed": completed,
                "total": total,
                "currentUrl": current_url,
            }),
        );
    }

    fn suggest_category(&self, related: &[crate::application::ai::tools::KbHit]) -> Option<String> {
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let mut counts: HashMap<String, usize> = HashMap::new();
        for hit in related {
            if let Ok(detail) = notes.get(&hit.note_id) {
                if let Some(category_id) = detail.note.category_id {
                    *counts.entry(category_id).or_default() += 1;
                }
            }
        }
        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(id, _)| id)
    }
}

fn deterministic_digest(markdown: &str, fallback_title: &str) -> (String, Vec<String>) {
    let plain = markdown_to_plain_text(markdown);
    let sentences = plain
        .split(['。', '！', '？', '.', '!', '?', '\n'])
        .map(str::trim)
        .filter(|value| value.chars().count() >= 8)
        .take(3)
        .map(|value| format!("{value}。"))
        .collect::<Vec<_>>();
    let summary = if sentences.is_empty() {
        format!("已捕获《{fallback_title}》，等待进一步阅读和整理。")
    } else {
        truncate_chars(&sentences.join(""), 420)
    };
    let mut points = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        let text = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
            .or_else(|| trimmed.strip_prefix("# "))
            .or_else(|| trimmed.strip_prefix("- "));
        if let Some(text) = text
            .map(str::trim)
            .filter(|value| value.chars().count() >= 4)
        {
            if text != fallback_title && !points.iter().any(|item| item == text) {
                points.push(truncate_chars(text, 160));
            }
        }
        if points.len() >= 5 {
            break;
        }
    }
    (summary, points)
}

fn extract_first_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if let Ok(url) = Url::parse(trimmed) {
        if matches!(url.scheme(), "http" | "https") {
            return Some(url.to_string());
        }
    }
    value
        .split_whitespace()
        .find(|part| part.starts_with("http://") || part.starts_with("https://"))
        .map(|part| {
            part.trim_matches(|ch: char| matches!(ch, '"' | '\'' | ')' | ']' | '，' | '。'))
                .to_string()
        })
}

fn normalize_url(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("URL 不能为空。"));
    }
    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    Url::parse(&candidate)
        .map(|url| url.to_string())
        .map_err(|error| AppError::invalid_input(format!("URL 无效：{error}")))
}

fn canonicalize_url(value: &str) -> Result<String> {
    let mut url = Url::parse(&normalize_url(value)?)
        .map_err(|error| AppError::invalid_input(format!("URL 无效：{error}")))?;
    url.set_fragment(None);
    let retained = url
        .query_pairs()
        .filter(|(key, _)| {
            !key.eq_ignore_ascii_case("utm_source")
                && !key.eq_ignore_ascii_case("utm_medium")
                && !key.eq_ignore_ascii_case("utm_campaign")
                && !key.eq_ignore_ascii_case("utm_term")
                && !key.eq_ignore_ascii_case("utm_content")
                && !matches!(key.as_ref(), "fbclid" | "gclid" | "yclid")
        })
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    if retained.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(retained);
    }
    Ok(url.to_string())
}

fn root_base_path(url: &Url) -> String {
    let path = url.path();
    if path.ends_with('/') {
        path.to_string()
    } else {
        path.rsplit_once('/')
            .map(|(parent, _)| format!("{parent}/"))
            .unwrap_or_else(|| "/".to_string())
    }
}

fn same_site(left: &str, right: &str) -> bool {
    let (Ok(left), Ok(right)) = (Url::parse(left), Url::parse(right)) else {
        return false;
    };
    root_domain(left.host_str().unwrap_or_default())
        == root_domain(right.host_str().unwrap_or_default())
}

fn candidate_allowed(
    candidate: &ContentDiscoveryCandidate,
    root_url: &str,
    options: &ContentDiscoveryOptions,
) -> bool {
    if candidate.hidden && !options.include_hidden {
        return false;
    }
    if !candidate.same_origin
        && !options.include_external
        && !(options.include_subdomains && same_site(root_url, &candidate.url))
    {
        return false;
    }
    if candidate.blocked_reason.as_deref() == Some("outsidePath") && options.same_path {
        return false;
    }
    matches!(
        candidate.blocked_reason.as_deref(),
        None | Some("hidden") | Some("external") | Some("outsidePath")
    )
}

fn root_domain(host: &str) -> String {
    let labels = host.trim_matches('.').split('.').collect::<Vec<_>>();
    if labels.len() >= 2 {
        labels[labels.len() - 2..].join(".")
    } else {
        host.to_string()
    }
}

fn detect_source_kind(url: &str) -> ContentSourceKind {
    let host = Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default();
    if host == "mp.weixin.qq.com" {
        ContentSourceKind::Wechat
    } else if host == "github.com"
        || host == "gitlab.com"
        || host == "gitee.com"
        || host.ends_with(".github.com")
    {
        ContentSourceKind::Git
    } else {
        ContentSourceKind::Web
    }
}

struct GitReadmeCandidate {
    raw_url: String,
    title: String,
    site: &'static str,
}

fn git_readme_candidate(url: &str) -> Option<GitReadmeCandidate> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let parts = parsed
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    let owner = parts[0];
    let repo = parts[1].trim_end_matches(".git");
    match host.as_str() {
        "github.com" => Some(GitReadmeCandidate {
            raw_url: format!("https://raw.githubusercontent.com/{owner}/{repo}/HEAD/README.md"),
            title: format!("{owner}/{repo}"),
            site: "GitHub",
        }),
        "gitlab.com" => Some(GitReadmeCandidate {
            raw_url: format!("https://gitlab.com/{owner}/{repo}/-/raw/HEAD/README.md"),
            title: format!("{owner}/{repo}"),
            site: "GitLab",
        }),
        "gitee.com" => Some(GitReadmeCandidate {
            raw_url: format!("https://gitee.com/{owner}/{repo}/raw/HEAD/README.md"),
            title: format!("{owner}/{repo}"),
            site: "Gitee",
        }),
        _ => None,
    }
}

fn source_kind_tag(kind: ContentSourceKind) -> &'static str {
    match kind {
        ContentSourceKind::Web => "网页发现",
        ContentSourceKind::Wechat => "微信公众号",
        ContentSourceKind::Git => "Git资料",
    }
}

fn source_reliability(kind: ContentSourceKind) -> i64 {
    match kind {
        ContentSourceKind::Git => 4,
        ContentSourceKind::Wechat | ContentSourceKind::Web => 3,
    }
}

fn robots_disallows(text: &str, path: &str) -> bool {
    let mut applies = false;
    let mut best_allow = None::<usize>;
    let mut best_disallow = None::<usize>;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "user-agent" => {
                applies = value.trim() == "*"
                    || value
                        .trim()
                        .to_ascii_lowercase()
                        .contains("personalknowledgeos");
            }
            "allow" if applies => {
                let rule = value.trim();
                if !rule.is_empty() && path.starts_with(rule) {
                    best_allow = Some(best_allow.unwrap_or(0).max(rule.len()));
                }
            }
            "disallow" if applies => {
                let rule = value.trim();
                if !rule.is_empty() && path.starts_with(rule) {
                    best_disallow = Some(best_disallow.unwrap_or(0).max(rule.len()));
                }
            }
            _ => {}
        }
    }
    match (best_allow, best_disallow) {
        (Some(allow), Some(disallow)) => disallow > allow,
        (None, Some(_)) => true,
        _ => false,
    }
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_public_v4(mapped);
            }
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                || (ip.segments()[0] & 0xffc0) == 0xfe80
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8))
        }
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 198 && matches!(octets[1], 18 | 19)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_url_removes_fragments_and_tracking() {
        assert_eq!(
            canonicalize_url("https://example.com/a?utm_source=x&id=2#part").unwrap(),
            "https://example.com/a?id=2"
        );
    }

    #[test]
    fn robots_disallow_respects_more_specific_allow() {
        let robots = "User-agent: *\nDisallow: /private\nAllow: /private/public\n";
        assert!(robots_disallows(robots, "/private/secret"));
        assert!(!robots_disallows(robots, "/private/public/doc"));
        assert!(!robots_disallows(robots, "/public"));
    }

    #[test]
    fn private_and_reserved_ips_are_rejected() {
        for value in ["127.0.0.1", "10.0.0.1", "169.254.1.1", "192.168.1.1"] {
            assert!(!is_public_ip(value.parse().unwrap()), "{value}");
        }
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn extracts_first_url_from_shared_text() {
        assert_eq!(
            extract_first_url("看看这个 https://example.com/a?q=1。").as_deref(),
            Some("https://example.com/a?q=1")
        );
    }

    #[test]
    fn detects_wechat_and_git_sources() {
        assert_eq!(
            detect_source_kind("https://mp.weixin.qq.com/s/example"),
            ContentSourceKind::Wechat
        );
        assert_eq!(
            detect_source_kind("https://github.com/example/project"),
            ContentSourceKind::Git
        );
        let readme =
            git_readme_candidate("https://github.com/example/project").expect("GitHub README 候选");
        assert_eq!(
            readme.raw_url,
            "https://raw.githubusercontent.com/example/project/HEAD/README.md"
        );
        let gitlab =
            git_readme_candidate("https://gitlab.com/example/project").expect("GitLab README 候选");
        assert_eq!(
            gitlab.raw_url,
            "https://gitlab.com/example/project/-/raw/HEAD/README.md"
        );
        let gitee =
            git_readme_candidate("https://gitee.com/example/project").expect("Gitee README 候选");
        assert_eq!(
            gitee.raw_url,
            "https://gitee.com/example/project/raw/HEAD/README.md"
        );
        assert!(git_readme_candidate("https://github.com/example/project/issues").is_none());
    }
}
