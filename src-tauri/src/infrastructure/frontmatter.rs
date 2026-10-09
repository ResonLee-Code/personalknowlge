//! Markdown frontmatter 解析与序列化（设计文档 12.3 ~ 12.6）。
//!
//! 约定：
//! - 必填字段：`id`、`title`、`created_at`、`updated_at`
//! - 可选字段：`type`、`status`、`category`、`tags`
//! - 未知字段必须原样保留，避免破坏用户自定义元数据
//!
//! 解析失败时不会丢弃任何内容：整个文件都会被当成正文返回。

use serde_yaml::{Mapping, Value};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frontmatter {
    pub id: Option<String>,
    pub title: Option<String>,
    pub note_type: Option<String>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// 来源类型（官方文档 / 书籍 / 实测 / 个人推测 …）
    pub source_type: Option<String>,
    /// 来源引用（URL、书名、出处）
    pub source_ref: Option<String>,
    /// 来源可靠性 1–5
    pub source_reliability: Option<i64>,
    /// 时效：超过该日期视为可能过期（ISO 8601 日期或时间）
    pub valid_until: Option<String>,
    /// 父文档（子文档层级）：父笔记 id 或标题
    pub parent: Option<String>,
    /// 手动排序：同层内的显示顺序（1 起，越小越靠前）
    pub order: Option<i64>,
    /// 用户显式声明的类型化关系（N2）
    pub relations: Vec<RelationEntry>,
    /// 未识别的字段，按原顺序保留
    pub extra: Vec<(String, Value)>,
}

/// frontmatter 中声明的一条类型化关系。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationEntry {
    /// RELATED_TO / DERIVED_FROM / SUPERSEDES / CONTRADICTS / CONFIRMS / DEPENDS_ON / PART_OF / EXAMPLE_OF / REFERENCES
    pub relation_type: String,
    /// 目标笔记标题（与 [[双向链接]] 同一套解析：标题优先，其次路径/文件名）
    pub target: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDocument {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub had_frontmatter: bool,
    /// YAML 解析失败时的错误信息（正文仍然完整保留）
    pub parse_error: Option<String>,
}

/// 解析 Markdown 文本。
pub fn parse(content: &str) -> ParsedDocument {
    let text = content.strip_prefix('\u{feff}').unwrap_or(content);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");

    if !normalized.starts_with("---") {
        return ParsedDocument {
            frontmatter: Frontmatter::default(),
            body: normalized,
            had_frontmatter: false,
            parse_error: None,
        };
    }

    let after_open = normalized[3..].strip_prefix('\n').unwrap_or(&normalized[3..]);
    let Some((yaml_block, body)) = split_frontmatter_block(after_open) else {
        return ParsedDocument {
            frontmatter: Frontmatter::default(),
            body: normalized,
            had_frontmatter: false,
            parse_error: None,
        };
    };

    match serde_yaml::from_str::<Value>(yaml_block) {
        Ok(Value::Mapping(mapping)) => ParsedDocument {
            frontmatter: from_mapping(&mapping),
            body: normalize_body(body),
            had_frontmatter: true,
            parse_error: None,
        },
        Ok(Value::Null) => ParsedDocument {
            frontmatter: Frontmatter::default(),
            body: normalize_body(body),
            had_frontmatter: true,
            parse_error: None,
        },
        Ok(_) => ParsedDocument {
            frontmatter: Frontmatter::default(),
            body: normalize_body(body),
            had_frontmatter: true,
            parse_error: Some("frontmatter 不是键值结构，已忽略。".to_string()),
        },
        Err(error) => ParsedDocument {
            // 解析失败时保留整份文件，避免丢内容
            frontmatter: Frontmatter::default(),
            body: normalized,
            had_frontmatter: false,
            parse_error: Some(format!("frontmatter 解析失败：{error}")),
        },
    }
}

/// 规范化正文首尾空行：frontmatter 与正文之间的分隔空行不属于内容。
fn normalize_body(body: &str) -> String {
    body.trim_start_matches('\n').trim_end_matches('\n').to_string()
}

/// 找到 frontmatter 与正文的分界。
fn split_frontmatter_block(text: &str) -> Option<(&str, &str)> {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n');
        if trimmed.trim() == "---" || trimmed.trim() == "..." {
            let yaml = &text[..offset];
            let body = &text[offset + line.len()..];
            return Some((yaml, body));
        }
        offset += line.len();
    }
    None
}

