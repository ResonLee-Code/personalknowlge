-- 005_note_file_stamp.sql
-- 记录笔记文件的「mtime + size」快照，用于扫描时短路：
-- 未变化的文件无需读取全文并计算 sha256（1 万篇规模下这一步是扫描的主要成本）。

ALTER TABLE notes ADD COLUMN file_size INTEGER;
ALTER TABLE notes ADD COLUMN file_mtime TEXT;
