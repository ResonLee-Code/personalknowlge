//! 分类查找辅助：路径、目录与树结构。

use std::collections::{HashMap, HashSet};

use crate::domain::category::{Category, CategoryNode};

/// 分类索引，避免在列表渲染时反复递归查询数据库。
pub struct CategoryLookup {
    by_id: HashMap<String, Category>,
    children: HashMap<String, Vec<String>>,
    roots: Vec<String>,
}

impl CategoryLookup {
    pub fn new(categories: Vec<Category>) -> Self {
        let mut by_id = HashMap::new();
        for category in categories {
            by_id.insert(category.id.clone(), category);
        }

        let mut ids: Vec<String> = by_id.keys().cloned().collect();
        ids.sort_by_key(|id| {
            by_id
                .get(id)
                .map(|category| (category.sort_order, category.name.clone()))
                .unwrap_or((0, String::new()))
        });

        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        let mut roots = Vec::new();
        for id in ids {
            let parent = by_id.get(&id).and_then(|category| category.parent_id.clone());
            match parent {
                Some(parent) if by_id.contains_key(&parent) => {
                    children.entry(parent).or_default().push(id);
                }
                _ => roots.push(id),
            }
        }

        Self { by_id, children, roots }
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Category> {
        self.by_id.get(id)
    }

    pub fn all(&self) -> Vec<Category> {
        let mut list: Vec<Category> = self.by_id.values().cloned().collect();
        list.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.name.cmp(&b.name)));
        list
    }

    /// 从根到该分类的名称路径。
    pub fn path_names(&self, id: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut current = Some(id.to_string());
        let mut guard = HashSet::new();
        while let Some(category_id) = current {
            if !guard.insert(category_id.clone()) {
                break;
            }
            let Some(category) = self.by_id.get(&category_id) else {
                break;
            };
            names.push(category.name.clone());
            current = category.parent_id.clone();
        }
        names.reverse();
        names
    }

    /// `技术/数据库` 形式的分类路径。
    pub fn path_string(&self, id: &str) -> String {
        self.path_names(id).join("/")
    }

    /// 分类对应的目录（工作区相对路径）。
    ///
    /// 若某一级没有目录（创建时选择不建目录），则退回到最近的有效目录；
    /// 完全没有目录时使用 Inbox。
    pub fn directory_of(&self, id: &str) -> String {
        let mut segments: Vec<String> = Vec::new();
        let mut current = Some(id.to_string());
        let mut guard = HashSet::new();

        while let Some(category_id) = current {
            if !guard.insert(category_id.clone()) {
                break;
            }
            let Some(category) = self.by_id.get(&category_id) else {
                break;
            };
            if category.dir_name.trim().is_empty() {
                break;
            }
            segments.push(category.dir_name.clone());
            current = category.parent_id.clone();
        }
        segments.reverse();

        if segments.is_empty() {
            crate::core::paths::INBOX_DIR.to_string()
        } else {
            segments.join("/")
        }
    }

    /// 反向查找：目录 -> 分类 ID。
    pub fn category_by_directory(&self, directory: &str) -> Option<String> {
        let target = directory.trim_matches('/');
        if target.is_empty() {
            return None;
        }
        self.by_id
            .values()
            .find(|category| self.directory_of(&category.id) == target)
            .map(|category| category.id.clone())
    }

    /// 构建分类树，并填充笔记数量。
    pub fn tree(&self, note_counts: &HashMap<String, i64>) -> Vec<CategoryNode> {
        self.build_nodes(&self.roots, 0, note_counts)
    }

    fn build_nodes(
        &self,
        ids: &[String],
        depth: usize,
        note_counts: &HashMap<String, i64>,
    ) -> Vec<CategoryNode> {
        let mut nodes = Vec::new();
        for id in ids {
            let Some(category) = self.by_id.get(id) else {
                continue;
            };
            let mut category = category.clone();
            category.note_count = note_counts.get(id).copied().unwrap_or(0);
            let children = self
                .children
                .get(id)
                .map(|children| self.build_nodes(children, depth + 1, note_counts))
                .unwrap_or_default();
            nodes.push(CategoryNode { category, depth, children });
        }
        nodes
    }
}
