//! 通用基础能力：错误、ID、哈希、时间、文本、路径、日志、事件。
//!
//! 该层不依赖任何业务模块，可被 domain / infrastructure / application 复用。

pub mod error;
pub mod events;
pub mod hash;
pub mod ids;
pub mod logging;
pub mod paths;
pub mod text;
pub mod time;
