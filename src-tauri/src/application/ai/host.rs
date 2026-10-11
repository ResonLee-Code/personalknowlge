//! URL 主机名解析（闸门与来源分级**共用一份实现**）。
//!
//! 为什么单独抽出来：域名白/黑名单、本地模型地址校验、来源分级都要「取主机名」，
//! 此前 `policy.rs` 与 `source_grade.rs` 各有一份几乎相同的私有实现——两处口径一旦漂移，
//! 「黑名单到底拦住了谁」就会变成说不清的事。
//!
//! 规则：小写；去掉 userinfo（`user@`）、端口与 `www.` 前缀；
//! IPv6 字面量去掉方括号（`[::1]:11434` → `::1`）。

/// 取 URL 或裸域名的主机名；解析不出（空串、只有 userinfo 等）返回 `None`。
pub fn host_of(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let rest = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let after_userinfo = authority.rsplit('@').next().unwrap_or("");

    let host = if let Some(inner) = after_userinfo.strip_prefix('[') {
        // IPv6 字面量：[::1]:11434 → ::1
        inner.split(']').next().unwrap_or("").to_string()
    } else {
        after_userinfo.split(':').next().unwrap_or("").to_string()
    };

    // 必须先小写再去 `www.`：`WWW.Example.com` 用大小写敏感的前缀匹配去不掉
    let lowered = host.trim().to_ascii_lowercase();
    let stripped = lowered.trim_start_matches("www.").to_string();
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_host_from_urls_and_bare_hosts() {
        assert_eq!(host_of("https://docs.example.com/a/b?c=1").as_deref(), Some("docs.example.com"));
        assert_eq!(host_of("http://127.0.0.1:11434/api").as_deref(), Some("127.0.0.1"));
        assert_eq!(host_of("HTTP://WWW.Example.com:443/x").as_deref(), Some("example.com"));
        assert_eq!(host_of("example.com").as_deref(), Some("example.com"));
        assert_eq!(host_of("user:pw@host.test/path").as_deref(), Some("host.test"));
        assert_eq!(host_of("").as_deref(), None);
        assert_eq!(host_of("   ").as_deref(), None);
    }

    #[test]
    fn ipv6_literals_keep_their_address_without_brackets() {
        assert_eq!(host_of("http://[::1]:11434/api").as_deref(), Some("::1"));
        assert_eq!(host_of("https://[2001:db8::1]/x").as_deref(), Some("2001:db8::1"));
        // 裸 IPv6（没有方括号）解析不出主机名：调用方按「不是域名」处理
        assert_eq!(host_of("::1").as_deref(), None);
    }
}
