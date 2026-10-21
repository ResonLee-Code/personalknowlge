//! 同步数据加密：Argon2id 派生密钥 + AES-256-GCM。
//!
//! 口令不落盘。远端只拿到随机 salt、nonce 与密文；AES-GCM 的附加认证数据把
//! manifest / object 的路由身份绑定进密文，防止对象调包。

use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};

use crate::core::base64;
use crate::core::error::{AppError, ErrorCode, Result};

const ENVELOPE_VERSION: u8 = 1;
const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncryptedEnvelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    if passphrase.trim().chars().count() < 12 {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "同步口令至少需要 12 个字符；口令不会保存，请自行妥善保管。",
        ));
    }
    if salt.len() < 16 {
        return Err(AppError::internal("同步 profile 的 KDF salt 无效。"));
    }
    let params = Params::new(19_456, 2, 1, Some(KEY_LEN))
        .map_err(|error| AppError::internal(format!("同步 KDF 参数无效：{error}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|error| AppError::internal(format!("同步口令派生失败：{error}")))?;
    Ok(key)
}

pub fn encrypt(key: &[u8; KEY_LEN], associated_data: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| AppError::internal("同步加密密钥长度无效。"))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|_| AppError::internal("同步内容加密失败。"))?;
    serde_json::to_vec(&EncryptedEnvelope {
        version: ENVELOPE_VERSION,
        nonce: base64::encode(&nonce),
        ciphertext: base64::encode(&ciphertext),
    })
    .map_err(AppError::from)
}

pub fn decrypt(key: &[u8; KEY_LEN], associated_data: &[u8], encrypted: &[u8]) -> Result<Vec<u8>> {
    let envelope: EncryptedEnvelope = serde_json::from_slice(encrypted).map_err(|_| {
        AppError::new(
            ErrorCode::InvalidInput,
            "远端同步对象不是受支持的密文格式。",
        )
    })?;
    if envelope.version != ENVELOPE_VERSION {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            format!("不支持的同步密文版本：{}", envelope.version),
        ));
    }
    let nonce = base64::decode(&envelope.nonce)?;
    if nonce.len() != 12 {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "远端同步对象的 nonce 长度无效。",
        ));
    }
    let ciphertext = base64::decode(&envelope.ciphertext)?;
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| AppError::internal("同步解密密钥长度无效。"))?;
    cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: associated_data,
            },
        )
        .map_err(|_| {
            AppError::new(
                ErrorCode::InvalidInput,
                "同步口令错误，或远端 manifest / 对象已被篡改。",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_associated_data_binding() {
        let salt = [7u8; 16];
        let key = derive_key("correct horse battery staple", &salt).expect("派生密钥");
        let encrypted = encrypt(&key, b"objects/note-a", "正文包含秘密".as_bytes()).expect("加密");
        assert!(!String::from_utf8_lossy(&encrypted).contains("正文包含秘密"));
        assert_eq!(
            decrypt(&key, b"objects/note-a", &encrypted).expect("解密"),
            "正文包含秘密".as_bytes()
        );
        assert!(
            decrypt(&key, b"objects/note-b", &encrypted).is_err(),
            "换对象 id 后必须因 AAD 不匹配而拒绝"
        );
    }

    #[test]
    fn tamper_and_wrong_passphrase_are_rejected() {
        let salt = [9u8; 16];
        let key = derive_key("another long passphrase", &salt).expect("派生密钥");
        let encrypted = encrypt(&key, b"manifest", b"{}").expect("加密");
        let mut tampered = encrypted.clone();
        let index = tampered.len() / 2;
        tampered[index] ^= 1;
        assert!(decrypt(&key, b"manifest", &tampered).is_err());

        let other = derive_key("a different passphrase", &salt).expect("派生另一密钥");
        assert!(decrypt(&other, b"manifest", &encrypted).is_err());
    }
}
