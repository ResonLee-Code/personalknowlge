-- 撤销只能回滚“接受建议后的那个版本”。记录接受后的完整文件哈希，
-- 防止用户在接受后继续编辑，再点旧建议的撤销时静默覆盖新内容。

ALTER TABLE ai_suggestions ADD COLUMN applied_hash TEXT;
