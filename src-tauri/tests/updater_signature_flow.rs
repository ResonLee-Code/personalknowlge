//! 更新包 minisign 正负验签：由 `scripts/release/verify-updater-signature.sh`
//! 用一次性测试密钥生成输入，避免把发布私钥带入测试环境。

use std::fs;

use base64::Engine;
use minisign_verify::{PublicKey, Signature};

#[test]
fn signed_updater_payload_verifies_and_tampering_is_rejected() {
    let decode = |name: &str| -> String {
        let path = std::env::var(name).unwrap_or_else(|_| panic!("缺少环境变量 {name}"));
        let encoded = fs::read_to_string(path).expect("读取签名夹具");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .expect("解码 base64");
        String::from_utf8(decoded).expect("签名夹具应为 UTF-8")
    };
    let public_key = decode("PKOS_TEST_UPDATER_PUBKEY");
    let signature = decode("PKOS_TEST_UPDATER_SIGNATURE");
    let payload_path =
        std::env::var("PKOS_TEST_UPDATER_PAYLOAD").expect("缺少更新夹具载荷路径");
    let payload = fs::read(payload_path).expect("读取更新夹具");

    let public_key = PublicKey::decode(&public_key).expect("解析公钥");
    let signature = Signature::decode(&signature).expect("解析签名");
    public_key
        .verify(&payload, &signature, true)
        .expect("有效更新载荷必须通过签名校验");
    assert!(
        signature.trusted_comment().contains("version:2.0.0"),
        "签名必须绑定清单版本：{}",
        signature.trusted_comment()
    );

    let mut tampered = payload;
    tampered.extend_from_slice(b"tampered");
    assert!(
        public_key.verify(&tampered, &signature, true).is_err(),
        "篡改更新载荷必须验签失败"
    );
}
