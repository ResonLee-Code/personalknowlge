//! Ask：带原文引用的本地知识库问答（V2.0 M1）。
//!
//! 这条链路只做四件事，且顺序不可交换：
//! 1. 过 AI 总开关；
//! 2. 在指定作用域检索，并把命中映射到 `note_chunks` 的真实原文字符区间；
//! 3. 在线（已授权且本次确认）→ 本地 → 模板逐档生成；
//! 4. **严格解析引用**：模型只能引用本次材料里的编号，出现未知编号就判为无依据并降级。
//!
//! 问答本身只读；不会创建会话文件、不会写笔记，也不把上一轮答案当事实。

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use regex::Regex;

use crate::application::note_service::NoteService;
use crate::application::search_service::SearchService;
use crate::core::chunk::chunk_markdown;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::text::truncate_chars;
use crate::domain::ai::{
    AskAnswer, AskCitation, AskNotePreview, AskQuestionInput, AskSaveInput, AskSaveResult,
    AskScope, AskSourcePreview, RetrievalChannelStatus,
};
use crate::domain::note::{CreateNoteInput, NoteListQuery};
use crate::domain::research::SourceLevel;
use crate::domain::search::{SearchHit, SearchQuery};
use crate::repositories::chunk_repository::{ChunkRepository, SqliteChunkRepository};

use super::model_gateway::generation_candidates;
use super::policy::AiPolicyService;
use super::query_expansion;
use super::source_grade::{grade_local_note, is_stale};
use super::traits::{GenerationRequest, GenerativeModel, RetrievalOrigin, RetrievedChunk};
use crate::application::WorkspaceContext;

const MAX_EVIDENCE: usize = 8;
const MAX_EVIDENCE_CHARS: usize = 700;
const MAX_QUESTION_CHARS: usize = 2000;
const MAX_SELECTION_CHARS: usize = 4000;
const MAX_ANSWER_CHARS: usize = 50_000;
const MAX_ASK_QUERIES: usize = 8;
const MAX_HISTORY_TURNS: usize = 6;
const MAX_HISTORY_ANSWER_CHARS: usize = 800;
const MAX_ASK_INPUT_TOKENS: usize = 6_000;
const ASK_MAX_OUTPUT_TOKENS: u32 = 700;

/// 问句里常见、但不该单独占据检索预算的双字组合。
const QUESTION_STOP_BIGRAMS: &[&str] = &[
    "之前", "关于", "请问", "我们", "可以", "如何", "怎么", "是否", "一下", "哪些", "有哪", "什么",
    "么结", "的结", "结论", "是什", "有什", "这里", "这篇", "笔记", "知识", "识库", "库里", "内容",
    "细节", "告诉",
];

struct Evidence {
    chunk: RetrievedChunk,
    title: String,
    level: SourceLevel,
    stale: bool,
}

struct Retrieval {
    evidence: Vec<Evidence>,
    channels: Vec<RetrievalChannelStatus>,
    warnings: Vec<String>,
}

/// Ask 业务服务。构造时不联网；只有 `answer` 在策略允许时才会构造在线模型。
pub struct AskService {
    ctx: Arc<WorkspaceContext>,
}

impl AskService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn answer(&self, input: AskQuestionInput) -> Result<AskAnswer> {
        let started = Instant::now();
        let question = input.question.trim().to_string();
        if question.is_empty() {
            return Err(AppError::invalid_input("问题不能为空。"));
        }
        if question.chars().count() > MAX_QUESTION_CHARS {
            return Err(AppError::invalid_input(format!(
                "问题过长：最多 {MAX_QUESTION_CHARS} 个字符。"
            )));
        }
        if input
            .selection
            .as_deref()
            .is_some_and(|selection| selection.chars().count() > MAX_SELECTION_CHARS)
        {
            return Err(AppError::invalid_input(format!(
                "选中文字过长：最多 {MAX_SELECTION_CHARS} 个字符。"
            )));
        }

        let policy_service = AiPolicyService::new(Arc::clone(&self.ctx));
        policy_service.ensure_enabled("ask_answer")?;
        let mut retrieval = self.retrieve(&input)?;
        let found_count = retrieval.evidence.len();
        let weak_count = retrieval
            .evidence
            .iter()
            .filter(|item| item.level == SourceLevel::E)
            .count();
        retrieval
            .evidence
            .retain(|item| item.level != SourceLevel::E);
        let stale_count = retrieval.evidence.iter().filter(|item| item.stale).count();
        let evidence_gap = if retrieval.evidence.is_empty() {
            Some(if weak_count > 0 {
                format!(
                    "找到 {weak_count} 条材料，但全部只是不可用线索（E 级），不能支撑事实性结论。"
                )
            } else {
                "当前作用域内没有找到可以回答这个问题的材料。".to_string()
            })
        } else if stale_count == retrieval.evidence.len() {
            Some(format!(
                "找到 {} 条材料，但全部可能已经过时，不能只靠这些材料给出事实性结论。",
                retrieval.evidence.len()
            ))
        } else {
            None
        };

