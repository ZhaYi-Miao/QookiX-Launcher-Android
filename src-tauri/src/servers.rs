//! 托管服务器（多人游戏开服）—— 配置层。
//!
//! 与桌面版 `QookiX-Launcher/src-tauri/src/servers.rs` 保持同一套数据格式：
//! 每个服务器一个目录 `servers/<id>/`，里面放 `server.json` + 服务端文件。
//! 这样两端通用，用户也能手动改配置。
//!
//! 存储格式与前端契约（`src/api.ts` 的 12 个 `*HostedServer*` 命令）严格对齐。

use crate::models::{ServerConfig, ServerCore, ServerEntry, ServerStatus};
use std::path::{Path, PathBuf};

/// 服务器根目录（数据目录下 servers/）
fn servers_root() -> Option<PathBuf> {
    let data = crate::settings::data_dir_sync()?;
    Some(PathBuf::from(data).join("servers"))
}

pub fn server_dir(id: &str) -> Result<PathBuf, String> {
    validate_server_id(id)?;
    let root = servers_root().ok_or_else(|| "数据目录不可用".to_string())?;
    Ok(root.join(id))
}

fn server_meta_path(id: &str) -> Result<PathBuf, String> {
    Ok(server_dir(id)?.join("server.json"))
}

/// 服务器 id 来自前端，必须是安全文件名（防目录穿越）
pub fn validate_server_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 64 {
        return Err("服务器 ID 不合法".into());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("服务器 ID 含有非法字符".into());
    }
    Ok(())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 列出全部服务器（按创建时间倒序，与桌面版一致）
pub fn list_servers() -> Vec<ServerConfig> {
    let Some(root) = servers_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out: Vec<ServerConfig> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path().join("server.json")).ok()?;
            serde_json::from_str::<ServerConfig>(&text).ok()
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

pub fn get_server(id: &str) -> Result<ServerConfig, String> {
    let path = server_meta_path(id)?;
    let text =
        std::fs::read_to_string(&path).map_err(|_| format!("服务器 {id} 不存在"))?;
    serde_json::from_str(&text).map_err(|e| format!("服务器数据损坏: {e}"))
}

pub fn save_server(s: &ServerConfig) -> Result<(), String> {
    let path = server_meta_path(&s.id)?;
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/**
 * 手机可用内存的 40%（上限 2GB，最少 512MB）。
 *
 * 手机上开服最怕和系统/游戏抢内存导致被 OOM 杀掉，所以默认给得保守；
 * 用户可以在服务器设置里手动改。
 */
pub fn suggest_memory_mb() -> (u32, u32) {
    let total_mb = total_device_memory_mb().unwrap_or(4096);
    let max = (total_mb as f64 * 0.4).round() as u32;
    let max = max.clamp(512, 2048);
    let min = (max / 2).max(256);
    (min, max)
}

/// 读系统总内存（MB）。拿不到就返回 None，调用方走保守默认值。
fn total_device_memory_mb() -> Option<u32> {
    // /proc/meminfo 第一行: MemTotal:  8123456 kB
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.trim().trim_end_matches(" kB").trim().parse().ok()?;
            return Some((kb / 1024) as u32);
        }
    }
    None
}

// ── Tauri 命令 ────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_hosted_servers() -> Vec<ServerConfig> {
    list_servers()
}

#[tauri::command]
pub fn get_hosted_server(id: String) -> Result<ServerConfig, String> {
    get_server(&id)
}

#[tauri::command]
pub fn create_hosted_server(
    name: String,
    core: String,
    mc_version: String,
) -> Result<ServerConfig, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("请填写服务器名称".into());
    }
    let core_parsed = ServerCore::parse(&core).ok_or_else(|| "不支持的服务端核心".to_string())?;
    let version = mc_version.trim().to_string();
    if version.is_empty() {
        return Err("请选择游戏版本".into());
    }
    // id 用时间戳+名随机段，避免重名冲突
    let id = format!("srv_{}_{}", now_secs(), uuid::Uuid::new_v4().simple());
    let (min_mem, max_mem) = suggest_memory_mb();
    let s = ServerConfig {
        id,
        name,
        core: core_parsed,
        mc_version: version,
        port: 25565,
        max_memory_mb: max_mem,
        min_memory_mb: min_mem,
        motd: format!("{} 的服务器", "QookiX"),
        eula: false, // 必须用户自己在界面里同意 EULA
        created: now_secs(),
        last_started: None,
        java_path: None,
        jvm_args: None,
        stop_command: None,
    };
    std::fs::create_dir_all(server_dir(&s.id)?).map_err(|e| e.to_string())?;
    save_server(&s)?;
    ensure_server_files(&s.id, &s)?;
    Ok(s)
}

#[tauri::command]
pub fn update_hosted_server(patch: serde_json::Value) -> Result<ServerConfig, String> {
    let id = patch
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "缺少服务器 ID".to_string())?
        .to_string();
    let mut s = get_server(&id)?;
    if let Some(v) = patch.get("name").and_then(|v| v.as_str()) {
        s.name = v.trim().to_string();
    }
    if let Some(v) = patch.get("port").and_then(|v| v.as_u64()) {
        s.port = v as u16;
    }
    if let Some(v) = patch.get("motd").and_then(|v| v.as_str()) {
        s.motd = v.to_string();
    }
    if let Some(v) = patch.get("eula").and_then(|v| v.as_bool()) {
        s.eula = v;
    }
    if let Some(v) = patch.get("max_memory_mb").and_then(|v| v.as_u64()) {
        s.max_memory_mb = v.clamp(256, 8192) as u32;
    }
    if let Some(v) = patch.get("min_memory_mb").and_then(|v| v.as_u64()) {
        s.min_memory_mb = v.clamp(128, 8192) as u32;
    }
    if let Some(v) = patch.get("jvm_args") {
        s.jvm_args = v.as_str().map(|x| x.to_string());
    }
    save_server(&s)?;
    Ok(s)
}

