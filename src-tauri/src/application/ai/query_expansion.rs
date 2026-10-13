//! 查询扩展（D5 / N9 第一步）：把「一个课题」变成**一组**检索查询。
//!
//! 为什么是规则而不是模型：这一层要**永远可用**（离线、零成本、可复现），
//! 模型改写属于「有模型时更好」的增强，不能成为检索能不能跑的前提。
//! 规则只做三件**能解释**的事：
//!
//! 1. **括号里的短语优先**——用户用 `「」` / 引号圈出来的，通常就是他真正在找的词；
//! 2. **按连接词拆开**——「A 与 B」「B 的 C」这类课题，拆出来的片段比整句更容易命中
//!    （中文 FTS 是「双字扩展 + 整段前缀」，整句过长时命中面太窄）；
//! 3. **保留拉丁词**——`Rust` / `SQLite` / `FTS5` 这类词在中文句子里是天然的强特征。
//!
//! **刻意不做的事**：不发明新词、不做同义词替换、不做拼写纠错——
//! 那些需要语义模型（下一阶段），规则瞎猜只会把噪声引进来源列表。

use crate::domain::research::SubQuestion;

/// 一次检索最多发几条查询（含课题原文）。上限存在的意义是**可预测的成本**：
/// 每条查询都要过一次 FTS，条数越多越慢；6 条在实测里已经够用。
pub const MAX_QUERIES: usize = 6;

/// 中文片段的最短长度：一个字太泛（"的"、"力"），至少两个字才有区分度。
const MIN_CJK_CHARS: usize = 2;
/// 拉丁词的最短长度：`a` / `io` 这类噪声直接丢掉。
const MIN_ASCII_CHARS: usize = 2;

/// 连接词 / 标点：课题里出现它们时，两侧通常是**不同的**检索意图。
const SPLITTERS: [char; 14] = [
    '与', '和', '及', '或', '、', '，', ',', '；', ';', '：', ':', '/', '|', '+',
];

/// 一对成对符号的内容会被单独当作一条查询。
const QUOTE_PAIRS: [(char, char); 5] = [
    ('「', '」'),
    ('“', '”'),
    ('"', '"'),
    ('‘', '’'),
    ('(', ')'),
];

/// 一条扩展出来的查询。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedQuery {
    pub text: String,
    /// 来源：`topic` / `quoted` / `split` / `latin` / `keyword`（写进 Trace，便于解释"这条从哪来"）
    pub origin: &'static str,
}

/// 展开课题为一组查询（顺序即优先级，前几条最重要）。
pub fn expand(topic: &str, sub_questions: &[SubQuestion], max: usize) -> Vec<ExpandedQuery> {
    let limit = max.max(1);
    let mut out: Vec<ExpandedQuery> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let mut push = |text: String, origin: &'static str, out: &mut Vec<ExpandedQuery>| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let key = normalize_key(trimmed);
        if key.is_empty() || seen.contains(&key) {
            return;
        }
        seen.push(key);
        out.push(ExpandedQuery { text: trimmed.to_string(), origin });
    };

    // ① 课题原文：永远是第一条——扩展的目的是**补充**，不是替换。
    push(topic.to_string(), "topic", &mut out);

    // ② 成对符号里的短语（用户明确圈定的词）
    for (open, close) in QUOTE_PAIRS {
        for piece in between_pairs(topic, open, close) {
            if piece.chars().count() >= MIN_CJK_CHARS {
                push(piece, "quoted", &mut out);
            }
        }
    }

    // ③ 连接词 / 标点切分出的片段
    for piece in split_on_connectives(topic) {
        push(piece, "split", &mut out);
    }

    // ④ 拉丁词（去掉纯数字与过短的）
    for piece in latin_tokens(topic) {
        push(piece, "latin", &mut out);
    }

    // ⑤ 子问题自带的关键词（当前模板只放课题本身，去重后自然为空；
    //    保留这一步是为了将来子问题能带更细的关键词时**不用改这里**）
    for question in sub_questions {
        for keyword in &question.keywords {
            push(keyword.clone(), "keyword", &mut out);
        }
    }

    out.truncate(limit);
    out
}

/// 去重用的归一化：小写 + 去掉空白与常见标点，避免「Rust 异步」与「rust异步」重复检索。
fn normalize_key(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace() && !"「」“”\"'‘’()（）".contains(*ch))
        .collect()
}

/// 取出 `open...close` 之间的内容（不处理嵌套，够用且可预测）。
fn between_pairs(text: &str, open: char, close: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut inside = false;
    for ch in text.chars() {
        if !inside && ch == open {
            inside = true;
            current.clear();
            continue;
        }
        if inside && ch == close {
            inside = false;
            let value = current.trim().to_string();
            if !value.is_empty() {
                out.push(value);
            }
            current.clear();
            continue;
        }
        if inside {
            current.push(ch);
        }
    }
    out
}