        if let Some(gap) = evidence_gap {
            let answer = AskAnswer {
                question,
                text: format!(
                    "证据不足：{gap} 请补充相关笔记、核对时效、调整作用域或换一种问法。"
                ),
                citations: Vec::new(),
                provider: "template.ask".to_string(),
                tier: "template".to_string(),
                evidence_sufficient: false,
                evidence_note: gap,
                warnings: retrieval.warnings,
                channels: retrieval.channels,
                retrieved_count: found_count,
                took_ms: started.elapsed().as_millis() as u64,
            };
            policy_service.audit(
                "ask_answer",
                false,
                "ok",
                answer.took_ms,
                Some("INSUFFICIENT_EVIDENCE"),
            );
            return Ok(answer);
        }

        let context = conversation_context(&input);
        let selection = input
            .selection
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                format!("\n用户选中的原文（只用于理解问题，不能作为引用来源）：\n{value}\n")
            })
            .unwrap_or_default();
        let request = GenerationRequest {
            prompt: format!(
                "回答用户关于知识库的问题：{question}\n\
                 {context}{selection}\
                 只依据本轮给定材料作答；每条事实后必须用 [n] 标注本轮材料编号；\
                 不得引用未出现在本轮材料列表里的编号；历史回答不是事实来源；\
                 材料不足时明确写“证据不足”，不要猜测。"
            ),
            instructions: None,
            evidence: retrieval
                .evidence
                .iter()
                .map(|item| item.chunk.clone())
                .collect(),
            max_tokens: Some(ASK_MAX_OUTPUT_TOKENS),
            allow_remote: false,
        };

        let mut warnings = retrieval.warnings;
        if weak_count > 0 {
            warnings.push(format!(
                "已忽略 {weak_count} 条 E 级不可用线索；答案只使用剩余可用来源。"
            ));
        }
        if stale_count > 0 {
            warnings.push(format!(
                "{stale_count} 条来源可能已过时；回答这些部分时需要人工核对。"
            ));
        }
        let estimated_input_tokens = estimate_request_tokens(&request);
        let candidates = if estimated_input_tokens > MAX_ASK_INPUT_TOKENS {
            warnings.push(format!(
                "本次上下文预估 {estimated_input_tokens} tokens，超过单次问答输入上限 \
                 {MAX_ASK_INPUT_TOKENS} tokens；已跳过在线 / 本地模型，改用模板整理，\
                 未发起模型请求。请缩短问题、选中文字或减少追问历史。"
            ));
            Vec::new()
        } else {
            generation_candidates(
                &self.ctx,
                "ask_answer",
                input.confirm_outbound,
                &mut warnings,
            )
        };
        // 只要构造了在线档，本次就可能已经发出请求；即使随后失败降级，审计也必须如实标记。
        let outbound_attempted = candidates.iter().any(|(_, tier, _, _)| tier == "online");
        let chosen = select_generated_answer(
            candidates,
            &request,
            retrieval.evidence.len(),
            &mut warnings,
        );

        let (provider, tier, _outbound, text) = chosen.unwrap_or_else(|| {
            (
                "template.ask".to_string(),
                "template".to_string(),
                false,
                template_answer(&retrieval.evidence, &question),
            )
        });

        // 模板答案固定引用全部材料；模型答案只保留正文实际引用的材料。
        let cited_numbers = citation_numbers(&text, retrieval.evidence.len())?;
        let citations = build_citations(cited_numbers, &retrieval.evidence);

        let took_ms = started.elapsed().as_millis() as u64;
        let answer = AskAnswer {
            question,
            text,
            citations,
            provider,
            tier,
            evidence_sufficient: true,
            evidence_note: format!(
                "检索到 {} 条可引用材料；答案只采纳通过引用校验的来源。",
                retrieval.evidence.len()
            ),
            warnings,
            channels: retrieval.channels,
            retrieved_count: retrieval.evidence.len(),
            took_ms,
        };
        policy_service.audit(
            "ask_answer",
            outbound_attempted,
            "ok",
            took_ms,
            Some(answer.provider.as_str()),
        );
        Ok(answer)
    }

    /// 生成“存为笔记”的预览，并重新核对每条引用对应的原文仍然未变化。
    pub fn preview_answer_note(&self, input: AskSaveInput) -> Result<AskNotePreview> {
        self.prepare_answer_note(&input)
    }

    /// 用户确认后把答案存为普通 Markdown 笔记。
    ///
    /// 写入前再次执行完整预览校验，避免界面停留期间来源被外部修改。
    pub fn save_answer_note(&self, input: AskSaveInput) -> Result<AskSaveResult> {
        if !input.confirm {
            return Err(AppError::invalid_input(
                "保存答案前必须确认预览；本次没有写入任何文件。",
            ));
        }
        let preview = self.prepare_answer_note(&input)?;
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let detail = notes.create(CreateNoteInput {
            title: preview.title.clone(),
            category_id: preview.category_id.clone(),
            initial_content: Some(preview.content),
            tags: None,
            parent_id: None,
        })?;
        AiPolicyService::new(Arc::clone(&self.ctx)).audit(
            "ask_save_answer",
            false,
            "ok",
            input.answer.took_ms,
            Some(detail.note.id.as_str()),
        );
        Ok(AskSaveResult {
            note_id: detail.note.id,
            title: detail.note.title,
            file_path: detail.note.file_path,
            index_warning: None,
        })
    }

    fn prepare_answer_note(&self, input: &AskSaveInput) -> Result<AskNotePreview> {
        let answer = &input.answer;
        if answer.question.trim().is_empty() {
            return Err(AppError::invalid_input("答案缺少原始问题，不能存为笔记。"));
        }
        if answer.text.trim().is_empty() || answer.text.chars().count() > MAX_ANSWER_CHARS {
            return Err(AppError::invalid_input(format!(
                "答案正文为空或超过 {MAX_ANSWER_CHARS} 字符。"
            )));
        }
        if !answer.evidence_sufficient || answer.citations.is_empty() {
            return Err(AppError::invalid_input(
                "证据不足的回答不能存为结论笔记；请补充材料后重新提问。",
            ));
        }

        let notes = NoteService::new(Arc::clone(&self.ctx));
        let mut sources: Vec<AskSourcePreview> = Vec::new();
        let mut source_ids: HashSet<String> = HashSet::new();
        for citation in &answer.citations {
            let numeric = citation
                .citation_id
                .strip_prefix('C')
                .filter(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
                .ok_or_else(|| AppError::invalid_input("答案包含无法识别的引用编号。"))?;
            if !answer.text.contains(&format!("[{}]", citation.citation_id))
                && !answer.text.contains(&format!("[{numeric}]"))
            {
                return Err(AppError::invalid_input(format!(
                    "答案正文没有使用引用 {}，预览已拒绝。",
                    citation.citation_id
                )));
            }

            let detail = notes.get(&citation.note_id).map_err(|_| {
                AppError::new(
                    ErrorCode::NoteConflict,
                    format!(
                        "来源笔记「{}」已不存在或已被移动；请重新提问生成答案。",
                        citation.title
                    ),
                )
            })?;
            let chars: Vec<char> = detail.content.chars().collect();
            if citation.char_start < 0
                || citation.char_end <= citation.char_start
                || citation.char_end as usize > chars.len()
            {
                return Err(AppError::new(
                    ErrorCode::NoteConflict,
                    format!(
                        "来源笔记「{}」的引用区间已失效，请重新提问。",
                        citation.title
                    ),
                ));
            }
            let source = chars[citation.char_start as usize..citation.char_end as usize]
                .iter()
                .collect::<String>();
            if truncate_chars(source.trim(), MAX_EVIDENCE_CHARS) != citation.snippet.trim() {
                return Err(AppError::new(
                    ErrorCode::NoteConflict,
                    format!(
                        "来源笔记「{}」在生成答案后被修改；为避免保存过期引用，请重新提问。",
                        citation.title
                    ),
                ));
            }
            if source_ids.insert(citation.note_id.clone()) {
                sources.push(AskSourcePreview {
                    citation_id: citation.citation_id.clone(),
                    note_id: citation.note_id.clone(),
                    title: detail.note.title,
                    heading: citation.heading.clone(),
                });
            }
        }

        let title = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_answer_title(&answer.question));
        let mut warnings: Vec<String> = Vec::new();
        if answer.tier == "template" {
            warnings.push("当前答案来自模板档；保存后仍需人工核对与补充。".to_string());
        }
        let duplicate = notes.list(NoteListQuery {
            keyword: Some(title.clone()),
            page: Some(1),
            page_size: Some(50),
            ..Default::default()
        })?;
        if duplicate
            .items
            .iter()
            .any(|note| note.title.eq_ignore_ascii_case(&title))
        {
            warnings.push("已有同标题笔记；本次会新建另一篇，不会覆盖原笔记。".to_string());
        }
        Ok(AskNotePreview {
            title: title.clone(),
            content: build_answer_note_content(answer, &sources),
            category_id: input.category_id.clone(),
            sources,
            warnings,
        })
    }

    fn retrieve(&self, input: &AskQuestionInput) -> Result<Retrieval> {
        match input.scope {
            AskScope::Note => {
                let searched = self.retrieve_search(input)?;
                if searched.evidence.is_empty() {
                    self.retrieve_note_context(input.scope_id.as_deref())
                } else {
                    Ok(searched)
                }
            }
            AskScope::Category | AskScope::Tag | AskScope::All => self.retrieve_search(input),
        }
    }

    /// 当前笔记里完全没有词面命中时，仍把它作为明确上下文：采用前若干个正文块，
    /// 让“这篇笔记讲了什么”这类元问题在模板档也有材料可核对。
    fn retrieve_note_context(&self, note_id: Option<&str>) -> Result<Retrieval> {
        let note_id =
            note_id.ok_or_else(|| AppError::invalid_input("当前笔记作用域缺少笔记 id。"))?;
        let detail = NoteService::new(Arc::clone(&self.ctx)).get(note_id)?;
        let grade = grade_local_note(
            detail.note.source_reliability,
            detail.note.source_type.as_deref(),
        );
        let stale = detail
            .note
            .valid_until
            .as_deref()
            .and_then(|valid_until| is_stale(Some(valid_until), 0))
            .unwrap_or(false);
        let chunks = chunk_markdown(&detail.content);
        let source_chars: Vec<char> = detail.content.chars().collect();
        let evidence = chunks
            .into_iter()
            .take(MAX_EVIDENCE)
            .filter_map(|chunk| {
                let start = chunk.char_start.min(source_chars.len());
                let end = chunk.char_end.min(source_chars.len());
                if start >= end {
                    return None;
                }
                let text = source_chars[start..end].iter().collect::<String>();
                Some(Evidence {
                    chunk: RetrievedChunk {
                        note_id: detail.note.id.clone(),
                        chunk_index: chunk.index as i64,
                        heading_path: split_heading(&chunk.heading),
                        snippet: truncate_chars(text.trim(), MAX_EVIDENCE_CHARS),
                        char_start: start as i64,
                        char_end: end as i64,
                        score: 1.0,
                        origin: RetrievalOrigin::Bm25,
                    },
                    title: detail.note.title.clone(),
                    level: grade.level,
                    stale,
                })
            })
            .collect::<Vec<_>>();
        let count = evidence.len();
        Ok(Retrieval {
            evidence,
            channels: vec![RetrievalChannelStatus {
                id: "note".to_string(),
                available: true,
                note: format!("当前笔记按标题分块，采用前 {count} 块作为材料"),
            }],
            warnings: Vec::new(),
        })
    }

    /// 分类 / 标签 / 全库作用域走现有 FTS5。自然语言问题先做可解释的规则扩展，
    /// 再按查询优先级合并结果——避免整句双字短语 AND 导致零命中。
    fn retrieve_search(&self, input: &AskQuestionInput) -> Result<Retrieval> {
        let mut scope_note_id = None;
        let (category_id, tag_id) =
            match input.scope {
                AskScope::Note => {
                    scope_note_id =
                        Some(input.scope_id.clone().ok_or_else(|| {
                            AppError::invalid_input("当前笔记作用域缺少笔记 id。")
                        })?);
                    (None, None)
                }
                AskScope::Category => {
                    (
                        Some(input.scope_id.clone().ok_or_else(|| {
                            AppError::invalid_input("当前分类作用域缺少分类 id。")
                        })?),
                        None,
                    )
                }
                AskScope::Tag => {
                    (
                        None,
                        Some(input.scope_id.clone().ok_or_else(|| {
                            AppError::invalid_input("当前标签作用域缺少标签 id。")
                        })?),
                    )
                }
                AskScope::All => (None, None),
            };
        let expanded = ask_queries(&retrieval_text(input));
        let search = SearchService::new(Arc::clone(&self.ctx));
        let mut evidence: Vec<Evidence> = Vec::new();
        let mut seen: HashSet<(String, i64)> = HashSet::new();
        let mut warnings = Vec::new();
        for query in &expanded {
            let response = search.search(SearchQuery {
                keyword: query.clone(),
                category_id: category_id.clone(),
                tag_id: tag_id.clone(),
                scope_note_id: scope_note_id.clone(),
                page: Some(1),
                page_size: Some((MAX_EVIDENCE * 2) as i64),
                ..Default::default()
            })?;
            if response.index_stale {
                let warning = "搜索索引可能已落后于文件，引用区间可能暂时不准确。".to_string();
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            for hit in response.hits {
                let Some(item) = self.evidence_from_hit(hit) else {
                    continue;
                };
                let key = (item.chunk.note_id.clone(), item.chunk.chunk_index);
                if seen.insert(key) {
                    evidence.push(item);
                }
                if evidence.len() >= MAX_EVIDENCE {
                    break;
                }
            }
            if evidence.len() >= MAX_EVIDENCE {
                break;
            }
        }
        let count = evidence.len();
        Ok(Retrieval {
            evidence,
            channels: vec![RetrievalChannelStatus {
                id: "bm25".to_string(),
                available: true,
                note: format!("用 {} 个关键词召回 {count} 条材料", expanded.len()),
            }],
            warnings,
        })
    }

    /// 把 FTS 的命中位置映射成“整块”引用区间。
    ///
    /// `SearchHit.location` 指向关键词本身；问答引用需要段落锚点，所以这里读分块表，
    /// 找到同一 `chunk_index` 的完整区间。定位不到时退回正文第一块，并保留真实原文区间。
    fn evidence_from_hit(&self, hit: SearchHit) -> Option<Evidence> {
        let detail = NoteService::new(Arc::clone(&self.ctx))
            .get(&hit.note.id)
            .ok()?;
        let grade = grade_local_note(
            detail.note.source_reliability,
            detail.note.source_type.as_deref(),
        );
        let stale = detail
            .note
            .valid_until
            .as_deref()
            .and_then(|valid_until| is_stale(Some(valid_until), 0))
            .unwrap_or(false);
        let source_chars: Vec<char> = detail.content.chars().collect();
        let stored = self
            .ctx
            .db
            .with_conn(|conn| SqliteChunkRepository.list(conn, &hit.note.id))
            .ok()?;
        let chunk_index = hit.location.as_ref().map(|location| location.chunk_index);
        let located: Option<(i64, String, i64, i64)> = chunk_index.and_then(|index| {
            stored
                .iter()
                .find(|chunk| chunk.index == index)
                .map(|chunk| {
                    (
                        chunk.index,
                        chunk.heading.clone(),
                        chunk.char_start,
                        chunk.char_end,
                    )
                })
        });
        let fallback: Option<(i64, String, i64, i64)> = stored.first().map(|chunk| {
            (
                chunk.index,
                chunk.heading.clone(),
                chunk.char_start,
                chunk.char_end,
            )
        });
        let (index, heading, start, end) = located.or(fallback)?;
        let start = start.max(0) as usize;
        let end = end.max(0).min(source_chars.len() as i64) as usize;
        if start >= end {
            return None;
        }
        let text = source_chars[start..end].iter().collect::<String>();
        Some(Evidence {
            chunk: RetrievedChunk {
                note_id: hit.note.id,
                chunk_index: index,
                heading_path: split_heading(&heading),
                snippet: truncate_chars(text.trim(), MAX_EVIDENCE_CHARS),
                char_start: start as i64,
                char_end: end as i64,
                score: (-hit.score) as f32,
                origin: RetrievalOrigin::Bm25,
            },
            title: hit.note.title,
            level: grade.level,
            stale,
        })
    }
}