fn from_mapping(mapping: &Mapping) -> Frontmatter {
    let mut frontmatter = Frontmatter::default();
    for (key, value) in mapping {
        let Some(key_name) = key.as_str() else {
            continue;
        };
        match key_name {
            "id" => frontmatter.id = as_string(value),
            "title" => frontmatter.title = as_string(value),
            "type" => frontmatter.note_type = as_string(value),
            "status" => frontmatter.status = as_string(value),
            "category" => frontmatter.category = as_string(value),
            "tags" => frontmatter.tags = as_string_list(value),
            "created_at" | "createdAt" => frontmatter.created_at = as_string(value),
            "updated_at" | "updatedAt" => frontmatter.updated_at = as_string(value),
            "source_type" | "sourceType" => frontmatter.source_type = as_string(value),
            "source_ref" | "sourceRef" => frontmatter.source_ref = as_string(value),
            "source_reliability" | "sourceReliability" => {
                frontmatter.source_reliability = as_i64(value).filter(|level| (1..=5).contains(level))
            }
            "valid_until" | "validUntil" => frontmatter.valid_until = as_string(value),
            "relations" => frontmatter.relations = as_relations(value),
            "parent" | "parent_id" | "parentId" => frontmatter.parent = as_string(value),
            "order" | "sort_order" | "sortOrder" => frontmatter.order = as_i64(value),
            _ => frontmatter.extra.push((key_name.to_string(), value.clone())),
        }
    }
    frontmatter
}

/// 数字解析：YAML 里 `5` 与 `'5'` 都要能读（序列化时可能被加引号）。
fn as_i64(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    value.as_str().and_then(|text| text.trim().parse::<i64>().ok())
}

fn as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// 解析 `relations:` 序列：`- type: SUPERSEDES` / `  target: 旧笔记` / `  note: 可选说明`
fn as_relations(value: &Value) -> Vec<RelationEntry> {
    let Value::Sequence(items) = value else {
        return Vec::new();
    };
    let mut relations = Vec::new();
    for item in items {
        let Value::Mapping(mapping) = item else {
            continue;
        };
        let mut relation_type: Option<String> = None;
        let mut target: Option<String> = None;
        let mut note: Option<String> = None;
        for (key, value) in mapping {
            match key.as_str().unwrap_or_default() {
                "type" | "relation" | "relation_type" | "relationType" => {
                    relation_type = as_string(value)
                }
                "target" | "to" => target = as_string(value),
                "note" | "reason" | "description" => note = as_string(value),
                _ => {}
            }
        }
        let (Some(relation_type), Some(target)) = (relation_type, target) else {
            continue;
        };
        relations.push(RelationEntry {
            relation_type: relation_type.to_ascii_uppercase(),
            target,
            note,
        });
    }
    relations
}

