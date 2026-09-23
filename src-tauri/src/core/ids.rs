//! ID 生成。
//!
//! 设计文档 12.3 使用 `note_01JXYZ123` 形式的 ID，即 ULID。
//! ULID = 48 位毫秒时间戳 + 80 位随机数，Crockford Base32 编码为 26 个字符，
//! 因此天然按时间有序，便于排序与调试。

use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 80 位随机部分的最大值。
const MAX_ENTROPY: u128 = (1u128 << 80) - 1;

/// 上一次生成的时间戳与随机数，用于保证同一毫秒内单调递增。
fn state() -> &'static Mutex<(u64, u128)> {
    static STATE: OnceLock<Mutex<(u64, u128)>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new((0, 0)))
}

/// 生成 26 位 ULID。
///
/// 同一毫秒内多次调用时，随机部分递增，保证生成的 ID 严格单调，
/// 因此可以直接按字符串排序得到时间顺序。
pub fn ulid() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let (timestamp, entropy) = {
        let mut guard = state().lock().unwrap_or_else(|error| error.into_inner());
        let (last_millis, last_entropy) = *guard;
        let next = if millis > last_millis {
            (millis, random_entropy())
        } else {
            // 时钟回拨或同一毫秒：在上一个值基础上递增
            let entropy = (last_entropy.wrapping_add(1)) & MAX_ENTROPY;
            (last_millis, if entropy == 0 { 1 } else { entropy })
        };
        *guard = next;
        next
    };

    let mut out = [0u8; 26];

    // 时间戳：48 位 -> 10 个字符
    let mut timestamp_value = timestamp & 0xFFFF_FFFF_FFFF;
    for index in (0..10).rev() {
        out[index] = CROCKFORD[(timestamp_value & 0x1F) as usize];
        timestamp_value >>= 5;
    }

    // 随机部分：80 位 -> 16 个字符
    let mut random = entropy & MAX_ENTROPY;
    for index in (10..26).rev() {
        out[index] = CROCKFORD[(random & 0x1F) as usize];
        random >>= 5;
    }

    String::from_utf8_lossy(&out).into_owned()
}

fn random_entropy() -> u128 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    let mut value: u128 = 0;
    for byte in bytes.iter().take(10) {
        value = (value << 8) | u128::from(*byte);
    }
    value & MAX_ENTROPY
}

pub fn note_id() -> String {
    format!("note_{}", ulid())
}

pub fn category_id() -> String {
    format!("cat_{}", ulid())
}

pub fn tag_id() -> String {
    format!("tag_{}", ulid())
}

pub fn workspace_id() -> String {
    format!("ws_{}", ulid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ulid_has_expected_shape_and_charset() {
        let id = ulid();
        assert_eq!(id.len(), 26, "ULID 长度必须是 26");
        assert!(
            id.chars().all(|c| CROCKFORD.contains(&(c as u8))),
            "ULID 只能使用 Crockford Base32 字符集：{id}"
        );
    }

    #[test]
    fn ulids_are_unique_and_time_ordered() {
        let mut seen = HashSet::new();
        let mut previous = String::new();
        for _ in 0..200 {
            let id = ulid();
            assert!(seen.insert(id.clone()), "ULID 必须唯一");
            assert!(id >= previous, "ULID 必须按时间递增");
            previous = id;
        }
    }

    #[test]
    fn prefixed_ids_keep_prefix() {
        assert!(note_id().starts_with("note_"));
        assert!(category_id().starts_with("cat_"));
        assert!(tag_id().starts_with("tag_"));
        assert!(workspace_id().starts_with("ws_"));
    }
}