fn split_heading(heading: &str) -> Vec<String> {
    heading
        .split(" > ")
        .filter(|part| !part.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// 多轮上下文只进提示词；每轮检索仍使用当前问题 + 最近问题 + 选中文字重新召回。
fn conversation_context(input: &AskQuestionInput) -> String {
    let turns = input
        .history
        .iter()
        .rev()
        .take(MAX_HISTORY_TURNS)
        .collect::<Vec<_>>();
    if turns.is_empty() {
        return String::new();
    }
    let mut context =
        String::from("\n对话上下文（只用于理解指代；历史回答不是事实来源，也不能用于引用）：\n");
    for turn in turns.into_iter().rev() {
        context.push_str(&format!(
            "用户：{}\n助手：{}\n",
            truncate_chars(turn.question.trim(), 500),
            truncate_chars(turn.answer.trim(), MAX_HISTORY_ANSWER_CHARS)
        ));
    }
    context.push('\n');
    context
}

fn retrieval_text(input: &AskQuestionInput) -> String {
    let mut parts: Vec<String> = input
        .history
        .iter()
        .rev()
        .take(2)
        .map(|turn| turn.question.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    parts.reverse();
    if let Some(selection) = input
        .selection
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(selection.to_string());
    }
    parts.push(input.question.trim().to_string());
    parts.join("\n")
}

fn default_answer_title(question: &str) -> String {
    let title = truncate_chars(question.trim().lines().next().unwrap_or("知识库问答"), 48)
        .trim_matches(|ch: char| char::is_whitespace(ch) || "#>*-`".contains(ch))
        .trim()
        .to_string();
    if title.is_empty() {
        "知识库问答".to_string()
    } else {
        title
    }
}

fn build_answer_note_content(answer: &AskAnswer, sources: &[AskSourcePreview]) -> String {
    let mut content = String::new();
    content.push_str(&format!("> 问题：{}\n", answer.question.trim()));
    content.push_str(&format!(
        "> 生成档位：{}（{}）\n> 证据：{}\n\n",
        answer.provider, answer.tier, answer.evidence_note
    ));
    content.push_str(answer.text.trim());
    content.push_str("\n\n## 来源\n\n");
    for source in sources {
        let heading = if source.heading.trim().is_empty() {
            String::new()
        } else {
            format!(" · {}", source.heading.trim())
        };
        content.push_str(&format!(
            "- {} · [[{}]]{}\n",
            source.citation_id, source.title, heading
        ));
    }
    content.push_str("\n<details>\n<summary>原文摘录</summary>\n\n");
    for citation in &answer.citations {
        content.push_str(&format!(
            "### {}\n\n{}\n\n",
            citation.citation_id, citation.snippet
        ));
    }
    content.push_str("</details>\n");
    content
}

/// 问答检索的规则扩展：先复用通用课题扩展，再补中文双字词。
///
/// 通用扩展的第一条是整句；中文 FTS 会把整句拆成双字 AND，像
/// 「之前关于索引设计的结论有哪些」这种问句通常一个都匹配不上。这里从中文连续段
/// 提取双字词并去掉问句套话，做成多条独立查询后再合并，仍然不发明同义词。
fn ask_queries(question: &str) -> Vec<String> {
    let mut queries: Vec<String> = query_expansion::expand(question, &[], 4)
        .into_iter()
        .map(|item| item.text)
        .collect();
    let mut seen: HashSet<String> = queries.iter().map(|item| item.to_lowercase()).collect();
    for run in question
        .split(|ch: char| !crate::core::text::is_cjk(ch))
        .filter(|run| run.chars().count() >= 2)
    {
        let chars: Vec<char> = run.chars().collect();
        for window in chars.windows(2) {
            let term: String = window.iter().collect();
            if QUESTION_STOP_BIGRAMS.contains(&term.as_str()) {
                continue;
            }
            if seen.insert(term.to_lowercase()) {
                queries.push(term);
            }
            if queries.len() >= MAX_ASK_QUERIES {
                return queries;
            }
        }
    }
    queries.truncate(MAX_ASK_QUERIES);
    queries
}

fn estimate_text_tokens(text: &str) -> usize {
    let mut cjk = 0usize;
    let mut other = 0usize;
    for ch in text.chars().filter(|ch| !ch.is_whitespace()) {
        if crate::core::text::is_cjk(ch) {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    cjk + other.div_ceil(4)
}

fn estimate_request_tokens(request: &GenerationRequest) -> usize {
    let evidence = request
        .evidence
        .iter()
        .map(|item| {
            estimate_text_tokens(&item.snippet)
                + item
                    .heading_path
                    .iter()
                    .map(|heading| estimate_text_tokens(heading))
                    .sum::<usize>()
                + 16
        })
        .sum::<usize>();
    estimate_text_tokens(&request.prompt)
        + request
            .instructions
            .as_deref()
            .map(estimate_text_tokens)
            .unwrap_or(0)
        + evidence
        + 32
}

fn template_answer(evidence: &[Evidence], question: &str) -> String {
    let query_terms = ask_queries(question)
        .into_iter()
        .filter(|term| term.chars().count() >= 2)
        .collect::<Vec<_>>();
    let scored = evidence
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let score = query_terms
                .iter()
                .filter(|term| item.chunk.snippet.contains(term.as_str()))
                .count();
            (index, item, score)
        })
        .collect::<Vec<_>>();
    let max_score = scored.iter().map(|(_, _, score)| *score).max().unwrap_or(0);
    let selected = if max_score == 0 {
        scored
    } else {
        scored
            .into_iter()
            .filter(|(_, _, score)| *score == max_score)
            .take(3)
            .collect()
    };
    let mut lines = vec![
        "**证据整理（模板档，未接模型）**".to_string(),
        String::new(),
        "当前环境没有可用的生成模型；以下只整理与问题最相关的材料，供人工核对：".to_string(),
        String::new(),
    ];
    for (index, item, _) in selected {
        let heading = if item.chunk.heading_path.is_empty() {
            String::new()
        } else {
            format!("（{}）", item.chunk.heading_path.join(" > "))
        };
        lines.push(format!(
            "{}. {}{}\n   [{}]",
            index + 1,
            truncate_chars(&item.chunk.snippet, 240),
            heading,
            index + 1
        ));
    }
    lines.push(String::new());
    lines.push("注意：模板档不推断结论；请根据上述原文片段自行判断。".to_string());
    lines.join("\n")
}

/// 严格解析 `[n]` / `[Cn]`。
///
/// 返回按首次出现顺序去重的 1-based 编号。没有引用或出现未知编号都返回错误——
/// 上层据此丢弃整份模型回答并降级，避免把编造来源混入界面。
fn citation_numbers(text: &str, evidence_count: usize) -> Result<Vec<usize>> {
    let pattern = Regex::new(r"\[(?:[Cc])?(\d{1,3})\]")
        .map_err(|error| AppError::internal(format!("引用解析规则初始化失败：{error}")))?;
    let mut seen = HashSet::new();
    let mut numbers = Vec::new();
    for captures in pattern.captures_iter(text) {
        let Some(raw) = captures.get(1).map(|entry| entry.as_str()) else {
            continue;
        };
        let number = raw.parse::<usize>().map_err(|_| {
            AppError::new(
                ErrorCode::InvalidInput,
                format!("无法解析引用编号 [{raw}]。"),
            )
        })?;
        if number == 0 || number > evidence_count {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!("回答引用了未提供的材料编号 [{number}]。"),
            ));
        }
        if seen.insert(number) {
            numbers.push(number);
        }
    }
    if numbers.is_empty() {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "回答没有引用任何本次提供的材料。",
        ));
    }
    Ok(numbers)
}

