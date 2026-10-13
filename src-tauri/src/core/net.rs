//! 出站 HTTP 的统一入口：超时 + **代理**（Track D4-④）。
//!
//! 为什么单独一层：真实环境里经常是"**只有浏览器上得了外网**"——VPN 按应用分流，
//! 系统里挂着本地代理（例如 `127.0.0.1:8118`），而 `ureq` **不会**自动读 macOS 的
//! 系统代理设置。于是会出现"Chrome 打得开、应用连不上"，且只在真机上才暴露。
//!
//! 解析顺序（先到先用）：
//! 1. **显式设置**（「设置 → AI → 出站代理」，给"自动探测不管用"的情况兜底）；
//! 2. **环境变量** `https_proxy` / `https_proxy` / `all_proxy`（大小写都认）；
//! 3. **macOS 系统代理**（`scutil --proxy`：HTTPS > HTTP > SOCKS）。
//!
//! 另一条纪律：**本机地址绝不走代理**——本地模型（Ollama）与本地嵌入服务就在本机，
//! 绕代理不仅没意义，还会把它们暴露给代理进程。

use std::time::Duration;

/// 目标地址是不是本机（回环 / localhost）：是的话不要走代理。
pub fn is_loopback_target(target: &str) -> bool {
    let Some(host) = host_of_url(target) else {
        return true; // 解析不出来的（相对路径等）一律当本机处理：宁可不用代理
    };
    let lowered = host.to_ascii_lowercase();
    lowered == "localhost"
        || lowered == "::1"
        || lowered == "[::1]"
        || lowered.starts_with("127.")
}

/// 从 URL 里取主机名（小写、无端口、无 userinfo）。解析不出来返回 `None`。
///
/// 刻意在 `core` 里自带一份：`core` 不依赖 `application`，而这里只需要"够用"的解析。
fn host_of_url(target: &str) -> Option<String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    let rest = trimmed.split_once("://").map(|(_, rest)| rest).unwrap_or(trimmed);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let after_userinfo = authority.rsplit('@').next().unwrap_or("");
    let host = if let Some(inner) = after_userinfo.strip_prefix('[') {
        inner.split(']').next().unwrap_or("").to_string()
    } else {
        after_userinfo.split(':').next().unwrap_or("").to_string()
    };
    let host = host.trim().to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// 生效的代理地址（形如 `http://127.0.0.1:8118` / `socks5h://127.0.0.1:8119`）。
pub fn resolve_proxy(target: &str, explicit: Option<&str>) -> Option<String> {
    if is_loopback_target(target) {
        return None;
    }
    if let Some(value) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(value.to_string());
    }
    if let Some(value) = proxy_from_env() {
        return Some(value);
    }
    system_proxy()
}

fn proxy_from_env() -> Option<String> {
    const KEYS: [&str; 8] = [
        "https_proxy",
        "HTTPS_PROXY",
        "http_proxy",
        "HTTP_PROXY",
        "all_proxy",
        "ALL_PROXY",
        "ALL_PROXY",
        "https_proxy",
    ];
    for key in KEYS {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// 系统代理（目前实现 macOS；其它平台返回 `None`，交给环境变量或显式设置）。
///
/// 只探一次并缓存：`scutil` 是一次子进程调用，不该每个请求都跑。
fn system_proxy() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        static CACHE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        CACHE
            .get_or_init(|| {
                let output = std::process::Command::new("scutil")
                    .arg("--proxy")
                    .output()
                    .ok()?;
                parse_scutil_proxy(&String::from_utf8_lossy(&output.stdout))
            })
            .clone()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// 解析 `scutil --proxy` 的输出（HTTPS > HTTP > SOCKS）。
fn parse_scutil_proxy(text: &str) -> Option<String> {
    let get = |key: &str| -> Option<String> {
        text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_string())
        })
    };
    let enabled = |key: &str| get(key).map(|value| value == "1").unwrap_or(false);

    for (flag, host_key, port_key, scheme) in [
        ("HTTPSEnable", "HTTPSProxy", "HTTPSPort", "http"),
        ("HTTPEnable", "HTTPProxy", "HTTPPort", "http"),
        ("SOCKSEnable", "SOCKSProxy", "SOCKSPort", "socks5h"),
    ] {
        if enabled(flag) {
            if let (Some(host), Some(port)) = (get(host_key), get(port_key)) {
                // 系统代理自己也指向本机时仍然可用（代理进程就在本机），所以这里不套 loopback 判断
                return Some(format!("{scheme}://{host}:{port}"));
            }
        }
    }
    None
}

/// 构造统一的 ureq agent：超时 + （必要时）代理。
///
/// 代理串不合法（写错了）时**退回直连**并记一条 warn——总比整个功能不可用强。
pub fn agent_for(target: &str, timeout: Duration, explicit_proxy: Option<&str>) -> ureq::Agent {
    let mut builder = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // **别把 4xx/5xx 当传输错误抛**：我们要按状态码给出人能看懂的原因
        // （401 = Key 无效、429 = 限流要等几秒…）。ureq 默认会抛，导致这些分支永远走不到。
        .http_status_as_error(false);
    if let Some(raw) = resolve_proxy(target, explicit_proxy) {
        match ureq::Proxy::new(&raw) {
            Ok(proxy) => builder = builder.proxy(Some(proxy)),
            Err(error) => {
                tracing::warn!(target: "net", "代理地址不可用（{raw}）：{error}，本次直连");
            }
        }
    }
    ureq::Agent::new_with_config(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_targets_never_use_a_proxy() {
        for target in [
            "http://127.0.0.1:11434/api/tags",
            "http://localhost:11434",
            "http://[::1]:8080",
        ] {
            assert!(
                resolve_proxy(target, Some("http://127.0.0.1:8118")).is_none(),
                "本机地址不该走代理：{target}"
            );
        }
        assert!(is_loopback_target("http://127.0.0.1:1234/v1"));
        assert!(!is_loopback_target("https://api.perplexity.ai/v1/decisions"));
    }

    #[test]
    fn explicit_setting_wins_over_env_and_system() {
        // 环境变量在测试进程里可能是任意值，这里只断言"显式设置优先"这一点
        let resolved = resolve_proxy("https://api.example.com/v1", Some("http://127.0.0.1:9999"));
        assert_eq!(resolved.as_deref(), Some("http://127.0.0.1:9999"));
    }

    #[test]
    fn scutil_output_is_parsed_in_priority_order() {
        let sample = "  HTTPEnable : 1\n  HTTPPort : 8118\n  HTTPProxy : 127.0.0.1\n  \
                      HTTPSEnable : 1\n  HTTPSPort : 8118\n  HTTPSProxy : 127.0.0.1\n  \
                      SOCKSEnable : 1\n  SOCKSPort : 8119\n  SOCKSProxy : 127.0.0.1\n";
        assert_eq!(parse_scutil_proxy(sample).as_deref(), Some("http://127.0.0.1:8118"));

        // 只有 SOCKS 时退回 socks5h
        let socks_only = "  HTTPEnable : 0\n  SOCKSEnable : 1\n  SOCKSPort : 8119\n  SOCKSProxy : 127.0.0.1\n";
        assert_eq!(parse_scutil_proxy(socks_only).as_deref(), Some("socks5h://127.0.0.1:8119"));

        // 全关：没有代理
        assert_eq!(parse_scutil_proxy("  HTTPEnable : 0\n"), None);
    }
}
