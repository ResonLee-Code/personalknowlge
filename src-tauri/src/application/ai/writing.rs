//! Writing 2.0：按引用生成草稿，并由服务端在最终确认时重新校验来源与目标哈希。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::application::context::WorkspaceContext;
use crate::application::note_service::NoteService;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::text::{markdown_to_plain_text, truncate_chars};
use crate::domain::ai::{AskCitation, AskQuestionInput, AskScope};
use crate::domain::note::SaveNoteInput;
use crate::domain::research::SourceLevel;
use crate::domain::writing::{
    WritingApplyInput, WritingApplyPreview, WritingApplyResult, WritingDraft, WritingMode,
    WritingRequest, WritingSegment, WritingSummarySaveInput,
};

use super::ask::{
    build_citations, citation_numbers, estimate_request_tokens, AskService, Evidence,
};
use super::model_gateway::generation_candidates;
use super::policy::AiPolicyService;
use super::traits::{GenerationRequest, GenerativeModel};

const MAX_SELECTION_CHARS: usize = 4_000;
const MAX_GENERATED_CHARS: usize = 50_000;
const MAX_INPUT_TOKENS: usize = 7_000;
const OUTPUT_TOKENS: u32 = 900;
const MAX_SOURCE_CHARS: usize = 700;

pub struct WritingService {
    ctx: Arc<WorkspaceContext>,
}

