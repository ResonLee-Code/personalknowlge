//! 时间工具。
//!
//! 设计文档 11.1：数据库统一保存 ISO 8601 且明确带时区的时间字符串。
//! 这里统一使用本地时区偏移（例如 `2026-09-23T12:00:00+08:00`），
//! 便于用户直接阅读 Markdown 中的 frontmatter。

use chrono::{DateTime, FixedOffset, Local, SecondsFormat, TimeZone, Utc};

/// 当前时间的 ISO 8601 字符串（带本地时区偏移）。
pub fn now_iso() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// 当前时间的 ISO 8601 字符串（UTC）。
pub fn now_utc_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// 解析 ISO 8601 时间字符串。
pub fn parse_iso(value: &str) -> Option<DateTime<FixedOffset>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Some(parsed);
    }
    // 兼容不带时区的 `2026-09-23T12:00:00`
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
        let offset = *Local::now().offset();
        return offset.from_local_datetime(&naive).single();
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let naive = date.and_hms_opt(0, 0, 0)?;
        let offset = *Local::now().offset();
        return offset.from_local_datetime(&naive).single();
    }
    None
}

/// 把毫秒时间戳转换为本地 ISO 8601。
pub fn millis_to_iso(millis: i64) -> String {
    match Local.timestamp_millis_opt(millis).single() {
        Some(dt) => dt.to_rfc3339_opts(SecondsFormat::Secs, false),
        None => now_iso(),
    }
}

/// 文件系统时间戳 -> ISO 8601。
pub fn system_time_to_iso(time: std::time::SystemTime) -> String {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => millis_to_iso(duration.as_millis() as i64),
        Err(_) => now_iso(),
    }
}

/// 用于文件名的紧凑时间戳：`20260923-120000`。
pub fn file_timestamp(dt: DateTime<Local>) -> String {
    dt.format("%Y%m%d-%H%M%S").to_string()
}

/// 今天的日期：`YYYY-MM-DD`（报告抬头「资料截至 YYYY-MM-DD」用）。
pub fn today_date() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

/// 从今天起偏移若干天的日期：`YYYY-MM-DD`（负数为过去）。
pub fn date_offset_days(days: i64) -> String {
    (Local::now().date_naive() + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

/// 两个 ISO/日期字符串之间的天数差（`later - earlier`）；任一无法解析时为 `None`。
pub fn days_between(earlier: &str, later: &str) -> Option<i64> {
    let start = parse_iso(earlier)?.date_naive();
    let end = parse_iso(later)?.date_naive();
    Some((end - start).num_days())
}

/// 当前本地时间，用于拼接文件名。
pub fn now_file_timestamp() -> String {
    file_timestamp(Local::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_iso_is_parseable_and_has_offset() {
        let value = now_iso();
        assert!(parse_iso(&value).is_some(), "{value} 应可解析");
        assert!(
            value.contains('+') || value.ends_with('Z'),
            "{value} 应带时区信息"
        );
    }

    #[test]
    fn parses_naive_and_date_only_values() {
        assert!(parse_iso("2026-09-23T12:00:00").is_some());
        assert!(parse_iso("2026-09-23").is_some());
        assert!(parse_iso("not-a-date").is_none());
    }

    #[test]
    fn file_timestamp_is_compact() {
        let stamp = now_file_timestamp();
        assert_eq!(stamp.len(), 15, "形如 20260923-120000");
        assert!(stamp.contains('-'));
    }
}
