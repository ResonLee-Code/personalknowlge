//! 基础设施层：数据库、Markdown 存储、搜索索引、文件监听、备份。

pub mod backup;
pub mod db;
pub mod docx;
pub mod fts;
pub mod frontmatter;
pub mod markdown_storage;
pub mod watcher;
