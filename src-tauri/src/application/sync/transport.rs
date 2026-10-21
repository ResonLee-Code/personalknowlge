//! WebDAV / S3 兼容传输层。
//!
//! 上层只认对象 key 与 ETag；响应错误不包含口令、账号或对象内容。

use std::time::Duration;

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::core::base64;
use crate::core::error::{AppError, ErrorCode, Result};

const TIMEOUT_SECS: u64 = 60;
#[derive(Debug, Clone)]
pub struct RemoteObject {
    pub bytes: Vec<u8>,
    pub etag: Option<String>,
}

/// 同步对象存储。读取不存在的对象返回 `None`，网络 / 认证错误必须显式返回。
pub trait SyncTransport: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<RemoteObject>>;
    /// `if_match` 非空时要求服务端进行乐观并发检查。
    /// `if_none_match=true` 时只在对象不存在时创建。
    fn put(
        &self,
        key: &str,
        bytes: &[u8],
        if_match: Option<&str>,
        if_none_match: bool,
    ) -> Result<Option<String>>;
    /// 保留物理删除能力；当前同步删除使用 manifest 墓碑，避免离线设备丢数据。
    #[allow(dead_code)]
    fn delete(&self, key: &str, if_match: Option<&str>) -> Result<bool>;
}

pub struct WebDavTransport {
    base_url: String,
    username: String,
    password: String,
}

impl WebDavTransport {
    pub fn new(base_url: &str, username: &str, password: &str) -> Self {
        Self {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    fn url(&self, key: &str) -> String {
        format!("{}/{}", self.base_url, encode_key(key))
    }

    fn agent(&self) -> ureq::Agent {
        crate::core::net::agent_for(&self.base_url, Duration::from_secs(TIMEOUT_SECS), None)
    }

    fn authorization(&self) -> String {
        format!(
            "Basic {}",
            base64::encode(format!("{}:{}", self.username, self.password).as_bytes())
        )
    }

    fn ensure_parents(&self, key: &str) -> Result<()> {
        let parts: Vec<&str> = key.split('/').filter(|part| !part.is_empty()).collect();
        if parts.len() < 2 {
            return Ok(());
        }
        let agent = self.agent();
        for depth in 1..parts.len() {
            let url = format!(
                "{}/{}",
                self.base_url,
                parts[..depth]
                    .iter()
                    .map(|part| encode_segment(part))
                    .collect::<Vec<_>>()
                    .join("/")
            );
            let request = ureq::http::Request::builder()
                .method("MKCOL")
                .uri(&url)
                .header("Authorization", self.authorization())
                .body(Vec::<u8>::new())
                .map_err(|error| sync_error(format!("构造 WebDAV MKCOL 请求失败：{error}")))?;
            let request = agent
                .configure_request(request)
                .allow_non_standard_methods(true)
                .build();
            let response = agent
                .run(request)
                .map_err(|error| sync_error(format!("WebDAV 建目录失败：{error}")))?;
            let status = response.status().as_u16();
            if !matches!(status, 200 | 201 | 204 | 301 | 405) {
                return Err(sync_error(format!(
                    "WebDAV 建目录返回 HTTP {status}；请确认目录可写且账号有创建权限。"
                )));
            }
        }
        Ok(())
    }

    fn request(
        &self,
        method: &str,
        key: &str,
        body: Vec<u8>,
        if_match: Option<&str>,
        if_none_match: bool,
    ) -> Result<ureq::http::Response<ureq::Body>> {
        let mut builder = ureq::http::Request::builder()
            .method(method)
            .uri(self.url(key))
            .header("Authorization", self.authorization())
            .header("Accept", "application/octet-stream");
        if let Some(etag) = if_match {
            builder = builder.header("If-Match", etag);
        }
        if if_none_match {
            builder = builder.header("If-None-Match", "*");
        }
        let request = builder
            .body(body)
            .map_err(|error| sync_error(format!("构造 WebDAV 请求失败：{error}")))?;
        self.agent()
            .run(request)
            .map_err(|error| sync_error(format!("WebDAV {method} 失败：{error}")))
    }
}

impl SyncTransport for WebDavTransport {
    fn get(&self, key: &str) -> Result<Option<RemoteObject>> {
        let response = self.request("GET", key, Vec::new(), None, false)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        if status >= 300 {
            return Err(sync_error(format!(
                "WebDAV 下载返回 HTTP {status}；请检查地址、账号与目录权限。"
            )));
        }
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bytes = response
            .into_body()
            .read_to_vec()
            .map_err(|error| sync_error(format!("读取 WebDAV 响应失败：{error}")))?;
        Ok(Some(RemoteObject { bytes, etag }))
    }

    fn put(
        &self,
        key: &str,
        bytes: &[u8],
        if_match: Option<&str>,
        if_none_match: bool,
    ) -> Result<Option<String>> {
        self.ensure_parents(key)?;
        let response = self.request("PUT", key, bytes.to_vec(), if_match, if_none_match)?;
        let status = response.status().as_u16();
        if status == 412 {
            return Err(AppError::new(
                ErrorCode::SyncConflict,
                "远端 manifest 已被另一台设备更新；本次没有覆盖，请重新预览。",
            ));
        }
        if status >= 300 {
            return Err(sync_error(format!(
                "WebDAV 上传返回 HTTP {status}；请检查剩余空间与写入权限。"
            )));
        }
        Ok(response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string))
    }

    fn delete(&self, key: &str, if_match: Option<&str>) -> Result<bool> {
        let response = self.request("DELETE", key, Vec::new(), if_match, false)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Ok(false);
        }
        if status == 412 {
            return Err(AppError::new(
                ErrorCode::SyncConflict,
                "远端对象在删除前已变化；本次删除已取消。",
            ));
        }
        if status >= 300 {
            return Err(sync_error(format!("WebDAV 删除返回 HTTP {status}。")));
        }
        Ok(true)
    }
}