fn as_string_list(value: &Value) -> Vec<String> {
    match value {
        Value::Sequence(items) => items.iter().filter_map(as_string).collect(),
        Value::String(text) => text
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// 生成规范化的 Markdown 文本。
pub fn serialize(frontmatter: &Frontmatter, body: &str) -> String {
    // 已知字段按设计文档 12.3 的顺序输出；tags 使用缩进块序列风格。
    let mut yaml = String::new();
    let push_scalar = |yaml: &mut String, key: &str, value: Option<&String>| {
        if let Some(value) = value {
            if !value.trim().is_empty() {
                yaml.push_str(&scalar_entry(key, value));
            }
        }
    };

    push_scalar(&mut yaml, "id", frontmatter.id.as_ref());
    push_scalar(&mut yaml, "title", frontmatter.title.as_ref());
    push_scalar(&mut yaml, "type", frontmatter.note_type.as_ref());
    push_scalar(&mut yaml, "status", frontmatter.status.as_ref());
    push_scalar(&mut yaml, "category", frontmatter.category.as_ref());
    if !frontmatter.tags.is_empty() {
        yaml.push_str(&tag_block(&frontmatter.tags));
    }
    push_scalar(&mut yaml, "created_at", frontmatter.created_at.as_ref());
    push_scalar(&mut yaml, "updated_at", frontmatter.updated_at.as_ref());
    push_scalar(&mut yaml, "source_type", frontmatter.source_type.as_ref());
    push_scalar(&mut yaml, "source_ref", frontmatter.source_ref.as_ref());
    if let Some(level) = frontmatter.source_reliability {
        if (1..=5).contains(&level) {
            yaml.push_str(&scalar_entry("source_reliability", &level.to_string()));
        }
    }
    push_scalar(&mut yaml, "valid_until", frontmatter.valid_until.as_ref());
    push_scalar(&mut yaml, "parent", frontmatter.parent.as_ref());
    if let Some(order) = frontmatter.order.filter(|value| *value > 0) {
        // 写成数字（不加引号）：`order: 3`，便于用户手改
        yaml.push_str(&format!("order: {order}\n"));
    }
    if !frontmatter.relations.is_empty() {
        yaml.push_str(&relation_block(&frontmatter.relations));
    }

    // 未知字段原样保留（由 serde_yaml 负责转义）
    let known = [
        "id",
        "title",
        "type",
        "status",
        "category",
        "tags",
        "created_at",
        "updated_at",
        "source_type",
        "source_ref",
        "source_reliability",
        "valid_until",
        "relations",
        "parent",
        "parent_id",
        "parentId",
        "order",
        "sort_order",
        "sortOrder",
    ];
    if !frontmatter.extra.is_empty() {
        let mut extra = Mapping::new();
        for (key, value) in &frontmatter.extra {
            if !known.contains(&key.as_str()) {
                extra.insert(Value::String(key.clone()), value.clone());
            }
        }
        if !extra.is_empty() {
            let text = serde_yaml::to_string(&Value::Mapping(extra)).unwrap_or_default();
            yaml.push_str(text.strip_prefix("---\n").unwrap_or(&text));
        }
    }

    let yaml = yaml.trim_end();

    let body = body.trim_end();
    if body.is_empty() {
        format!("---\n{yaml}\n---\n")
    } else {
        format!("---\n{yaml}\n---\n\n{body}\n")
    }
}

/// 单个标量字段：`key: value`（由 serde_yaml 负责必要时的引号与块标量）。
fn scalar_entry(key: &str, value: &str) -> String {
    let mut mapping = Mapping::new();
    mapping.insert(
        Value::String(key.to_string()),
        Value::String(value.to_string()),
    );
    serde_yaml::to_string(&Value::Mapping(mapping)).unwrap_or_else(|_| format!("{key}: {value}\n"))
}

/// 关系块序列：`relations:` 后接缩进的 `- type/target/note` 行。
fn relation_block(relations: &[RelationEntry]) -> String {
    let mut block = String::from("relations:\n");
    for relation in relations {
        let mut mapping = Mapping::new();
        mapping.insert(
            Value::String("type".to_string()),
            Value::String(relation.relation_type.clone()),
        );
        mapping.insert(
            Value::String("target".to_string()),
            Value::String(relation.target.clone()),
        );
        if let Some(note) = relation.note.as_ref().filter(|value| !value.trim().is_empty()) {
            mapping.insert(Value::String("note".to_string()), Value::String(note.clone()));
        }
        let item = serde_yaml::to_string(&Value::Sequence(vec![Value::Mapping(mapping)]))
            .unwrap_or_else(|_| format!("- type: {}\n", relation.relation_type));
        for line in item.lines() {
            block.push_str("  ");
            block.push_str(line);
            block.push('\n');
        }
    }
    block
}

/// 标签块序列：`tags:` 后接两空格缩进的 `- 标签` 行。
fn tag_block(tags: &[String]) -> String {
    let mut block = String::from("tags:\n");
    for tag in tags {
        let item = serde_yaml::to_string(&Value::Sequence(vec![Value::String(tag.clone())]))
            .unwrap_or_else(|_| format!("- {tag}\n"));
        for line in item.lines() {
            block.push_str("  ");
            block.push_str(line);
            block.push('\n');
        }
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nid: note_01JXYZ123\ntitle: SQLite 全文搜索学习笔记\ntype: note\nstatus: active\ncategory: 技术/数据库\ntags:\n  - sqlite\n  - search\ncreated_at: 2026-09-23T12:00:00+08:00\nupdated_at: 2026-09-23T12:00:00+08:00\n---\n\n# 标题\n\n正文内容。\n";

    #[test]
    fn parses_documented_example() {
        let parsed = parse(SAMPLE);
        assert!(parsed.had_frontmatter);
        assert_eq!(parsed.frontmatter.id.as_deref(), Some("note_01JXYZ123"));
        assert_eq!(parsed.frontmatter.title.as_deref(), Some("SQLite 全文搜索学习笔记"));
        assert_eq!(parsed.frontmatter.category.as_deref(), Some("技术/数据库"));
        assert_eq!(parsed.frontmatter.tags, vec!["sqlite", "search"]);
        assert!(parsed.body.contains("正文内容。"));
        assert!(parsed.parse_error.is_none());
    }

    #[test]
    fn round_trip_keeps_body_and_unknown_fields() {
        let with_extra = "---\nid: note_1\ntitle: 标题\ncustom_field: keep-me\ncreated_at: 2026-09-23T12:00:00+08:00\nupdated_at: 2026-09-23T12:00:00+08:00\n---\n\n正文\n";
        let parsed = parse(with_extra);
        let serialized = serialize(&parsed.frontmatter, &parsed.body);
        assert!(serialized.contains("custom_field: keep-me"), "{serialized}");
        let reparsed = parse(&serialized);
        assert_eq!(reparsed.frontmatter.id.as_deref(), Some("note_1"));
        assert_eq!(reparsed.body.trim(), "正文");
    }

    #[test]
    fn file_without_frontmatter_keeps_full_content() {
        let parsed = parse("# 只有正文\n\n没有元数据。\n");
        assert!(!parsed.had_frontmatter);
        assert!(parsed.frontmatter.id.is_none());
        assert!(parsed.body.contains("没有元数据。"));
    }

    #[test]
    fn broken_yaml_does_not_lose_content() {
        let broken = "---\nid: note_1\ntitle: [未闭合\n---\n\n正文仍然要保留\n";
        let parsed = parse(broken);
        assert!(parsed.parse_error.is_some());
        assert!(parsed.body.contains("正文仍然要保留"));
    }

    #[test]
    fn handles_crlf_and_bom() {
        let crlf = "\u{feff}---\r\nid: note_2\r\ntitle: 标题\r\n---\r\n\r\n正文\r\n";
        let parsed = parse(crlf);
        assert_eq!(parsed.frontmatter.id.as_deref(), Some("note_2"));
        assert_eq!(parsed.body.trim(), "正文");
    }

    #[test]
    fn serializes_canonical_field_order() {
        let frontmatter = Frontmatter {
            id: Some("note_x".into()),
            title: Some("标题".into()),
            tags: vec!["a".into(), "b".into()],
            created_at: Some("2026-09-23T12:00:00+08:00".into()),
            updated_at: Some("2026-09-23T12:00:00+08:00".into()),
            ..Default::default()
        };
        let text = serialize(&frontmatter, "正文");
        let id_pos = text.find("id:").unwrap();
        let title_pos = text.find("title:").unwrap();
        let created_pos = text.find("created_at:").unwrap();
        assert!(id_pos < title_pos && title_pos < created_pos);
        assert!(text.starts_with("---\n"));
        assert!(text.ends_with("正文\n"));
    }

    #[test]
    fn serializes_tags_as_indented_block() {
        let frontmatter = Frontmatter {
            id: Some("note_x".into()),
            title: Some("标题".into()),
            tags: vec!["sqlite".into(), "search".into()],
            created_at: Some("2026-09-23T12:00:00+08:00".into()),
            updated_at: Some("2026-09-23T12:00:00+08:00".into()),
            ..Default::default()
        };
        let text = serialize(&frontmatter, "正文");
        assert!(text.contains("tags:\n  - sqlite\n  - search\n"), "{text}");

        // 输出必须可以被自己解析回来（风格与语义都要正确）
        let reparsed = parse(&text);
        assert_eq!(reparsed.frontmatter.tags, vec!["sqlite", "search"]);
    }

    #[test]
    fn preserves_nested_unknown_fields() {
        let source = "---\nid: note_1\ntitle: 标题\ncustom:\n  - a\n  - b\nmeta:\n  owner: me\ncreated_at: 2026-09-23T12:00:00+08:00\nupdated_at: 2026-09-23T12:00:00+08:00\n---\n\n正文\n";
        let parsed = parse(source);
        let text = serialize(&parsed.frontmatter, &parsed.body);
        let reparsed = parse(&text);
        assert_eq!(reparsed.frontmatter.extra.len(), 2);
        let custom = reparsed
            .frontmatter
            .extra
            .iter()
            .find(|(key, _)| key == "custom")
            .map(|(_, value)| value.clone())
            .expect("custom 字段应保留");
        let items: Vec<String> = match custom {
            Value::Sequence(items) => items
                .iter()
                .map(|item| item.as_str().unwrap_or_default().to_string())
                .collect(),
            _ => panic!("custom 应为序列"),
        };
        assert_eq!(items, vec!["a", "b"]);
        assert!(reparsed.body.contains("正文"));
    }
}