#[tauri::command]
pub fn delete_hosted_server(id: String) -> Result<(), String> {
    validate_server_id(&id)?;
    let dir = server_dir(&id)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("删除失败: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn suggest_server_memory() -> serde_json::Value {
    let (min_mb, max_mb) = suggest_memory_mb();
    serde_json::json!({ "minMemoryMb": min_mb, "maxMemoryMb": max_mb })
}

// ── 服务端基础文件 ────────────────────────────────────────────────────

/**
 * 生成服务端最小可运行文件集。
 *
 * 重点是 **eula.txt**：Minecraft EULA 要求运行前必须显式同意，官方服务端
 * 第一次启动会自己退出并提示「You need to agree to the EULA」。
 * 我们预生成一个 false 的文件，让用户在界面上明确勾选后改成 true。
 */
pub fn ensure_server_files(id: &str, s: &ServerConfig) -> Result<(), String> {
    let dir = server_dir(id)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let eula = dir.join("eula.txt");
    if !eula.exists() {
        let body = format!(
            "# Minecraft EULA（运行服务端即表示你同意）\n# 由 QookiX 生成；请在「服务器设置」里勾选同意后改成 true\neula={}\n",
            s.eula
        );
        let _ = std::fs::write(&eula, body);
    } else if let Ok(cur) = std::fs::read_to_string(&eula) {
        // 同步用户在界面上的同意状态
        let want = if s.eula { "true" } else { "false" };
        if !cur.contains(&format!("eula={want}")) {
            let fixed = cur
                .lines()
                .map(|l| {
                    if l.trim_start().starts_with("eula=") {
                        format!("eula={want}")
                    } else {
                        l.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let _ = std::fs::write(&eula, fixed);
        }
    }
    let props = dir.join("server.properties");
    if !props.exists() {
        let body = format!(
            "# 由 QookiX 生成\nmotd={}\nserver-port={}\nmax-players=10\nonline-mode=true\ndifficulty=normal\ngamemode=survival\nlevel-name=world\nsync-chunk-writes=false\nview-distance=6\nsimulation-distance=4\n",
            s.motd, s.port
        );
        let _ = std::fs::write(&props, body);
    }
    Ok(())
}

/// 写一个 `server.properties` 的键（给设置页用）
pub fn write_server_property(id: &str, key: &str, value: &str) -> Result<(), String> {
    let dir = server_dir(id)?;
    let path = dir.join("server.properties");
    let cur = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = cur.lines().map(|l| l.to_string()).collect();
    let kv = format!("{key}={value}");
    if let Some(slot) = lines
        .iter_mut()
        .find(|l| l.split('=').next().map(|k| k.trim() == key).unwrap_or(false))
    {
        *slot = kv;
    } else {
        lines.push(kv);
    }
    std::fs::write(&path, lines.join("\n")).map_err(|e| e.to_string())
}

/// 读 `server.properties` 的某个键
pub fn read_server_property(id: &str, key: &str) -> Option<String> {
    let dir = server_dir(id).ok()?;
    let cur = std::fs::read_to_string(dir.join("server.properties")).ok()?;
    cur.lines()
        .find_map(|l| {
            let mut it = l.splitn(2, '=');
            let k = it.next()?.trim();
            let v = it.next().unwrap_or("").trim();
            if k == key {
                Some(v.to_string())
            } else {
                None
            }
        })
}

/// 服务端目录里某个子目录是否存在（给 ServerFileManager 的「打开目录」用）
pub fn server_sub_dir(id: &str, sub: &str) -> Result<PathBuf, String> {
    let dir = server_dir(id)?;
    if sub.is_empty() || sub == "." {
        return Ok(dir);
    }
    // 只允许单层目录名，防穿越
    if sub.contains('/') || sub.contains('\\') || sub.contains("..") {
        return Err("非法路径".into());
    }
    let p = dir.join(sub);
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p)
}

/// 服务器 jar 的实际文件名（Fabric 的启动器 jar 名字带版本号）
pub fn launch_jar_name(dir: &Path, core: ServerCore) -> String {
    if core == ServerCore::Fabric {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("fabric-server-launch") && name.ends_with(".jar") {
                    return name;
                }
            }
        }
    }
    "server.jar".to_string()
}

// ── 局域网「别人的服务器」列表与 ping（沿用原模块，改名避免与上面的 list_servers 冲突）──
use crate::settings::get_data_dir;
use serde_json::Value;
use std::io::Read;
use tokio::fs;

/// 读取某个实例的多人服务器列表（servers.json 优先，回退到 servers.dat）
pub async fn list_remote_servers(instance_id: &str) -> Result<Vec<ServerEntry>, String> {
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
pub async fn ping_remote_server(address: &str) -> ServerStatus {
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