pub struct S3Transport {
    endpoint: String,
    bucket: String,
    region: String,
    access_key: String,
    secret_key: String,
}

impl S3Transport {
    pub fn new(
        endpoint: &str,
        bucket: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
    ) -> Self {
        Self {
            endpoint: endpoint.trim().trim_end_matches('/').to_string(),
            bucket: bucket.trim().to_string(),
            region: region.trim().to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
        }
    }

    fn url_and_path(&self, key: &str) -> (String, String) {
        let encoded_bucket = encode_segment(&self.bucket);
        let encoded_key = encode_key(key);
        let endpoint_rest = self
            .endpoint
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or(&self.endpoint);
        let endpoint_path = endpoint_rest
            .split_once('/')
            .map(|(_, path)| format!("/{}", path.trim_start_matches('/')))
            .filter(|path| path != "/")
            .unwrap_or_default();
        let path = format!("{endpoint_path}/{encoded_bucket}/{encoded_key}");
        (
            format!(
                "{}{}",
                self.endpoint,
                path.strip_prefix(&endpoint_path).unwrap_or(&path)
            ),
            path,
        )
    }

    fn host(&self) -> Result<String> {
        let without_scheme = self
            .endpoint
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or(&self.endpoint);
        let host = without_scheme
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .trim();
        if host.is_empty() {
            Err(sync_error("S3 endpoint 缺少主机名。"))
        } else {
            Ok(host.to_string())
        }
    }

    fn request(
        &self,
        method: &str,
        key: &str,
        body: Vec<u8>,
        if_match: Option<&str>,
        if_none_match: bool,
    ) -> Result<ureq::http::Response<ureq::Body>> {
        let (url, path) = self.url_and_path(key);
        let host = self.host()?;
        let timestamp = Utc::now();
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
        let date = timestamp.format("%Y%m%d").to_string();
        let payload_hash = sha256_hex(&body);
        let authorization = self.authorization(
            method,
            &path,
            &host,
            &amz_date,
            &date,
            &payload_hash,
            if_match,
            if_none_match,
        );
        let mut builder = ureq::http::Request::builder()
            .method(method)
            .uri(url)
            .header("Host", host)
            .header("x-amz-content-sha256", payload_hash)
            .header("x-amz-date", amz_date)
            .header("Authorization", authorization);
        if let Some(etag) = if_match {
            builder = builder.header("If-Match", etag);
        }
        if if_none_match {
            builder = builder.header("If-None-Match", "*");
        }
        let request = builder
            .body(body)
            .map_err(|error| sync_error(format!("构造 S3 请求失败：{error}")))?;
        crate::core::net::agent_for(&self.endpoint, Duration::from_secs(TIMEOUT_SECS), None)
            .run(request)
            .map_err(|error| sync_error(format!("S3 {method} 失败：{error}")))
    }

