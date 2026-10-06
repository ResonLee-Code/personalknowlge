//! 极简 Base64 编解码。
//!
//! 仅用于「编辑器粘贴图片 → 落盘到 attachments/」这一处需求，
//! 因此不引入额外依赖，代码量控制在可审计范围内（标准字母表 + `=` 填充）。

use crate::core::error::{AppError, ErrorCode, Result};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn invalid() -> AppError {
    AppError::new(ErrorCode::InvalidInput, "附件数据不是合法的 Base64 字符串。")
}

/// 编码为 Base64（无换行）。
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

/// 解码 Base64；忽略换行与空格，遇到 `=` 视作结束。
pub fn decode(input: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;

    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            // 填充字符之后不再有有效数据
            b'=' => break,
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return Err(invalid()),
        } as u32;

        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1u32 << bits) - 1;
        }
    }

    Ok(out)
}

/// 去掉 `data:image/png;base64,` 之类的前缀，返回纯 Base64 与 MIME 类型。
pub fn split_data_url(value: &str) -> (Option<String>, &str) {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("data:") {
        if let Some((meta, payload)) = rest.split_once(',') {
            let mime = meta
                .split(';')
                .next()
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string);
            return (mime, payload);
        }
    }
    (None, trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_round_trip() {
        for case in [
            "",
            "f",
            "fo",
            "foo",
            "foob",
            "fooba",
            "foobar",
            "中文附件内容",
        ] {
            let encoded = encode(case.as_bytes());
            assert_eq!(decode(&encoded).unwrap(), case.as_bytes(), "case={case}");
        }
    }

    #[test]
    fn matches_known_vectors() {
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn ignores_whitespace_and_rejects_invalid_characters() {
        assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert_eq!(decode(" Zm9vYmFy ").unwrap(), b"foobar");
        assert!(decode("Zm9v*YmFy").is_err());
    }

    #[test]
    fn strips_data_url_prefix() {
        let (mime, payload) = split_data_url("data:image/png;base64,Zm9v");
        assert_eq!(mime.as_deref(), Some("image/png"));
        assert_eq!(payload, "Zm9v");

        let (mime, payload) = split_data_url("Zm9v");
        assert!(mime.is_none());
        assert_eq!(payload, "Zm9v");
    }
}