impl WritingService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn draft(&self, input: WritingRequest) -> Result<WritingDraft> {
        let started = Instant::now();
        let detail = NoteService::new(Arc::clone(&self.ctx)).get(&input.note_id)?;
        let policy_service = AiPolicyService::new(Arc::clone(&self.ctx));
        policy_service.ensure_enabled("writing_generate")?;
        let mode = input.mode.unwrap_or_default();
        let chars: Vec<char> = detail.content.chars().collect();
        let target_start = input
            .selection_start
            .unwrap_or(
                if matches!(mode, WritingMode::Outline | WritingMode::Continue) {
                    chars.len() as i64
                } else {
                    0
                },
            )
            .clamp(0, chars.len() as i64);
        let target_end = input
            .selection_end
            .unwrap_or(chars.len() as i64)
            .clamp(target_start, chars.len() as i64);
        let selected = input
            .selection
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                (target_end > target_start).then(|| {
                    chars[target_start as usize..target_end as usize]
                        .iter()
                        .collect::<String>()
                })
            })
            .map(|value| truncate_chars(&value, MAX_SELECTION_CHARS));
        let instruction = input.instruction.trim();
        let retrieval_query = format!(
            "{} {}\n{}",
            mode.label(),
            instruction,
            selected.as_deref().unwrap_or_default()
        );
        let ask = AskService::new(Arc::clone(&self.ctx));
        let ask_input = AskQuestionInput {
            question: retrieval_query,
            scope: AskScope::Note,
            scope_id: Some(input.note_id.clone()),
            selection: selected.clone(),
            history: Vec::new(),
            confirm_outbound: input.confirm_outbound,
        };
        let mut retrieval = ask.retrieve(&ask_input)?;
        // 当前笔记是写作上下文，但相关笔记也应当能成为引用来源。两次检索都走
        // Ask 的同一套作用域与证据分级，不引入第二套召回规则。
        if let Ok(related) = ask.retrieve(&AskQuestionInput {
            scope: AskScope::All,
            scope_id: None,
            ..ask_input
        }) {
            let mut seen: HashSet<(String, i64)> = retrieval
                .evidence
                .iter()
                .map(|item| (item.chunk.note_id.clone(), item.chunk.chunk_index))
                .collect();
            for item in related.evidence {
                if seen.insert((item.chunk.note_id.clone(), item.chunk.chunk_index)) {
                    retrieval.evidence.push(item);
                }
                if retrieval.evidence.len() >= 8 {
                    break;
                }
            }
            retrieval.warnings.extend(related.warnings);
        }
        let weak = retrieval
            .evidence
            .iter()
            .filter(|item| item.level == SourceLevel::E)
            .count();
        retrieval
            .evidence
            .retain(|item| item.level != SourceLevel::E);
        if retrieval.evidence.is_empty() {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                if weak > 0 {
                    "当前笔记只有 E 级不可用材料，不能生成带可靠引用的写作草稿。"
                } else {
                    "当前笔记没有可用于生成引用的材料；请先补充正文或选择包含事实的段落。"
                },
            ));
        }
        let stale = retrieval.evidence.iter().filter(|item| item.stale).count();
        let mut warnings = retrieval.warnings;
        if weak > 0 {
            warnings.push(format!("已忽略 {weak} 条 E 级不可用线索。"));
        }
        if stale > 0 {
            warnings.push(format!("{stale} 条材料可能已过时，写入前需要人工核对。"));
        }

        let prompt = generation_prompt(mode, instruction, selected.as_deref());
        let request = GenerationRequest {
            prompt,
            instructions: Some(
                "只输出正文草稿。每个独立段落或每个列表项都必须以至少一个 [n] 引用结尾；\
                 只能引用本轮材料编号，不得编造编号；不要输出解释、前言或来源清单。"
                    .to_string(),
            ),
            evidence: retrieval
                .evidence
                .iter()
                .map(|item| item.chunk.clone())
                .collect(),
            max_tokens: Some(OUTPUT_TOKENS),
            allow_remote: false,
        };
        let input_tokens = estimate_request_tokens(&request);
        let candidates = if input_tokens > MAX_INPUT_TOKENS {
            warnings.push(format!(
                "本次材料预估 {input_tokens} tokens，超过输入预算 {MAX_INPUT_TOKENS}；\
                 未调用模型，只生成可核对的材料提纲。"
            ));
            Vec::new()
        } else {
            generation_candidates(
                &self.ctx,
                "writing_generate",
                input.confirm_outbound,
                &mut warnings,
            )
        };
        let outbound_attempted = candidates.iter().any(|(_, tier, _, _)| tier == "online");
        let generated = select_draft(candidates, &request, &retrieval.evidence, &mut warnings);
        let (provider, tier, text) = generated.unwrap_or_else(|| {
            (
                "template.writing".to_string(),
                "template".to_string(),
                template_draft(mode, &retrieval.evidence),
            )
        });
        let segments = parse_segments(&text, &retrieval.evidence)?;
        if segments.is_empty() {
            return Err(AppError::invalid_input(
                "生成草稿为空，未产生任何可接受段落。",
            ));
        }
        let citations = unique_citations(&segments);
        let took_ms = started.elapsed().as_millis() as u64;
        policy_service.audit(
            "writing_generate",
            outbound_attempted,
            "ok",
            took_ms,
            Some(provider.as_str()),
        );
        Ok(WritingDraft {
            note_id: detail.note.id,
            mode,
            text: segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n"),
            segments,
            citations,
            provider,
            tier,
            evidence_sufficient: true,
            warnings,
            base_content_hash: detail.note.content_hash.unwrap_or_default(),
            target_start,
            target_end,
            input_tokens,
            took_ms,
        })
    }

    pub fn apply_preview(&self, input: &WritingApplyInput) -> Result<WritingApplyPreview> {
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let detail = notes.get(&input.note_id)?;
        if input.base_content_hash.trim().is_empty()
            || detail.note.content_hash.as_deref() != Some(input.base_content_hash.as_str())
        {
            return Err(AppError::new(
                ErrorCode::NoteConflict,
                "目标笔记在草稿生成后已被修改；请重新生成写作建议。",
            ));
        }
        if input.accepted_segments.is_empty() {
            return Err(AppError::invalid_input(
                "没有接受任何生成段落，本次不会写入笔记。",
            ));
        }
        let chars: Vec<char> = detail.content.chars().collect();
        let start = input.target_start.clamp(0, chars.len() as i64) as usize;
        let end = input.target_end.clamp(start as i64, chars.len() as i64) as usize;
        let (insertion, citations) =
            validate_and_join_segments(&notes, &input.accepted_segments, MAX_GENERATED_CHARS)?;
        let mut content: String = chars[..start]
            .iter()
            .chain(insertion.chars().collect::<Vec<_>>().iter())
            .chain(chars[end..].iter())
            .collect();
        append_source_section(&mut content, &citations);
        let added_chars = insertion.chars().count();
        let removed_chars = end.saturating_sub(start);
        Ok(WritingApplyPreview {
            note_id: input.note_id.clone(),
            content,
            added_chars,
            removed_chars,
            accepted_segments: input.accepted_segments.len(),
            citations,
            warnings: if input.mode == WritingMode::Continue {
                vec!["续写按插入位置拼接；请确认没有重复原文。".to_string()]
            } else {
                Vec::new()
            },
        })
    }

    pub fn apply(&self, input: WritingApplyInput) -> Result<WritingApplyResult> {
        if !input.confirm {
            return Err(AppError::invalid_input(
                "写入写作草稿前必须确认 diff；本次没有修改笔记。",
            ));
        }
        let preview = self.apply_preview(&input)?;
        let detail = NoteService::new(Arc::clone(&self.ctx)).save(SaveNoteInput {
            id: input.note_id.clone(),
            content: preview.content,
            base_content_hash: Some(input.base_content_hash),
            title: None,
            category_id: None,
            tags: None,
        })?;
        AiPolicyService::new(Arc::clone(&self.ctx)).audit(
            "writing_apply",
            false,
            "ok",
            preview.added_chars as u64,
            Some(&format!(
                "segments={} sources={}",
                preview.accepted_segments,
                preview.citations.len()
            )),
        );
        let saved = NoteService::new(Arc::clone(&self.ctx)).get(&detail.note.id)?;
        Ok(WritingApplyResult {
            detail: saved,
            accepted_segments: preview.accepted_segments,
            source_count: preview.citations.len(),
        })
    }

    pub fn save_summary(&self, input: WritingSummarySaveInput) -> Result<WritingApplyResult> {
        if !input.confirm {
            return Err(AppError::invalid_input(
                "新建摘要笔记前必须确认预览；本次没有写入。",
            ));
        }
        let title = input.title.trim();
        if title.is_empty() || title.chars().count() > 120 {
            return Err(AppError::invalid_input("摘要笔记标题为空或过长。"));
        }
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let (insertion, citations) =
            validate_and_join_segments(&notes, &input.segments, MAX_GENERATED_CHARS)?;
        let mut content = insertion;
        append_source_section(&mut content, &citations);
        let detail = notes.create(crate::domain::note::CreateNoteInput {
            title: title.to_string(),
            category_id: None,
            initial_content: Some(content),
            tags: Some(vec!["图谱摘要".to_string()]),
            parent_id: None,
        })?;
        AiPolicyService::new(Arc::clone(&self.ctx)).audit(
            "writing_cluster_summary",
            false,
            "ok",
            input.segments.len() as u64,
            Some(&format!(
                "segments={} sources={}",
                input.segments.len(),
                citations.len()
            )),
        );
        Ok(WritingApplyResult {
            accepted_segments: input.segments.len(),
            source_count: citations.len(),
            detail,
        })
    }
}