    #[allow(clippy::too_many_arguments)]
    fn authorization(
        &self,
        method: &str,
        path: &str,
        host: &str,
        amz_date: &str,
        date: &str,
        payload_hash: &str,
        if_match: Option<&str>,
        if_none_match: bool,
    ) -> String {
        let mut headers: Vec<(&str, String)> = vec![
            ("host", host.to_string()),
            ("x-amz-content-sha256", payload_hash.to_string()),
            ("x-amz-date", amz_date.to_string()),
        ];
        if let Some(etag) = if_match {
            headers.push(("if-match", etag.to_string()));
        }
        if if_none_match {
            headers.push(("if-none-match", "*".to_string()));
        }
        headers.sort_by(|left, right| left.0.cmp(right.0));
        let signed_headers = headers
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(";");
        let canonical_headers = headers
            .iter()
            .map(|(name, value)| format!("{name}:{value}\n"))
            .collect::<String>();
        let canonical_request =
            format!("{method}\n{path}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
        let scope = format!("{date}/{}/s3/aws4_request", self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );
        let date_key = hmac_sha256(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date.as_bytes(),
        );
        let region_key = hmac_sha256(&date_key, self.region.as_bytes());
        let service_key = hmac_sha256(&region_key, b"s3");
        let signing_key = hmac_sha256(&service_key, b"aws4_request");
        let signature = hex(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key
        )
    }
}

impl SyncTransport for S3Transport {
    fn get(&self, key: &str) -> Result<Option<RemoteObject>> {
        let response = self.request("GET", key, Vec::new(), None, false)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        if status >= 300 {
            return Err(sync_error(format!(
                "S3 下载返回 HTTP {status}；请检查 endpoint、bucket、region 与密钥。"
            )));
        }
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bytes = response
            .into_body()
            .read_to_vec()
            .map_err(|error| sync_error(format!("读取 S3 响应失败：{error}")))?;
        Ok(Some(RemoteObject { bytes, etag }))
    }

    fn put(
        &self,
        key: &str,
        bytes: &[u8],
        if_match: Option<&str>,
        if_none_match: bool,
    ) -> Result<Option<String>> {
        let response = self.request("PUT", key, bytes.to_vec(), if_match, if_none_match)?;
        let status = response.status().as_u16();
        if status == 412 {
            return Err(AppError::new(
                ErrorCode::SyncConflict,
                "远端 manifest 已被另一台设备更新；本次没有覆盖，请重新预览。",
            ));
        }
        if status >= 300 {
            return Err(sync_error(format!("S3 上传返回 HTTP {status}。")));
        }
        Ok(response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string))
    }

    fn delete(&self, key: &str, if_match: Option<&str>) -> Result<bool> {
        let response = self.request("DELETE", key, Vec::new(), if_match, false)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Ok(false);
        }
        if status == 412 {
            return Err(AppError::new(
                ErrorCode::SyncConflict,
                "远端对象在删除前已变化；本次删除已取消。",
            ));
        }
        if status >= 300 {
            return Err(sync_error(format!("S3 删除返回 HTTP {status}。")));
        }
        Ok(true)
    }
}

fn sync_error(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::SyncFailed, message)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key)
        .unwrap_or_else(|_| unreachable!("HMAC 接受任意长度密钥"));
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push_str(&format!("{byte:02x}"));
    }
    value
}

fn encode_key(key: &str) -> String {
    key.split('/')
        .filter(|part| !part.is_empty())
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~') {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_encoded_without_losing_structure() {
        assert_eq!(
            encode_key("pkos/v1/工作区/a b.enc"),
            "pkos/v1/%E5%B7%A5%E4%BD%9C%E5%8C%BA/a%20b.enc"
        );
    }

    #[test]
    fn webdav_basic_auth_is_not_url_visible() {
        let transport = WebDavTransport::new("https://example.test/dav", "user", "secret");
        assert_eq!(
            transport.url("pkos/a.enc"),
            "https://example.test/dav/pkos/a.enc"
        );
        assert!(!transport.url("pkos/a.enc").contains("secret"));
    }

    #[test]
    fn s3_authorization_changes_with_method_and_etag() {
        let transport = S3Transport::new(
            "https://s3.example.test",
            "bucket",
            "us-east-1",
            "AKID",
            "SECRET",
        );
        let first = transport.authorization(
            "PUT",
            "/bucket/pkos/manifest.enc",
            "s3.example.test",
            "20261022T010203Z",
            "20261022",
            &sha256_hex(b"{}"),
            None,
            false,
        );
        let second = transport.authorization(
            "PUT",
            "/bucket/pkos/manifest.enc",
            "s3.example.test",
            "20261022T010203Z",
            "20261022",
            &sha256_hex(b"{}"),
            Some("\"etag\""),
            false,
        );
        assert_ne!(first, second);
        assert!(first.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"));
        assert!(!first.contains("SECRET"));

        let create_only = transport.authorization(
            "PUT",
            "/bucket/pkos/manifest.enc",
            "s3.example.test",
            "20261022T010203Z",
            "20261022",
            &sha256_hex(b"{}"),
            None,
            true,
        );
        assert!(create_only
            .contains("SignedHeaders=host;if-none-match;x-amz-content-sha256;x-amz-date"));
    }
}
