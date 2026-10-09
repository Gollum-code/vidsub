use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use vidsub_lib::license::{build_token, verify_token, DeviceBinding, LicenseToken};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_hex(path: &str) -> Result<String, String> {
    Ok(std::fs::read_to_string(path)
        .map_err(|e| format!("读取 {} 失败: {e}", path))?
        .trim()
        .to_string())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("genkey") => {
            let dir = PathBuf::from(args.get(2).ok_or("usage: license-issuer genkey <dir>")?);
            std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
            let secret: [u8; 32] = rand::random();
            let sk = ed25519_dalek::SigningKey::from_bytes(&secret);
            let pk = sk.verifying_key();
            let priv_path = dir.join("private.hex");
            let pub_path = dir.join("public.hex");
            std::fs::write(&priv_path, to_hex(&sk.to_bytes())).map_err(|e| e.to_string())?;
            std::fs::write(&pub_path, to_hex(pk.to_bytes().as_ref())).map_err(|e| e.to_string())?;
            println!("private={}", priv_path.display());
            println!("public={}", pub_path.display());
        }
        Some("issue") => {
            let key_path = args.get(2).ok_or("usage: license-issuer issue <private.hex> <name> <max_devices> <device_id...>")?;
            let name = args.get(3).ok_or("缺少许可名")?.clone();
            let max_devices: u32 = args
                .get(4)
                .ok_or("缺少设备数")?
                .parse()
                .map_err(|_| "设备数应为数字")?;
            let device_ids: Vec<String> = args.iter().skip(5).cloned().collect();
            if max_devices == 0 || device_ids.is_empty() {
                return Err("至少需要 1 个设备并指定设备数".to_string());
            }
            if device_ids.len() as u32 > max_devices {
                return Err("绑定设备数超过上限".to_string());
            }
            let t = now();
            let token = LicenseToken {
                v: 1,
                kid: "dev".to_string(),
                plan: "pro".to_string(),
                max_devices,
                devices: device_ids
                    .into_iter()
                    .map(|id| DeviceBinding {
                        id,
                        name: name.clone(),
                        bound_at: t,
                    })
                    .collect(),
                issued_at: t,
                name,
            };
            let private = read_hex(key_path)?;
            println!("{}", build_token(&private, &token)?);
        }
        Some("verify") => {
            let token = args.get(2).ok_or("usage: license-issuer verify <token>")?;
            let t = verify_token(token)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&t).map_err(|e| e.to_string())?
            );
        }
        _ => {
            return Err(
                "usage:\n  genkey <dir>\n  issue <private.hex> <name> <max_devices> <device_id...>\n  verify <token>"
                    .to_string(),
            );
        }
    }
    Ok(())
}