/// 按连接词切分课题，逐段判断是否值得单独检索。
fn split_on_connectives(topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let flush = |current: &mut String, out: &mut Vec<String>| {
        let value = current.trim().to_string();
        current.clear();
        if value.is_empty() {
            return;
        }
        // 片段要么够长的中文，要么含拉丁词——否则就是「的」「等等」这类碎片
        let cjk_chars = value.chars().filter(|ch| crate::core::text::is_cjk(*ch)).count();
        let has_latin = value.chars().any(|ch| ch.is_ascii_alphanumeric());
        if cjk_chars >= MIN_CJK_CHARS || (has_latin && value.chars().count() >= MIN_ASCII_CHARS) {
            out.push(value);
        }
    };
    for ch in topic.chars() {
        if SPLITTERS.contains(&ch) {
            flush(&mut current, &mut out);
            continue;
        }
        current.push(ch);
    }
    flush(&mut current, &mut out);
    out
}

/// 抽出拉丁词（`Rust`、`SQLite`、`FTS5`、`api.perplexity.ai`）。
fn latin_tokens(topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let flush = |current: &mut String, out: &mut Vec<String>| {
        let value = std::mem::take(current);
        if value.is_empty() {
            return;
        }
        // 纯数字没有区分度；字母开头的才留
        let letters = value.chars().filter(|ch| ch.is_ascii_alphabetic()).count();
        if letters >= 1 && value.chars().count() >= MIN_ASCII_CHARS {
            out.push(value);
        }
    };
    for ch in topic.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            current.push(ch);
        } else {
            flush(&mut current, &mut out);
        }
    }
    flush(&mut current, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn questions(keywords: &[&str]) -> Vec<SubQuestion> {
        keywords
            .iter()
            .enumerate()
            .map(|(index, keyword)| SubQuestion {
                id: format!("q{}", index + 1),
                question: format!("「{keyword}」是什么？"),
                keywords: vec![keyword.to_string()],
                local_hits: 0,
            })
            .collect()
    }

    #[test]
    fn topic_always_comes_first_and_is_never_dropped() {
        let expanded = expand("Rust 异步运行时的调度策略", &questions(&[]), MAX_QUERIES);
        assert_eq!(expanded[0].text, "Rust 异步运行时的调度策略");
        assert_eq!(expanded[0].origin, "topic");
    }

    #[test]
    fn connectives_surface_the_two_sides_as_their_own_queries() {
        let expanded = expand("SQLite 全文检索与中文分词", &questions(&[]), MAX_QUERIES);
        let texts: Vec<&str> = expanded.iter().map(|item| item.text.as_str()).collect();
        assert!(texts.contains(&"SQLite 全文检索"), "{texts:?}");
        assert!(texts.contains(&"中文分词"), "{texts:?}");
    }

    #[test]
    fn quoted_phrases_are_lifted_out_before_other_expansions() {
        let expanded = expand("关于「冲突保护」的实现", &questions(&[]), MAX_QUERIES);
        assert_eq!(expanded[1].text, "冲突保护");
        assert_eq!(expanded[1].origin, "quoted");
    }

    #[test]
    fn latin_tokens_are_kept_but_pure_numbers_are_not() {
        let expanded = expand("FTS5 与 SQLite 的 12345 号问题", &questions(&[]), MAX_QUERIES);
        let texts: Vec<&str> = expanded.iter().map(|item| item.text.as_str()).collect();
        assert!(texts.contains(&"FTS5"), "{texts:?}");
        assert!(texts.contains(&"SQLite"), "{texts:?}");
        assert!(!texts.iter().any(|text| *text == "12345"), "{texts:?}");
    }

    #[test]
    fn duplicates_are_deduped_case_and_space_insensitively() {
        let expanded = expand("Rust 并发", &questions(&["rust并发", "Rust 并发"]), MAX_QUERIES);
        let count = expanded
            .iter()
            .filter(|item| normalize_key(&item.text) == "rust并发")
            .count();
        assert_eq!(count, 1, "同一个查询不该发两遍：{:?}", expanded);
    }

    #[test]
    fn the_cap_is_respected_even_for_a_long_topic() {
        let topic = "Rust / SQLite / 中文分词 / 调度 / 备份 / 恢复 / 图谱 / 搜索 / 索引 / 同步";
        let expanded = expand(topic, &questions(&[]), 3);
        assert_eq!(expanded.len(), 3);
    }
}
