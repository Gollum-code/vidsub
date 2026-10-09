use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub const TOKEN_PREFIX: &str = "VSB1.";
const SERVICE: &str = "vidsub";
const USER_LICENSE: &str = "license";
const USER_API: &str = "api";

pub const PUBLIC_KEY_HEX: &str = "7fcaacd12bd067b1d9e1be917830e047fd8d46597674f226de8bbcc7bc0024b4";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub is_pro: bool,
    pub reason: String,
    pub device_limit: u32,
    pub key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LicenseToken {
    pub v: u32,
    pub kid: String,
    pub plan: String,
    pub max_devices: u32,
    pub devices: Vec<DeviceBinding>,
    pub issued_at: u64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceBinding {
    pub id: String,
    pub name: String,
    pub bound_at: u64,
}

fn env_pro() -> bool {
    std::env::var("VIDSUB_PRO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn device_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("device.json"))
}

pub fn device_id(app: &AppHandle) -> String {
    if let Some(path) = device_file(app) {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(id) = v["id"].as_str() {
                    if !id.is_empty() {
                        return id.to_string();
                    }
                }
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({ "id": id });
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&payload).unwrap_or_default());
        return id;
    }
    uuid::Uuid::new_v4().to_string()
}

fn license_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, USER_LICENSE).map_err(|e| format!("初始化凭据存储失败: {e}"))
}

fn stored_token() -> Option<String> {
    license_entry()
        .and_then(|e| e.get_password().map_err(|_| "读取失败".to_string()))
        .ok()
        .filter(|s| !s.is_empty())
}

fn decode_token(token: &str) -> Result<(Vec<u8>, Vec<u8>, LicenseToken), String> {
    if !token.starts_with(TOKEN_PREFIX) {
        return Err("激活码格式不正确（应以 VSB1. 开头）".to_string());
    }
    let rest = &token[TOKEN_PREFIX.len()..];
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() != 2 {
        return Err("激活码结构不完整".to_string());
    }
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|_| "激活码载荷解码失败".to_string())?;
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| "激活码签名解码失败".to_string())?;
    let payload: LicenseToken = serde_json::from_slice(&payload_bytes)
        .map_err(|_| "激活码载荷解析失败".to_string())?;
    Ok((payload_bytes, sig_bytes, payload))
}

pub fn verify_token(token: &str) -> Result<LicenseToken, String> {
    verify_with(PUBLIC_KEY_HEX, token)
}

fn verify_with(pub_hex: &str, token: &str) -> Result<LicenseToken, String> {
    let (payload_bytes, sig_bytes, payload) = decode_token(token)?;
    let key_bytes = pubkey_bytes_from(pub_hex)?;
    let key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| "公钥解析失败".to_string())?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| "签名数据无效".to_string())?;
    key.verify(&payload_bytes, &sig)
        .map_err(|_| "激活码签名验证失败".to_string())?;
    if payload.v != 1 {
        return Err("不支持的激活码版本".to_string());
    }
    if payload.plan != "pro" {
        return Err("激活码类型不正确".to_string());
    }
    Ok(payload)
}

pub fn pubkey_bytes_from(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("公钥应为 64 位十六进制".to_string());
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let b = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or(""), 16)
            .map_err(|_| "公钥格式错误".to_string())?;
        out[i] = b;
    }
    Ok(out)
}

pub fn build_token(private_hex: &str, token: &LicenseToken) -> Result<String, String> {
    let priv_bytes = hex_to_bytes(private_hex)?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&priv_bytes);
    let payload_bytes =
        serde_json::to_vec(token).map_err(|e| format!("序列化激活码失败: {e}"))?;
    let sig = signing_key.sign(&payload_bytes);
    Ok(format!(
        "{}{}.{}",
        TOKEN_PREFIX,
        URL_SAFE_NO_PAD.encode(&payload_bytes),
        URL_SAFE_NO_PAD.encode(sig.to_bytes().as_ref())
    ))
}

pub fn pubkey_hex() -> String {
    PUBLIC_KEY_HEX.to_string()
}

fn hex_to_bytes(hex: &str) -> Result<[u8; 32], String> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return Err("私钥应为 64 位十六进制".to_string());
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let b = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or(""), 16)
            .map_err(|_| "私钥格式错误".to_string())?;
        out[i] = b;
    }
    Ok(out)
}

pub fn current(app: &AppHandle) -> LicenseInfo {
    if env_pro() {
        return LicenseInfo {
            is_pro: true,
            reason: "env".to_string(),
            device_limit: 3,
            key: None,
        };
    }

    if let Some(token) = stored_token() {
        match verify_token(&token) {
            Ok(t) => {
                let my_id = device_id(app);
                let bound = t.devices.iter().any(|d| d.id == my_id);
                return LicenseInfo {
                    is_pro: bound,
                    reason: if bound { "license" } else { "device_not_bound" }.to_string(),
                    device_limit: t.max_devices,
                    key: if bound { Some(token) } else { None },
                };
            }
            Err(_) => {
                let _ = license_entry().and_then(|e| {
                    e.delete_credential()
                        .map_err(|_| "清除失败".to_string())
                });
            }
        }
    }

    LicenseInfo {
        is_pro: false,
        reason: "free".to_string(),
        device_limit: 3,
        key: None,
    }
}

pub fn activate(app: &AppHandle, token: &str) -> Result<LicenseInfo, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("请输入激活码".to_string());
    }
    let t = verify_token(token)?;
    let my_id = device_id(app);
    if !t.devices.iter().any(|d| d.id == my_id) {
        return Err(format!(
            "当前设备（{my_id}）未绑定到该激活码（已绑定 {} 台），请联系发放方解绑或重新签发。",
            t.devices.len()
        ));
    }
    let entry = license_entry()?;
    entry
        .set_password(token)
        .map_err(|e| format!("保存激活信息失败: {e}"))?;
    Ok(current(app))
}

pub fn deactivate(_app: &AppHandle) -> Result<(), String> {
    license_entry()?
        .delete_credential()
        .map_err(|e| format!("清除激活信息失败: {e}"))
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, USER_API).map_err(|e| e.to_string())?;
    entry
        .set_password(key)
        .map_err(|e| format!("保存 API Key 失败: {e}"))
}

pub fn get_api_key() -> Option<String> {
    keyring::Entry::new(SERVICE, USER_API)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.is_empty())
}

pub fn clear_api_key() -> Result<(), String> {
    keyring::Entry::new(SERVICE, USER_API)
        .map_err(|e| e.to_string())?
        .delete_credential()
        .map_err(|e| format!("清除 API Key 失败: {e}"))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_roundtrip_and_tamper() {
        let secret: [u8; 32] = rand::random();
        let sk = ed25519_dalek::SigningKey::from_bytes(&secret);
        let pub_hex: String = sk
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let priv_hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
        let t = LicenseToken {
            v: 1,
            kid: "test".into(),
            plan: "pro".into(),
            max_devices: 3,
            devices: vec![DeviceBinding {
                id: "d1".into(),
                name: "pc".into(),
                bound_at: 0,
            }],
            issued_at: 0,
            name: "test".into(),
        };
        let token = build_token(&priv_hex, &t).unwrap();
        let got = verify_with(&pub_hex, &token).unwrap();
        assert_eq!(got.max_devices, 3);
        assert_eq!(got.devices[0].id, "d1");
        let mut bad = token.clone();
        bad.push('x');
        assert!(verify_with(&pub_hex, &bad).is_err());
    }
}
