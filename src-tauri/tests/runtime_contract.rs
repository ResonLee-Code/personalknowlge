//! 启动前的静态契约：防止只在 `tauri build` 成功、运行到插件初始化才 panic。

use std::fs;

#[test]
fn updater_plugin_has_a_deserializable_runtime_config() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = fs::read_to_string(root.join("tauri.conf.json")).expect("读取 tauri.conf.json");
    let config: serde_json::Value = serde_json::from_str(&raw).expect("解析 tauri.conf.json");
    let updater = config
        .get("plugins")
        .and_then(|plugins| plugins.get("updater"))
        .expect("plugins.updater 必须存在；缺失会在启动时 panic");
    assert!(
        updater.get("pubkey").is_some_and(serde_json::Value::is_string),
        "plugins.updater.pubkey 必须可反序列化为字符串；运行时命令会另行注入发布公钥"
    );
    assert!(
        updater
            .get("endpoints")
            .is_some_and(serde_json::Value::is_array),
        "plugins.updater.endpoints 必须是数组"
    );
}

#[test]
fn window_bounds_restore_permissions_are_declared() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = fs::read_to_string(root.join("capabilities/default.json"))
        .expect("读取 capabilities/default.json");
    let config: serde_json::Value =
        serde_json::from_str(&raw).expect("解析 capabilities/default.json");
    let permissions = config
        .get("permissions")
        .and_then(serde_json::Value::as_array)
        .expect("capabilities.permissions 必须是数组")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    for required in [
        "core:window:allow-available-monitors",
        "core:window:allow-outer-position",
        "core:window:allow-outer-size",
        "core:window:allow-set-position",
        "core:window:allow-set-size",
    ] {
        assert!(
            permissions.contains(&required),
            "窗口尺寸/位置恢复缺少权限：{required}"
        );
    }
}
