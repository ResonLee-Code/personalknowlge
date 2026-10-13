//! 本机凭据存放（在线模型的 API Key、决策模型的 API Key）。
//!
//! 为什么单独放一个文件，而不写进工作区设置：
//! - **不进工作区** → 不会被备份包 / 整库导出 / Markdown 带走（备份里出现 API Key 是事故）；
//! - **不进前端的策略对象** → 界面只回显「已配置 / 未配置」（`remoteModelKeySet`），
//!   Key 本身不会出现在 IPC 响应、日志或审计里；
//! - 文件权限在 Unix 上收紧到 `0600`。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::error::Result;

/// 文件名（位于应用数据目录，与 `workspaces.json` 同级）。
pub const CREDENTIAL_FILE: &str = "ai-credentials.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Credentials {
    remote_model_api_key: Option<String>,
    /// 决策模型（TypeSafe Jev / Perplexity Decisions）的 API Key
    decision_api_key: Option<String>,
}

fn path_of(data_dir: &Path) -> PathBuf {
    data_dir.join(CREDENTIAL_FILE)
}

fn load(data_dir: &Path) -> Credentials {
    std::fs::read_to_string(path_of(data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str::<Credentials>(&raw).ok())
        .unwrap_or_default()
}

/// 读取在线模型 API Key；未配置返回 `None`。
pub fn remote_model_key(data_dir: &Path) -> Option<String> {
    load(data_dir)
        .remote_model_api_key
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

/// 是否已配置 Key（供界面显示，**不回显 Key 本身**）。
pub fn remote_model_key_set(data_dir: &Path) -> bool {
    remote_model_key(data_dir).is_some()
}

/// 写入 / 清除在线模型 API Key（`None` 或空串表示清除）。
pub fn set_remote_model_key(data_dir: &Path, key: Option<&str>) -> Result<()> {
    let mut credentials = load(data_dir);
    credentials.remote_model_api_key = key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    persist(data_dir, &credentials)
}

/// 读取决策模型 API Key；未配置返回 `None`。
pub fn decision_key(data_dir: &Path) -> Option<String> {
    load(data_dir)
        .decision_api_key
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

/// 是否已配置决策模型 Key（**不回显 Key 本身**）。
pub fn decision_key_set(data_dir: &Path) -> bool {
    decision_key(data_dir).is_some()
}

/// 写入 / 清除决策模型 API Key（`None` 或空串表示清除）。
pub fn set_decision_key(data_dir: &Path, key: Option<&str>) -> Result<()> {
    let mut credentials = load(data_dir);
    credentials.decision_api_key = key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    persist(data_dir, &credentials)
}

/// 落盘：**两个 Key 都没有时把文件也删掉**，不留空壳，并收紧权限到 `0600`。
fn persist(data_dir: &Path, credentials: &Credentials) -> Result<()> {
    let path = path_of(data_dir);
    if credentials.remote_model_api_key.is_none() && credentials.decision_api_key.is_none() {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&credentials)?;
    std::fs::write(&path, text)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // 只有本用户可读写（凭据不该被同机其它账号读到）
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, permissions)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_clears_the_key_outside_the_workspace() {
        let dir = tempfile::tempdir().expect("临时目录");
        assert!(!remote_model_key_set(dir.path()));
        assert!(remote_model_key(dir.path()).is_none());

        set_remote_model_key(dir.path(), Some("  sk-test-123  ")).expect("写入凭据");
        assert!(remote_model_key_set(dir.path()));
        assert_eq!(remote_model_key(dir.path()).as_deref(), Some("sk-test-123"));

        // 清除后文件都不留
        set_remote_model_key(dir.path(), None).expect("清除凭据");
        assert!(!remote_model_key_set(dir.path()));
        assert!(!dir.path().join(CREDENTIAL_FILE).exists());

        // 空串等价于清除
        set_remote_model_key(dir.path(), Some("sk-2")).expect("写入");
        set_remote_model_key(dir.path(), Some("   ")).expect("空白视为清除");
        assert!(!remote_model_key_set(dir.path()));
    }

    #[test]
    fn decision_key_is_independent_from_the_remote_model_key() {
        let dir = tempfile::tempdir().expect("临时目录");
        set_remote_model_key(dir.path(), Some("sk-remote")).expect("写在线模型 Key");
        set_decision_key(dir.path(), Some(" pplx-test ")).expect("写决策 Key");

        assert_eq!(decision_key(dir.path()).as_deref(), Some("pplx-test"));
        assert!(decision_key_set(dir.path()));
        assert_eq!(remote_model_key(dir.path()).as_deref(), Some("sk-remote"), "互不影响");

        // 清掉决策 Key 后，在线模型的 Key 还在 → 文件不能删
        set_decision_key(dir.path(), None).expect("清决策 Key");
        assert!(!decision_key_set(dir.path()));
        assert_eq!(remote_model_key(dir.path()).as_deref(), Some("sk-remote"));
        assert!(dir.path().join(CREDENTIAL_FILE).exists());

        // 两个都清掉 → 文件也不留
        set_remote_model_key(dir.path(), None).expect("清在线模型 Key");
        assert!(!dir.path().join(CREDENTIAL_FILE).exists());
    }
}
