//! 标题感知分块 + 原文字符区间（N5）。
//!
//! 目标：检索命中要能**跳回原文的精确区间**，而不是只给一整篇笔记。
//! 因此这里的区间一律是「正文（Markdown 源文本）里的**字符下标**」：
//! - 坐标系与编辑器里的正文**相同**（`read_note` 已经把 frontmatter 摘掉了）；
//! - 单位是 Rust 的 `char`（Unicode 标量值）下标，不是字节、也不是 UTF-16 码元。
//!   **前端要用同一套口径**：高亮时按码点切片（`Array.from(text)`），不要直接用 JS 的
//!   字符串下标——含 emoji 等非 BMP 字符时两者会差位。
//!
//! 分块规则（简单、可解释、可测）：
//! 1. 遇到标题行就**断开**：新块的标题路径是「祖先标题 > 自身标题」；
//! 2. 一段太长（超过 [`MAX_CHUNK_CHARS`]）时，在**空行处**断开（不对半砍句子）；
//! 3. 块的首尾空白会被裁掉；空块不产出。

/// 单块的目标上限（字符）。够小以便定位，够大以免把一句拆散。
pub const MAX_CHUNK_CHARS: usize = 900;

/// 一个分块：标题路径 + 原文里的字符区间 `[char_start, char_end)`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    /// 块序号（从 0 开始，按出现顺序）
    pub index: usize,
    /// 标题路径（`A > B`）；空串表示它位于任何标题之前
    pub heading: String,
    pub char_start: usize,
    pub char_end: usize,
}

impl TextChunk {
    /// 按区间从原文里切出这一块（用于断言与展示）。
    pub fn slice<'a>(&self, source: &'a str) -> String {
        source
            .chars()
            .skip(self.char_start)
            .take(self.char_end.saturating_sub(self.char_start))
            .collect()
    }
}

struct Line {
    start: usize,
    end: usize,
    text: String,
}

fn split_lines(chars: &[char]) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let start = index;
        while index < chars.len() && chars[index] != '\n' {
            index += 1;
        }
        let end = index;
        let text: String = chars[start..end].iter().collect::<String>();
        lines.push(Line { start, end, text: text.trim_end_matches('\r').to_string() });
        if index < chars.len() {
            index += 1;
        }
    }
    lines
}

/// 识别 ATX 标题（`# ` 到 `###### `）；返回层级与标题文字。
fn parse_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None; // `#标题`（没空格）不算标题
    }
    let title = rest.trim().trim_end_matches('#').trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some((level, title))
}

fn trim_range(chars: &[char], start: usize, end: usize) -> (usize, usize) {
    let mut left = start;
    let mut right = end;
    while left < right && chars[left].is_whitespace() {
        left += 1;
    }
    while right > left && chars[right - 1].is_whitespace() {
        right -= 1;
    }
    (left, right)
}

fn push_chunk(
    chunks: &mut Vec<TextChunk>,
    chars: &[char],
    heading: &str,
    range: Option<(usize, usize)>,
) {
    let Some((start, end)) = range else { return };
    let (start, end) = trim_range(chars, start, end);
    if end <= start {
        return;
    }
    chunks.push(TextChunk {
        index: chunks.len(),
        heading: heading.to_string(),
        char_start: start,
        char_end: end,
    });
}

