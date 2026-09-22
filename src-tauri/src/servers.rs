//! 多人游戏服务器：读取实例目录下的 servers.json / servers.dat，以及实时 ping。

use crate::models::{ServerEntry, ServerStatus};
use crate::settings::get_data_dir;
use std::io::Read;
use std::path::Path;
use serde_json::Value;
use tokio::fs;

/// 读取某个实例的多人服务器列表（servers.json 优先，回退到 servers.dat）
pub async fn list_servers(instance_id: &str) -> Result<Vec<ServerEntry>, String> {
    crate::fsutil::validate_id(instance_id, "实例")?;
    let data_dir = get_data_dir().await.map_err(|e| e.to_string())?;
    let dir = Path::new(&data_dir).join("instances").join(instance_id);

    // 现代 Minecraft (1.20.5+) 使用 servers.json
    let json_path = dir.join("servers.json");
    if json_path.is_file() {
        if let Ok(text) = fs::read_to_string(&json_path).await {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                if let Some(servers) = v.get("servers").and_then(|x| x.as_array()) {
                    let list: Vec<ServerEntry> = servers
                        .iter()
                        .filter_map(|s| {
                            let name = s.get("name").and_then(|x| x.as_str())?.to_string();
                            let ip = s
                                .get("ip")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            if ip.is_empty() {
                                return None;
                            }
                            let icon = s.get("icon").and_then(|x| x.as_str()).map(|x| x.to_string());
                            Some(ServerEntry { name, address: ip, icon })
                        })
                        .collect();
                    return Ok(list);
                }
            }
        }
    }

    // 旧版 Minecraft 使用 servers.dat (GZIP 压缩的 NBT)
    let dat_path = dir.join("servers.dat");
    if dat_path.is_file() {
        if let Ok(bytes) = fs::read(&dat_path).await {
            let raw: Vec<u8> = if bytes.starts_with(&[0x1f, 0x8b]) {
                let mut decompressed = Vec::new();
                match flate2::read::GzDecoder::new(&bytes[..]).read_to_end(&mut decompressed) {
                    Ok(_) => decompressed,
                    Err(_) => bytes,
                }
            } else {
                bytes
            };
            if let Ok(root) = fastnbt::from_bytes::<ServersDat>(&raw) {
                let list: Vec<ServerEntry> = root
                    .servers
                    .into_iter()
                    .filter_map(|s| {
                        if s.ip.trim().is_empty() {
                            return None;
                        }
                        let icon = s.icon.filter(|i| !i.trim().is_empty()).map(|i| {
                            if i.starts_with("data:") {
                                i
                            } else {
                                format!("data:image/png;base64,{}", i)
                            }
                        });
                        Some(ServerEntry { name: s.name, address: s.ip, icon })
                    })
                    .collect();
                return Ok(list);
            }
        }
    }

    Ok(Vec::new())
}

/// 实时 ping 一个服务器（Server List Ping 协议）
pub async fn ping_server(address: &str) -> ServerStatus {
    crate::mcping::ping_server(address).await
}

#[derive(serde::Deserialize)]
struct ServersDat {
    servers: Vec<ServerNbt>,
}

#[derive(serde::Deserialize)]
struct ServerNbt {
    name: String,
    ip: String,
    #[serde(default)]
    icon: Option<String>,
}