fn build_citations(numbers: Vec<usize>, evidence: &[Evidence]) -> Vec<AskCitation> {
    numbers
        .into_iter()
        .filter_map(|number| {
            evidence
                .get(number.saturating_sub(1))
                .map(|item| AskCitation {
                    // 引用编号必须与正文里的 [n] 保持一致，不能按返回顺序重新编号。
                    citation_id: format!("C{number}"),
                    note_id: item.chunk.note_id.clone(),
                    title: item.title.clone(),
                    chunk_index: item.chunk.chunk_index,
                    heading: item.chunk.heading_path.join(" > "),
                    snippet: item.chunk.snippet.clone(),
                    char_start: item.chunk.char_start,
                    char_end: item.chunk.char_end,
                })
        })
        .collect()
}

/// 逐档尝试生成并校验引用。模型输出未知编号时**整份丢弃**，继续下一档。
fn select_generated_answer(
    candidates: Vec<(String, String, bool, Arc<dyn GenerativeModel>)>,
    request: &GenerationRequest,
    evidence_count: usize,
    warnings: &mut Vec<String>,
) -> Option<(String, String, bool, String)> {
    for (provider_id, tier, is_local, model) in candidates {
        let mut outbound_request = request.clone();
        outbound_request.allow_remote = !is_local;
        match model.complete(&outbound_request) {
            Ok(response) => match citation_numbers(&response.text, evidence_count) {
                Ok(_) => return Some((provider_id, tier, !is_local, response.text)),
                Err(error) => warnings.push(format!(
                    "{provider_id} 的回答未通过引用校验（{}），已丢弃并降级。",
                    error.message
                )),
            },
            Err(error) => warnings.push(format!(
                "{provider_id} 生成失败（{}），已降级。",
                error.message
            )),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::GenerationResponse;
    use crate::core::error::Result as AppResult;

    struct FakeModel {
        text: &'static str,
    }

    impl GenerativeModel for FakeModel {
        fn id(&self) -> &'static str {
            "test.fake"
        }

        fn is_local(&self) -> bool {
            true
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        fn complete(&self, _request: &GenerationRequest) -> AppResult<GenerationResponse> {
            Ok(GenerationResponse {
                text: self.text.to_string(),
                model: "test.fake".to_string(),
                duration_ms: 1,
                usage: None,
            })
        }
    }

    #[test]
    fn citation_parser_only_accepts_material_numbers_from_this_request() {
        assert_eq!(
            citation_numbers("结论一 [1]，结论二 [C2]。", 2).unwrap(),
            vec![1, 2]
        );
        assert_eq!(
            citation_numbers("[C1] 重复 [1] [2]", 2).unwrap(),
            vec![1, 2]
        );
        assert!(citation_numbers("没有引用", 2).is_err());
        assert!(
            citation_numbers("编造来源 [3]", 2).is_err(),
            "越界编号必须整份拒绝"
        );
        assert!(citation_numbers("零号 [0]", 2).is_err());
    }

    #[test]
    fn template_answer_always_has_valid_citations() {
        let evidence = vec![Evidence {
            chunk: RetrievedChunk {
                note_id: "n1".to_string(),
                chunk_index: 0,
                heading_path: vec!["索引".to_string()],
                snippet: "SQLite FTS5 使用倒排索引。".to_string(),
                char_start: 0,
                char_end: 18,
                score: 1.0,
                origin: RetrievalOrigin::Bm25,
            },
            title: "索引设计".to_string(),
            level: SourceLevel::D,
            stale: false,
        }];
        let text = template_answer(&evidence, "SQLite FTS5 索引有什么结论？");
        assert_eq!(citation_numbers(&text, evidence.len()).unwrap(), vec![1]);
    }

    #[test]
    fn citation_ids_stay_aligned_with_the_model_text_numbering() {
        let evidence = vec![
            Evidence {
                chunk: RetrievedChunk {
                    note_id: "n1".to_string(),
                    chunk_index: 0,
                    heading_path: Vec::new(),
                    snippet: "材料一".to_string(),
                    char_start: 0,
                    char_end: 3,
                    score: 1.0,
                    origin: RetrievalOrigin::Bm25,
                },
                title: "一".to_string(),
                level: SourceLevel::D,
                stale: false,
            },
            Evidence {
                chunk: RetrievedChunk {
                    note_id: "n2".to_string(),
                    chunk_index: 0,
                    heading_path: Vec::new(),
                    snippet: "材料二".to_string(),
                    char_start: 0,
                    char_end: 3,
                    score: 1.0,
                    origin: RetrievalOrigin::Bm25,
                },
                title: "二".to_string(),
                level: SourceLevel::D,
                stale: false,
            },
        ];
        let citations = build_citations(vec![2], &evidence);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].citation_id, "C2");
        assert_eq!(citations[0].note_id, "n2");
    }

    #[test]
    fn fabricated_citation_discards_the_model_and_falls_back_to_the_next_tier() {
        let request = GenerationRequest {
            prompt: "回答".to_string(),
            instructions: None,
            evidence: Vec::new(),
            max_tokens: None,
            allow_remote: false,
        };
        let mut warnings = Vec::new();
        let selected = select_generated_answer(
            vec![
                (
                    "fake.bad".to_string(),
                    "local".to_string(),
                    true,
                    Arc::new(FakeModel {
                        text: "编造来源 [9]",
                    }),
                ),
                (
                    "fake.good".to_string(),
                    "local".to_string(),
                    true,
                    Arc::new(FakeModel {
                        text: "有依据的结论 [1]",
                    }),
                ),
            ],
            &request,
            2,
            &mut warnings,
        )
        .expect("第二档应被采纳");
        assert_eq!(selected.0, "fake.good");
        assert!(selected.3.contains("[1]"));
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("未通过引用校验")),
            "降级原因必须可见：{warnings:?}"
        );
    }

    #[test]
    fn oversized_ask_context_is_estimated_before_generation() {
        let request = GenerationRequest {
            prompt: "问".repeat(MAX_ASK_INPUT_TOKENS + 1),
            instructions: None,
            evidence: Vec::new(),
            max_tokens: Some(ASK_MAX_OUTPUT_TOKENS),
            allow_remote: false,
        };
        assert!(estimate_request_tokens(&request) > MAX_ASK_INPUT_TOKENS);
    }
}