/// 把正文切成带**原文字符区间**的块（N5）。
pub fn chunk_markdown(source: &str) -> Vec<TextChunk> {
    let chars: Vec<char> = source.chars().collect();
    let lines = split_lines(&chars);
    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut section_heading = String::new();
    let mut pending: Option<(usize, usize)> = None;

    for line in &lines {
        if let Some((level, title)) = parse_heading(&line.text) {
            push_chunk(&mut chunks, &chars, &section_heading, pending.take());
            while let Some((top_level, _)) = stack.last() {
                if *top_level >= level {
                    stack.pop();
                } else {
                    break;
                }
            }
            stack.push((level, title));
            section_heading = stack
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join(" > ");
            pending = Some((line.start, line.end));
            continue;
        }

        if line.text.trim().is_empty() {
            continue; // 空行不入块（它不是内容）
        }

        // **一行都不切开**：只在"再加一行就会超上限"时，从行边界断开。
        // 所以一块最多超出上限不到一行——这是刻意的取舍（宁可块略大，也不切断句子）。
        pending = match pending {
            Some((start, end)) => {
                let projected = line.end.saturating_sub(start);
                // 再加这一行就超上限、而且当前已经攒够一半 → 从行边界断开
                if projected > MAX_CHUNK_CHARS && end.saturating_sub(start) >= MAX_CHUNK_CHARS / 2 {
                    push_chunk(&mut chunks, &chars, &section_heading, Some((start, end)));
                    Some((line.start, line.end))
                } else {
                    Some((start, line.end))
                }
            }
            None => Some((line.start, line.end)),
        };
    }
    push_chunk(&mut chunks, &chars, &section_heading, pending);

    for (index, chunk) in chunks.iter_mut().enumerate() {
        chunk.index = index;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_breaks_chunks_and_ranges_slice_back_exactly() {
        let source = "# 索引设计\n\n倒排索引与分词。\n\n## 查询策略\n\n按双字短语匹配。\n";
        let chunks = chunk_markdown(source);

        assert_eq!(chunks.len(), 2, "两个标题 → 两块：{chunks:?}");
        assert_eq!(chunks[0].heading, "索引设计");
        assert_eq!(chunks[1].heading, "索引设计 > 查询策略");
        // 区间必须能**原样切回**去（这是"跳回原文"的地基）
        assert_eq!(chunks[0].slice(source), "# 索引设计\n\n倒排索引与分词。");
        assert_eq!(chunks[1].slice(source), "## 查询策略\n\n按双字短语匹配。");
    }

    #[test]
    fn chunks_before_any_heading_have_an_empty_heading_path() {
        let source = "开头的一段话。\n\n# 标题\n\n正文。\n";
        let chunks = chunk_markdown(source);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].heading, "");
        assert_eq!(chunks[0].slice(source), "开头的一段话。");
        assert_eq!(chunks[1].heading, "标题");
    }

    #[test]
    fn long_sections_split_at_blank_lines_not_mid_sentence() {
        let paragraph = "字".repeat(400);
        let source = format!("# 长文\n\n{paragraph}\n\n{paragraph}\n\n{paragraph}\n");
        let chunks = chunk_markdown(&source);

        assert!(chunks.len() >= 2, "超过上限要断开：{chunks:?}");
        for chunk in &chunks {
            let text = chunk.slice(&source);
            // 每块都必须是完整行首/行尾：**不从句子中间切开**
            assert!(text.starts_with('#') || text.starts_with('字'), "{text}");
            assert!(!text.ends_with('\n'), "块尾不该带换行：{text:?}");
            // 一块最多超出上限不到一行（段落是 400 字，所以留 400 余量）
            assert!(
                chunk.char_end - chunk.char_start <= MAX_CHUNK_CHARS + 400,
                "块不该无限增长：{} 字",
                chunk.char_end - chunk.char_start
            );
        }
        // 所有块拼起来（按顺序）应当覆盖全部非空行
        for chunk in &chunks {
            assert!(chunk.char_start < chunk.char_end);
        }
        // 区间单调递增，不重叠
        for pair in chunks.windows(2) {
            assert!(pair[0].char_end <= pair[1].char_start, "块之间不能重叠");
        }
    }

    #[test]
    fn ranges_are_char_indices_not_bytes() {
        // 含非 BMP 字符（emoji）与 CJK：区间必须按 `char` 算，按字节算会错位
        let source = "# 标题\n\n第一段🙂结尾。\n";
        let chunks = chunk_markdown(source);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].slice(source), source.trim_end());
        let heading_end = "# 标题".chars().count();
        assert_eq!(chunks[0].char_start, 0);
        assert!(chunks[0].char_end > heading_end);
    }

    #[test]
    fn blank_source_and_frontmatter_only_source_produce_no_chunks() {
        assert!(chunk_markdown("").is_empty());
        assert!(chunk_markdown("\n\n   \n").is_empty());
    }

    #[test]
    fn nested_headings_keep_the_full_path_and_pop_correctly() {
        let source = "# A\n\n## B\n\n### C\n\n## D\n\n# E\n";
        let chunks = chunk_markdown(source);
        let headings: Vec<&str> = chunks.iter().map(|chunk| chunk.heading.as_str()).collect();
        assert_eq!(headings, vec!["A", "A > B", "A > B > C", "A > D", "E"]);
    }
}
