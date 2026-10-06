-- 004_note_assets.sql
-- V1.1.2 知识资产元数据：来源与时效（N1）+ 类型化关系（N2）
--
-- 1) notes 增加来源与时效列：来自 frontmatter，用于「知识可靠性 / 是否已过期」展示与筛选；
-- 2) note_relations 保存用户显式声明的类型化关系（SUPERSEDES / CONTRADICTS / ...）。
--    与 note_links 一样属于**派生数据**：正文（frontmatter.relations）是唯一事实来源，
--    重命名/删除/重建都会按正文重新生成，因此该表可以整表重建。

ALTER TABLE notes ADD COLUMN source_type TEXT;
ALTER TABLE notes ADD COLUMN source_ref TEXT;
ALTER TABLE notes ADD COLUMN source_reliability INTEGER;
ALTER TABLE notes ADD COLUMN valid_until TEXT;

CREATE TABLE IF NOT EXISTS note_relations (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL,
    source_note_id TEXT NOT NULL,
    target_note_id TEXT NOT NULL,
    relation_type  TEXT NOT NULL,
    note           TEXT,
    position       INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL,
    UNIQUE (source_note_id, target_note_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_note_relations_source ON note_relations(source_note_id);
CREATE INDEX IF NOT EXISTS idx_note_relations_target ON note_relations(target_note_id);
CREATE INDEX IF NOT EXISTS idx_note_relations_type   ON note_relations(relation_type);