fn generation_prompt(mode: WritingMode, instruction: &str, selected: Option<&str>) -> String {
    let task = match mode {
        WritingMode::Outline => {
            "根据材料生成可执行的 Markdown 大纲；每个列表项写成一行、独立可接受，并附引用。"
        }
        WritingMode::Continue => "承接当前笔记继续写，避免重复已写内容；每个正文段落都附引用。",
        WritingMode::Rewrite => "在不改变事实含义的前提下重写选中文字；每个输出段落都附引用。",
        WritingMode::Shorten => "压缩选中文字，只保留材料能够支持的信息；每个输出段落都附引用。",
        WritingMode::Expand => {
            "扩写选中文字，补充材料中的解释和例子，不得补充无来源事实；每个输出段落都附引用。"
        }
    };
    format!(
        "{task}\n用户要求：{}\n选中文字：{}\n",
        if instruction.is_empty() {
            "（无额外要求）"
        } else {
            instruction
        },
        selected.unwrap_or("（未选择，按当前笔记整体处理）")
    )
}

fn select_draft(
    candidates: Vec<(String, String, bool, Arc<dyn GenerativeModel>)>,
    request: &GenerationRequest,
    evidence: &[Evidence],
    warnings: &mut Vec<String>,
) -> Option<(String, String, String)> {
    for (provider_id, tier, is_local, model) in candidates {
        let mut candidate_request = request.clone();
        candidate_request.allow_remote = !is_local;
        match model.complete(&candidate_request) {
            Ok(response) => match parse_segments(&response.text, evidence) {
                Ok(segments) if !segments.is_empty() => {
                    return Some((
                        provider_id,
                        tier,
                        segments
                            .iter()
                            .map(|segment| segment.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n\n"),
                    ))
                }
                Ok(_) => warnings.push(format!("{provider_id} 返回空草稿，已降级。")),
                Err(error) => warnings.push(format!(
                    "{provider_id} 的草稿未通过逐段引用校验（{}），已丢弃。",
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

fn parse_segments(text: &str, evidence: &[Evidence]) -> Result<Vec<WritingSegment>> {
    let mut raw_segments = Vec::new();
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for block in normalized.split("\n\n") {
        let lines: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        if lines.iter().any(|line| {
            line.starts_with("- ")
                || line.starts_with("* ")
                || line.split_once('.').is_some_and(|(prefix, _)| {
                    !prefix.is_empty() && prefix.chars().all(|ch| ch.is_ascii_digit())
                })
        }) {
            let mut current = String::new();
            for line in lines {
                let starts_item = line.starts_with("- ")
                    || line.starts_with("* ")
                    || line.split_once('.').is_some_and(|(prefix, _)| {
                        !prefix.is_empty() && prefix.chars().all(|ch| ch.is_ascii_digit())
                    });
                if starts_item {
                    if !current.is_empty() {
                        raw_segments.push(current);
                    }
                    current = line.to_string();
                } else if current.is_empty() {
                    current = line.to_string();
                } else {
                    current.push(' ');
                    current.push_str(line);
                }
            }
            if !current.is_empty() {
                raw_segments.push(current);
            }
        } else {
            raw_segments.push(lines.join("\n"));
        }
    }
    let mut segments = Vec::new();
    for (index, text) in raw_segments.into_iter().enumerate() {
        let numbers = citation_numbers(&text, evidence.len())?;
        let citations = build_citations(numbers, evidence);
        if citations.is_empty() {
            return Err(AppError::invalid_input("生成段落没有可解析引用。"));
        }
        segments.push(WritingSegment {
            id: format!("W{}", index + 1),
            text,
            citations,
        });
    }
    Ok(segments)
}

fn template_draft(mode: WritingMode, evidence: &[Evidence]) -> String {
    let limit = match mode {
        WritingMode::Outline => 6,
        WritingMode::Shorten => 3,
        _ => 5,
    };
    evidence
        .iter()
        .take(limit)
        .enumerate()
        .map(|(index, item)| {
            let heading = if item.chunk.heading_path.is_empty() {
                String::new()
            } else {
                format!("（{}）", item.chunk.heading_path.join(" > "))
            };
            let plain = markdown_to_plain_text(&item.chunk.snippet)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let text = truncate_chars(plain.trim(), 260);
            if mode == WritingMode::Outline {
                format!("- {} · {}{} [{}]", item.title, text, heading, index + 1)
            } else {
                format!("{text}{heading} [{}]", index + 1)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn validate_and_join_segments(
    notes: &NoteService,
    segments: &[WritingSegment],
    max_chars: usize,
) -> Result<(String, Vec<AskCitation>)> {
    if segments.is_empty() {
        return Err(AppError::invalid_input("没有可写入的生成段落。"));
    }
    let mut citations = Vec::new();
    let mut insertion_parts = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let text = segment.text.trim();
        if text.is_empty() || text.chars().count() > 8_000 {
            return Err(AppError::invalid_input(format!(
                "第 {} 个生成段落为空或过长。",
                index + 1
            )));
        }
        if segment.citations.is_empty() {
            return Err(AppError::invalid_input(format!(
                "第 {} 个生成段落没有引用，不能写入。",
                index + 1
            )));
        }
        for citation in &segment.citations {
            let numeric = citation
                .citation_id
                .strip_prefix('C')
                .filter(|value| value.chars().all(|ch| ch.is_ascii_digit()))
                .ok_or_else(|| AppError::invalid_input("写作引用编号格式无效。"))?;
            if !text.contains(&format!("[{}]", citation.citation_id))
                && !text.contains(&format!("[{numeric}]"))
            {
                return Err(AppError::invalid_input(format!(
                    "第 {} 个段落没有使用引用 {}。",
                    index + 1,
                    citation.citation_id
                )));
            }
            validate_citation(notes, citation)?;
        }
        insertion_parts.push(text.to_string());
        citations.extend(segment.citations.clone());
    }
    let insertion = insertion_parts.join("\n\n");
    if insertion.chars().count() > max_chars {
        return Err(AppError::invalid_input("接受段落的正文总量超过写入上限。"));
    }
    Ok((insertion, dedupe_citations(citations)))
}

fn validate_citation(notes: &NoteService, citation: &AskCitation) -> Result<()> {
    let detail = notes.get(&citation.note_id).map_err(|_| {
        AppError::new(
            ErrorCode::NoteConflict,
            format!("来源笔记「{}」已不存在；请重新生成草稿。", citation.title),
        )
    })?;
    let chars: Vec<char> = detail.content.chars().collect();
    if citation.char_start < 0
        || citation.char_end <= citation.char_start
        || citation.char_end as usize > chars.len()
    {
        return Err(AppError::new(
            ErrorCode::NoteConflict,
            format!("来源笔记「{}」的引用区间已失效。", citation.title),
        ));
    }
    let source: String = chars[citation.char_start as usize..citation.char_end as usize]
        .iter()
        .collect();
    if truncate_chars(source.trim(), MAX_SOURCE_CHARS) != citation.snippet.trim() {
        return Err(AppError::new(
            ErrorCode::NoteConflict,
            format!(
                "来源笔记「{}」在草稿生成后已修改；请重新生成。",
                citation.title
            ),
        ));
    }
    Ok(())
}

fn unique_citations(segments: &[WritingSegment]) -> Vec<AskCitation> {
    dedupe_citations(
        segments
            .iter()
            .flat_map(|segment| segment.citations.clone())
            .collect(),
    )
}

fn dedupe_citations(citations: Vec<AskCitation>) -> Vec<AskCitation> {
    let mut seen = HashSet::new();
    citations
        .into_iter()
        .filter(|citation| seen.insert(citation.citation_id.clone()))
        .collect()
}

fn append_source_section(content: &mut String, citations: &[AskCitation]) {
    if citations.is_empty() {
        return;
    }
    let mut sources: HashMap<String, (&str, HashSet<String>)> = HashMap::new();
    for citation in citations {
        let entry = sources
            .entry(citation.note_id.clone())
            .or_insert((citation.title.as_str(), HashSet::new()));
        if !citation.heading.trim().is_empty() {
            entry.1.insert(citation.heading.clone());
        }
    }
    content.push_str("\n\n## 写作来源\n\n");
    let mut sources: Vec<_> = sources.into_values().collect();
    sources.sort_by(|left, right| left.0.cmp(right.0));
    for (title, headings) in sources {
        let heading = if headings.is_empty() {
            String::new()
        } else {
            let mut headings: Vec<_> = headings.into_iter().collect();
            headings.sort();
            format!(" · {}", headings.join("；"))
        };
        content.push_str(&format!("- [[{title}]]{heading}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::{RetrievalOrigin, RetrievedChunk};

    fn evidence() -> Vec<Evidence> {
        vec![Evidence {
            chunk: RetrievedChunk {
                note_id: "note".to_string(),
                chunk_index: 0,
                heading_path: vec!["策略".to_string()],
                snippet: "本地优先是核心边界。".to_string(),
                char_start: 0,
                char_end: 10,
                score: 1.0,
                origin: RetrievalOrigin::Bm25,
            },
            title: "原则".to_string(),
            level: SourceLevel::B,
            stale: false,
        }]
    }

    #[test]
    fn every_list_item_requires_its_own_valid_citation() {
        let parsed = parse_segments("- 第一项 [1]\n- 第二项 [2]", &evidence());
        assert!(parsed.is_err(), "未知引用必须使整份草稿失败");
        let parsed = parse_segments("- 第一项 [1]\n- 第二项 [1]", &evidence()).expect("合法引用");
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().all(|segment| segment.citations.len() == 1));
    }

    #[test]
    fn template_draft_is_honest_and_cited() {
        let text = template_draft(WritingMode::Outline, &evidence());
        assert!(text.contains("本地优先"));
        assert!(text.ends_with("[1]"));
        assert!(parse_segments(&text, &evidence()).is_ok());
    }
}
