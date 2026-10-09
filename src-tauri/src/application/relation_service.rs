//! 类型化关系服务（N2）。
//!
//! 设计：**frontmatter 是唯一事实来源**。用户声明的关系写在来源笔记的
//! `relations: [{ type, target, note }]` 里；`note_relations` 表是按标题解析后的派生数据，
//! 重命名、删除、重建都会按正文重新生成（与 `note_links` 同一套思路）。
//!
//! 目标解析复用链接的标题索引：标题（大小写不敏感）→ 工作区相对路径 → 文件名。

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::ids::ulid;
use crate::core::time::now_iso;
use crate::domain::note::{Note, NoteStatus};
use crate::infrastructure::frontmatter::{self, Frontmatter, RelationEntry};
use crate::repositories::link_repository::{LinkRepository, NoteTitleRow, SqliteLinkRepository};
use crate::repositories::{
    NoteRepository, RelationRepository, RelationRow, SqliteNoteRepository,
    SqliteRelationRepository,
};

use super::context::WorkspaceContext;

/// 允许的关系类型（与 V2.0 架构文档 §13 对齐）。
pub const RELATION_TYPES: [&str; 9] = [
    "RELATED_TO",
    "DERIVED_FROM",
    "SUPERSEDES",
    "CONTRADICTS",
    "CONFIRMS",
    "DEPENDS_ON",
    "PART_OF",
    "EXAMPLE_OF",
    "REFERENCES",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationView {
    /// 本笔记声明的关系（出边）
    pub outgoing: Vec<OutgoingRelation>,
    /// 别人指向本笔记的关系（入边）
    pub incoming: Vec<IncomingRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingRelation {
    pub id: String,
    pub relation_type: String,
    pub target_note_id: String,
    pub target_title: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingRelation {
    pub id: String,
    pub relation_type: String,
    pub source_note_id: String,
    pub source_title: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRelationInput {
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    #[serde(default)]
    pub note: Option<String>,
}

pub struct RelationService {
    ctx: Arc<WorkspaceContext>,
}

impl RelationService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    /// 某篇笔记的关系（出边 + 入边）。
    pub fn list(&self, note_id: &str) -> Result<RelationView> {
        let outgoing_rows = self
            .ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.list_for_source(conn, note_id))?;
        let incoming_rows = self
            .ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.list_for_target(conn, note_id))?;

        let mut outgoing = Vec::with_capacity(outgoing_rows.len());
        for row in outgoing_rows {
            let Some(target) = self.note(&row.target_note_id)? else {
                continue;
            };
            outgoing.push(OutgoingRelation {
                id: row.id,
                relation_type: row.relation_type,
                target_note_id: target.id,
                target_title: target.title,
                note: row.note,
            });
        }

        let mut incoming = Vec::with_capacity(incoming_rows.len());
        for row in incoming_rows {
            let Some(source) = self.note(&row.source_note_id)? else {
                continue;
            };
            incoming.push(IncomingRelation {
                id: row.id,
                relation_type: row.relation_type,
                source_note_id: source.id,
                source_title: source.title,
                note: row.note,
            });
        }

        Ok(RelationView { outgoing, incoming })
    }

    /// 新增一条关系：写入来源笔记的 frontmatter，并刷新派生表。
    pub fn add(&self, input: AddRelationInput) -> Result<RelationView> {
        let relation_type = normalize_relation_type(&input.relation_type)?;
        if input.source_id == input.target_id {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "不能给笔记自己建立关系。",
            ));
        }
        let source = self.note_or_err(&input.source_id)?;
        let target = self.note_or_err(&input.target_id)?;

        let stored = self.ctx.storage.read_note(&source.file_path)?;
        let mut relations = stored.frontmatter.relations.clone();
        let already = relations.iter().any(|entry| {
            entry.target == target.title && normalize_relation_type(&entry.relation_type).is_ok()
                && entry.relation_type.eq_ignore_ascii_case(&relation_type)
        });
        if already {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "这条关系已经存在。",
            ));
        }
        relations.push(RelationEntry {
            relation_type: relation_type.clone(),
            target: target.title.clone(),
            note: input
                .note
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        });
        self.write_relations(&source, &stored, &relations)?;
        self.list(&source.id)
    }

    /// 删除一条关系（按派生表 id，回到 frontmatter 里按类型 + 目标标题定位）。
    pub fn remove(&self, relation_id: &str) -> Result<RelationView> {
        let row = self
            .ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.get(conn, relation_id))?;
        let Some(row) = row else {
            return Err(AppError::new(ErrorCode::InvalidInput, "关系不存在。"));
        };
        let source = self.note_or_err(&row.source_note_id)?;
        let target = self.note(&row.target_note_id)?;
        let stored = self.ctx.storage.read_note(&source.file_path)?;
        let target_title = target.map(|note| note.title).unwrap_or_default();
        let relations: Vec<RelationEntry> = stored
            .frontmatter
            .relations
            .iter()
            .filter(|entry| {
                !(entry.relation_type.eq_ignore_ascii_case(&row.relation_type)
                    && (entry.target == target_title || target_title.is_empty()))
            })
            .cloned()
            .collect();
        self.write_relations(&source, &stored, &relations)?;
        self.list(&source.id)
    }

    /// 重命名目标笔记后：改写所有来源笔记 frontmatter 里的目标标题，并刷新派生表。
    pub fn on_note_renamed(&self, note_id: &str, old_title: &str, new_title: &str) -> Result<usize> {
        if old_title == new_title {
            return Ok(0);
        }
        let rows = self
            .ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.list_for_target(conn, note_id))?;
        let mut seen: Vec<String> = Vec::new();
        let mut changed = 0usize;
        for row in rows {
            if row.source_note_id == note_id || seen.contains(&row.source_note_id) {
                continue;
            }
            seen.push(row.source_note_id.clone());
            let Some(source) = self.note(&row.source_note_id)? else {
                continue;
            };
            let stored = self.ctx.storage.read_note(&source.file_path)?;
            let mut touched = false;
            let relations: Vec<RelationEntry> = stored
                .frontmatter
                .relations
                .iter()
                .map(|entry| {
                    if entry.target == old_title || entry.target == row.target_note_id {
                        touched = true;
                        RelationEntry {
                            target: new_title.to_string(),
                            ..entry.clone()
                        }
                    } else {
                        entry.clone()
                    }
                })
                .collect();
            if touched {
                self.write_relations(&source, &stored, &relations)?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// 目标笔记不可用（删除 / 移入回收站）：解除派生关系。
    pub fn on_note_unavailable(&self, note_id: &str) -> Result<usize> {
        self.ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.unbind_target(conn, note_id))
    }

    /// 全量重建派生关系表（正文是唯一事实来源）。
    pub fn rebuild(&self) -> Result<usize> {
        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, &self.ctx.workspace.id, NoteStatus::Active)
        })?;
        self.ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.clear_workspace(conn, &self.ctx.workspace.id))?;
        // 标题索引只构建一次：refresh_for_source 每篇都会重建它，逐篇调用即 O(N²)
        let index = self.title_index()?;
        let mut total = 0usize;
        for note in &notes {
            if let Ok(stored) = self.ctx.storage.read_note(&note.file_path) {
                total += self.refresh_for_source_with_index(note, &stored.frontmatter, &index)?;
            }
        }
        Ok(total)
    }

    /// 按笔记的 frontmatter 重建它的出边（保存 / 扫描 / 导入后调用）。
    pub fn refresh_note(&self, note: &Note) -> Result<usize> {
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        self.refresh_for_source(note, &stored.frontmatter)
    }

    // ------------------------------------------------------------ 内部

    fn refresh_for_source(&self, note: &Note, frontmatter: &Frontmatter) -> Result<usize> {
        let index = self.title_index()?;
        self.refresh_for_source_with_index(note, frontmatter, &index)
    }

    /// 复用已构建的标题索引（批量场景，避免 O(N²)）。
    fn refresh_for_source_with_index(
        &self,
        note: &Note,
        frontmatter: &Frontmatter,
        index: &HashMap<String, String>,
    ) -> Result<usize> {
        let timestamp = now_iso();
        let workspace_id = self.ctx.workspace.id.clone();
        let mut rows: Vec<RelationRow> = Vec::new();
        for (position, entry) in frontmatter.relations.iter().enumerate() {
            let Ok(relation_type) = normalize_relation_type(&entry.relation_type) else {
                continue;
            };
            let Some(target_id) = resolve_target(&index, &entry.target) else {
                // 目标还不存在：跳过（frontmatter 仍保留声明，目标出现后重建即可恢复）
                continue;
            };
            if target_id == note.id {
                continue;
            }
            rows.push(RelationRow {
                id: format!("rel_{}", ulid()),
                workspace_id: workspace_id.clone(),
                source_note_id: note.id.clone(),
                target_note_id: target_id,
                relation_type,
                note: entry.note.clone(),
                position: position as i64,
                created_at: timestamp.clone(),
            });
        }
        let count = rows.len();
        self.ctx.db.with_transaction(|tx| {
            SqliteRelationRepository.delete_for_source(tx, &note.id)?;
            for row in &rows {
                SqliteRelationRepository.insert(tx, &workspace_id, &note.id, row)?;
            }
            Ok(())
        })?;
        Ok(count)
    }

    /// 写回 frontmatter（保持其余字段不变）。
    fn write_relations(
        &self,
        note: &Note,
        stored: &crate::infrastructure::markdown_storage::StoredNote,
        relations: &[RelationEntry],
    ) -> Result<()> {
        let lookup = self.ctx.category_lookup()?;
        let category_path = note
            .category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(note.title.clone()),
            note_type: stored.frontmatter.note_type.clone(),
            status: stored.frontmatter.status.clone(),
            category: Some(category_path).filter(|value| !value.is_empty()),
            tags: stored.frontmatter.tags.clone(),
            created_at: stored.frontmatter.created_at.clone(),
            updated_at: Some(now_iso()),
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
            parent: stored.frontmatter.parent.clone(),
            relations: relations.to_vec(),
            extra: stored.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &stored.body);
        self.ctx.storage.write_atomic(&note.file_path, &markdown)?;
        self.refresh_for_source(note, &frontmatter)?;
        Ok(())
    }

    fn title_index(&self) -> Result<HashMap<String, String>> {
        let rows: Vec<NoteTitleRow> = self.ctx.db.with_conn(|conn| {
            SqliteLinkRepository.list_titles(conn, &self.ctx.workspace.id)
        })?;
        let mut index = HashMap::new();
        for row in rows {
            index
                .entry(normalize_key(&row.title))
                .or_insert_with(|| row.id.clone());
            index
                .entry(normalize_key(&row.file_path))
                .or_insert_with(|| row.id.clone());
            let file_name = row
                .file_path
                .rsplit('/')
                .next()
                .unwrap_or(&row.file_path)
                .to_string();
            index
                .entry(normalize_key(&file_name))
                .or_insert_with(|| row.id.clone());
        }
        Ok(index)
    }

    fn note(&self, id: &str) -> Result<Option<Note>> {
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, id))
    }

    fn note_or_err(&self, id: &str) -> Result<Note> {
        self.note(id)?.ok_or_else(|| {
            AppError::new(ErrorCode::NoteNotFound, "笔记不存在或已被删除。")
        })
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_lowercase().replace('\\', "/")
}

fn resolve_target(index: &HashMap<String, String>, target: &str) -> Option<String> {
    let key = normalize_key(target);
    if key.is_empty() {
        return None;
    }
    if let Some(id) = index.get(&key) {
        return Some(id.clone());
    }
    let with_md = if key.ends_with(".md") {
        key.clone()
    } else {
        format!("{key}.md")
    };
    index.get(&with_md).cloned()
}

fn normalize_relation_type(value: &str) -> Result<String> {
    let upper = value.trim().to_ascii_uppercase().replace([' ', '-'], "_");
    if RELATION_TYPES.contains(&upper.as_str()) {
        Ok(upper)
    } else {
        Err(AppError::new(
            ErrorCode::InvalidInput,
            format!("不支持的关系类型：{value}"),
        ))
    }
}
