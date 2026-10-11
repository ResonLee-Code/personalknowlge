//! 文本字节解码（导入外部文件 / 抓取网页共用一份实现）。
//!
//! 背景：中文用户手里的文件与网页并不总是 UTF-8——Windows 记事本、Excel 另存的
//! `.txt` / `.csv` 常是 GBK，老站点也常在 `<meta charset>` 里写 gbk。
//! 直接 `String::from_utf8_lossy` 会**静默**把整篇文本变成乱码（比报错更糟）。
//!
//! 解码顺序（保守、可解释）：BOM → 严格 UTF-8 → GBK。
//! GBK 解码用 `encoding_rs`——它本来就在依赖树里（`ureq` 的 charset 特性带入），
//! 因此这不引入新的第三方代码。

/// 解码一段文本；第二个返回值是**兜底编码名**（用上了才非空，供上层提示用户）。
pub fn decode_text(bytes: &[u8]) -> (String, Option<&'static str>) {
    if let Some((encoding, _)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, _, _) = encoding.decode(bytes);
        let label = (encoding != encoding_rs::UTF_8).then_some(encoding.name());
        return (text.into_owned(), label);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_string(), None),
        Err(_) => {
            // 严格 UTF-8 失败：按 GBK 解（中文网页与中文文本文件最常见的非 UTF-8 编码）。
            // 真解错时正文里会出现替换字符——那是「这里可能解错了」的可见信号，不会静默糊掉。
            let (text, _, _) = encoding_rs::GBK.decode(bytes);
            (text.into_owned(), Some(encoding_rs::GBK.name()))
        }
    }
}

/// 从 `Content-Type` 里取 charset（大小写不敏感，支持带引号写法）。
pub fn charset_from_content_type(content_type: &str) -> Option<String> {
    let lowered = content_type.to_ascii_lowercase();
    let (_, rest) = lowered.split_once("charset=")?;
    let label = rest
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(['"', '\''])
        .to_string();
    (!label.is_empty()).then_some(label)
}

/// 从前 4 KB 里找 HTML 的 `<meta charset=...>`（也覆盖 `content="text/html; charset=..."`）。
pub fn charset_from_meta(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(4096)];
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    let index = text.find("charset=")?;
    let rest = text[index + "charset=".len()..].trim_start();
    // 值可能带引号：`<meta charset="gbk">`
    let rest = rest.trim_start_matches(['"', '\'']);
    let label: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    (!label.is_empty()).then_some(label)
}

/// 用指定编码解码；`label` 不认识时返回 `None`（调用方继续走别的兜底）。
/// `Content-Type` 里是否声明了**非 UTF-8** 的 charset。
///
/// 与 `ureq` 内部同一套判断（它只在此时才会替我们把正文转成 UTF-8）：
/// 抓网页的分支要靠它决定「拿到的是已解码文本还是原始字节」。
pub fn charset_declared_non_utf8(content_type: &str) -> bool {
    charset_from_content_type(content_type)
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .is_some_and(|encoding| encoding != encoding_rs::UTF_8)
}

pub fn decode_with(bytes: &[u8], label: &str) -> Option<(String, &'static str)> {
    let encoding = encoding_rs::Encoding::for_label(label.as_bytes())?;
    let (text, _, _) = encoding.decode(bytes);
    let used = (encoding != encoding_rs::UTF_8).then_some(encoding.name());
    Some((text.into_owned(), used.unwrap_or("UTF-8")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 「标题：中文内容」的 GBK 字节（B1 EA CC E2 A3 BA D6 D0 CE C4 C4 DA C8 DD）。
    const GBK_SAMPLE: &[u8] = &[
        0xB1, 0xEA, 0xCC, 0xE2, 0xA3, 0xBA, 0xD6, 0xD0, 0xCE, 0xC4, 0xC4, 0xDA, 0xC8, 0xDD,
    ];

    #[test]
    fn decodes_utf8_untouched() {
        let (text, fallback) = decode_text("标题：中文内容".as_bytes());
        assert_eq!(text, "标题：中文内容");
        assert!(fallback.is_none(), "UTF-8 不需要兜底");
    }

    #[test]
    fn falls_back_to_gbk_and_says_so() {
        let (text, fallback) = decode_text(GBK_SAMPLE);
        assert_eq!(text, "标题：中文内容", "GBK 字节要解成正确的中文");
        assert_eq!(fallback, Some("GBK"), "用上了兜底编码要如实报出来");
    }

    #[test]
    fn honours_bom_and_declared_labels() {
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice("标题".as_bytes());
        assert_eq!(decode_text(&with_bom).0, "标题");

        assert_eq!(
            decode_with(GBK_SAMPLE, "gbk").map(|(text, _)| text),
            Some("标题：中文内容".to_string())
        );
        assert!(decode_with(GBK_SAMPLE, "不存在的编码").is_none());
    }

    #[test]
    fn sniffs_charset_from_content_type_and_meta() {
        assert_eq!(
            charset_from_content_type("text/html; charset=GBK"),
            Some("gbk".to_string())
        );
        assert_eq!(charset_from_content_type("text/html"), None);
        assert_eq!(
            charset_from_meta(br#"<html><head><meta charset="gbk">"#),
            Some("gbk".to_string())
        );
    }
}
