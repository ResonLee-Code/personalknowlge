//! 内容哈希。用于判断文件是否真正发生变化，避免重复索引。

use sha2::{Digest, Sha256};

/// 计算字节内容的 SHA-256（十六进制小写）。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 计算字符串内容的 SHA-256。
pub fn sha256_hex_str(content: &str) -> String {
    sha256_hex(content.as_bytes())
}

/// 前 16 位短哈希，用于日志与文件名冲突检测。
pub fn short_hash(content: &str) -> String {
    sha256_hex_str(content).chars().take(16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex_str("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn different_content_gives_different_hash() {
        assert_ne!(sha256_hex_str("a"), sha256_hex_str("b"));
        assert_eq!(short_hash("hello").len(), 16);
    }
}
