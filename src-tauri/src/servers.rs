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
    // RCON 是「优雅停服 + 控制台命令」的唯一通道，必须在 properties 里开启。
    // 注意：Minecraft 只在**启动时**读这些项，所以改完要重启服才生效。
    let rcon = crate::rcon::load_or_create_creds(&dir, s.port)?;
    if !props.exists() {
    let body = format!(
  "# 由 QookiX 生成\nmotd={}\nserver-port={}\nmax-players=10\nonline-mode=true\ndifficulty=normal\ngamemode=survival\nlevel-name=world\nsync-chunk-writes=false\nview-distance=6\nsimulation-distance=4\n\n# RCON：QookiX 用它执行 stop（优雅停服，世界会存盘）与控制台命令。\n# 密码在同目录 rcon.json 里；请勿泄露（等同服务器后台权限）。\nenable-rcon=true\nrcon.password={}\nrcon.port={}\nbroadcast-rcon-to-ops=false\n",
  s.motd, s.port, rcon.password, rcon.port
  );
  let _ = std::fs::write(&props, body);
    } else {
  // 已存在的 properties：把 RCON 三项补上（用户可能是手动删过或老版本生成的）
  if let Ok(cur) = std::fs::read_to_string(&props) {
    let mut need = false;
    for k in ["enable-rcon", "rcon.password", "rcon.port"] {
      if !cur.lines().any(|l| l.trim_start().starts_with(k)) {
        need = true;
      }
    }
    if need {
      let mut lines: Vec<String> = cur
      .lines()
      .filter(|l| {
        let t = l.trim_start();
        !t.starts_with("enable-rcon")
          && !t.starts_with("rcon.password")
          && !t.starts_with("rcon.port")
          && !t.starts_with("broadcast-rcon-to-ops")
      })
      .map(|l| l.to_string())
      .collect();
      lines.push(String::new());
      lines.push("# 由 QookiX 补上：RCON（优雅停服 / 控制台命令）".into());
      lines.push("enable-rcon=true".into());
      lines.push(format!("rcon.password={}", rcon.password));
      lines.push(format!("rcon.port={}", rcon.port));
      lines.push("broadcast-rcon-to-ops=false".into());
      let _ = std::fs::write(&props, lines.join("\n") + "\n");
    }
  }
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

// ── 服务端核心下载 ────────────────────────────────────────────────────

/// PaperMC 官方 fill API（v3）。**必须带 User-Agent**，否则 403。
const PAPERMC_API: &str = "https://fill.papermc.io/v3/projects";
const PAPERMC_UA: &str =
    "QookiX-Launcher/1.1.0 (https://github.com/weimosheng/QookiX-Launcher)";

/// 原版服务端核心的下载 URL：走官方 version manifest。
/// Android 侧没有 mcmeta 模块，这里直接用 piston 元数据。
const PISTON_META: &str = "https://piston-meta.mojang.com/v1/packages";

async fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(PAPERMC_UA)
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())
}

/// 取元数据用的 client：**短超时**（15 秒）。
///
/// 不能复用下载那个 600 秒超时的 client：Mojang / CDN 在这台设备上
/// 经常连得上却发不出数据（实测 install 挂到 CDP 超时也没返回），
/// 600 秒的等待会让「点一下装核心」看起来像卡死。15 秒 × 4 次重试
/// 最多 1 分钟就能明确告诉用户「这个源连不上」。
async fn meta_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(PAPERMC_UA)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// 带重试的 JSON GET。
///
/// 这台设备上到 CDN / API 的连接**会中途断**（实测大文件固定在 1.8MB 左右被切，
/// 小 JSON 也会偶发 `error decoding response body`）。一次失败就报错的话，
/// 「点一下装核心」经常直接失败，而重试一次通常就好。
async fn get_json_retry(
    client: &reqwest::Client,
    url: &str,
    tries: usize,
) -> Result<serde_json::Value, String> {
    let mut last = String::new();
    for i in 1..=tries.max(1) {
        let attempt = async {
            let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status()));
            }
            resp.json::<serde_json::Value>()
                .await
                .map_err(|e| format!("解析响应失败: {e}"))
        }
        .await;
        match attempt {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = e;
                tracing::warn!("[core] 请求 {url} 第 {i} 次失败: {last}");
                if i < tries.max(1) {
                    tokio::time::sleep(std::time::Duration::from_millis(500 * i as u64)).await;
                }
            }
        }
    }
    Err(last)
}

/**
 * 流式下载到文件，**带 sha256 校验与断点续传**。
 *
 * ## 为什么必须做这两件事
 *
 * 实测发现下载会被中途截断（同一个 1.8MB 上限，PC curl 和手机 reqwest 都中招）：
 * 拿到的是**半个 jar**，大小 1.8MB 而非 49MB。旧实现直接把它当成功返回，
 * 于是「核心已安装」为真，启动时才炸（zip 解析失败 / 主类找不到），
 * 错误信息完全指不到真凶。
 *
 * - **sha256 校验**：Paper 的 builds 接口给了 `downloads["server:default"].sha256`，
 *   是唯一可靠的「文件完整」判据。
 * - **断点续传**：既然每次连接只给 ~1.8MB，就用 `Range: bytes=N-` 一段段接上，
 *   而不是每次从 0 开始（那样永远下不完）。
 */
async fn download_verified(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expect_sha: Option<&str>,
) -> Result<u64, String> {
    use sha2::{Digest, Sha256};
    use std::io::Write as _;
    use tokio::io::AsyncWriteExt;

    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let part = dest.with_extension("part");
    // 最多 200 段：1.8MB × 200 ≈ 360MB，足够任何核心
    const MAX_CHUNKS: usize = 200;

    for attempt in 1..=MAX_CHUNKS {
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let req = client.get(url).header("Range", format!("bytes={have}-"));
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                if attempt == 1 {
                    return Err(format!("下载失败: {e}"));
                }
                tracing::warn!("[core] 第 {attempt} 段请求失败: {e}，重试");
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                continue;
            }
        };

        let status = resp.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(format!("下载失败: HTTP {status}"));
        }

        // 206 = 服务端支持续传；200 = 不支持，只能从头下（先清掉已有分片）
        let append = status == reqwest::StatusCode::PARTIAL_CONTENT && have > 0;
        if !append && have > 0 {
            let _ = std::fs::remove_file(&part);
        }

        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(append)
            .write(true)
            .open(&part)
            .await
            .map_err(|e| e.to_string())?;
        let mut resp = resp;
        let mut got = 0u64;
        // **关键**：连接中途断开（`error decoding response body`）是这里的常态，
        // 不是致命错误 —— 已经写进 .part 的字节都算数，下一轮带 Range 接着下。
        // 早先写成 `chunk().await?` 会让第一次断流就把整个下载判失败，
        // 续传代码永远走不到，等于没有续传。
        let mut cut = false;
        loop {
            // 单段最多 15 秒没有新数据就换下一段。
            // 整体 timeout 是 600 秒，若不在读层面加超时，一个卡住的连接会
            // 干等十分钟才断 —— 续传轮数有限，等不起（实测 240 秒只下了 391KB）。
            let step = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                resp.chunk(),
            )
            .await;
            match step {
                // step: Result<Result<Option<Bytes>, Error>, Elapsed>（三层，别写错）
                Ok(Ok(Some(chunk))) => {
                    if let Err(e) = f.write_all(&chunk).await {
                        return Err(format!("写入失败: {e}"));
                    }
                    got += chunk.len() as u64;
                }
                Ok(Ok(None)) => break,
                Ok(Err(e)) => {
                    tracing::warn!("[core] 本段被中断（已写入 {got} 字节）: {e}");
                    cut = true;
                    break;
                }
                Err(_) => {
                    tracing::warn!("[core] 本段 15 秒无进展，续传（已写入 {got} 字节）");
                    cut = true;
                    break;
                }
            }
        }
        drop(f);
        let _ = cut;

        let total = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        tracing::info!("[core] 第 {attempt} 段 +{got} 字节，累计 {total}");

        // 有 sha256 就以它为准判完成
        if let Some(want) = expect_sha {
            if total > 0 {
                let mut h = Sha256::new();
                let mut buf = vec![0u8; 256 * 1024];
                let mut rf = std::fs::File::open(&part).map_err(|e| e.to_string())?;
                use std::io::Read as _;
                loop {
                    let n = rf.read(&mut buf).map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    h.update(&buf[..n]);
                }
                let got_sha = format!("{:x}", h.finalize());
                if got_sha.eq_ignore_ascii_case(want) {
                    std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
                    return Ok(total);
                }
                tracing::warn!(
                    "[core] sha256 不符（{got_sha}…），续传中或需重下（已有 {total} 字节）"
                );
                // 校验不过：文件内容有问题（比如服务端忽略了 Range 从头重发），
                // 删掉重来，别在坏文件上无限续传
                if !append {
                    let _ = std::fs::remove_file(&part);
                }
            }
        } else if got == 0 {
            return Err("下载中断：服务器没有返回任何数据".to_string());
        } else if total > 0 && !append {
            // 无 sha256 可校验时，用「本次读完了整个响应」作为完成判据
            let _ = std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
            return Ok(total);
        }
    }

    Err("下载未完成：超过最大重试次数".to_string())
}

/**
 * 下载 Paper 核心。
 *
 * API：`GET /v3/projects/paper/versions/{mc}/builds` → 取 STABLE 通道的
 * `downloads["server:default"].url`。v3 的 body 直接是数组（老 v2 有 {ok, builds} 包装）。
 */
async fn download_paper(
    client: &reqwest::Client,
    id: &str,
    mc_version: &str,
) -> Result<String, String> {
    let url = format!("{PAPERMC_API}/paper/versions/{mc_version}/builds");
    let meta = meta_client().await?;
    let body = get_json_retry(&meta, &url, 4)
        .await
        .map_err(|e| format!("获取 Paper 版本信息失败: {e}"))?;

    let builds = body
        .as_array()
        .ok_or_else(|| "Paper API 返回格式异常".to_string())?;
    if builds.is_empty() {
        return Err(format!("Paper 暂无 {mc_version} 的构建"));
    }
    // 选 build：**在 STABLE 里取 id 最大的那个**。
    //
    // 两个坑叠在一起：
    // ① 不能靠数组顺序。虽然实测 fill v3 是新→旧，但依赖顺序很脆；
    // ② 字段名是 **`id`**，没有 `build` 字段。写成 `b.get("build")` 时所有 key 都是 0，
    //    `max_by_key` 会**静默返回最后一个元素** —— 而那恰好是最旧的 build 2。
    //    它在 CDN 上没缓存，实测速率 ~1KB/s（49MB 要十几个小时），
    //    最新的 build 133 却是 10 秒下完。字段名写错会伪装成「网络慢」。
    let build_no = |b: &serde_json::Value| {
        b.get("id")
            .and_then(|v| v.as_i64())
            .or_else(|| b.get("build").and_then(|v| v.as_i64()))
            .unwrap_or(0)
    };
    if builds.iter().all(|b| build_no(b) == 0) {
        return Err("Paper API 返回格式异常：所有构建都没有 id 字段".to_string());
    }
    let build = builds
        .iter()
        .filter(|b| b.get("channel").and_then(|v| v.as_str()) == Some("STABLE"))
        .max_by_key(|b| build_no(b))
        .or_else(|| builds.iter().max_by_key(|b| build_no(b)))
        .ok_or_else(|| "Paper 没有可用构建".to_string())?;
    tracing::info!(
        "[core] 选用 Paper build {}（channel={}）",
        build_no(build),
        build.get("channel").and_then(|v| v.as_str()).unwrap_or("?")
    );

    let dl_info = build
        .get("downloads")
        .and_then(|d| d.get("server:default"))
        .ok_or_else(|| "该构建没有服务端核心".to_string())?;
    let dl = dl_info
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "下载地址为空".to_string())?
        .to_string();
    // sha256 在 `downloads["server:default"].checksums.sha256`（v3 是嵌套的，
    // 不是 v2 那种直接挂在 downloads 下）。取不到就退化为「无校验」而不是假装有。
    let sha = dl_info
        .get("checksums")
        .and_then(|c| c.get("sha256"))
        .or_else(|| dl_info.get("sha256"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    if sha.is_none() {
        tracing::warn!("[core] Paper 没给 sha256，只能按长度判断完整性");
    }

    let dest = server_dir(id)?.join("server.jar");
    let bytes = download_verified(client, &dl, &dest, sha.as_deref()).await?;
    tracing::info!("[core] Paper 核心完成 {bytes} 字节");
    Ok(dl)
}

/**
 * 下载原版服务端核心。
 *
 * 路径是「版本清单 → 该版本详情 → downloads.server」两步：
 * `piston-meta.mojang.com/v1/packages/...` 那个新接口实测全部 404（已废弃），
 * 只有 `version_manifest_v2.json` 这条老路是通的（项目 mirror.rs 也在用它）。
 *
 * 官方只给 **sha1**（不是 sha256），所以完整性判据用 sha1。
 */
async fn download_vanilla(
    client: &reqwest::Client,
    id: &str,
    mc_version: &str,
) -> Result<(), String> {
    let meta = meta_client().await?;
    let manifest = get_json_retry(&meta, crate::mirror::OFFICIAL_MANIFEST, 4)
        .await
        .map_err(|e| format!("获取版本清单失败: {e}"))?;
    let entry = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|v| v.get("id").and_then(|x| x.as_str()) == Some(mc_version))
        })
        .ok_or_else(|| format!("版本清单里没有 {mc_version}"))?;
    let vurl = entry
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "版本清单缺少 url".to_string())?
        .to_string();
    let vj = get_json_retry(&meta, &vurl, 4)
        .await
        .map_err(|e| format!("获取 {mc_version} 详情失败: {e}"))?;
    let dl = vj
        .get("downloads")
        .and_then(|d| d.get("server"))
        .ok_or_else(|| format!("版本 {mc_version} 没有官方服务端核心"))?;
    let url = dl
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "下载地址为空".to_string())?
        .to_string();
    let sha1 = dl.get("sha1").and_then(|v| v.as_str()).map(|s| s.to_string());

    // 官方源慢就换 BMCLAPI 镜像（同路径替换）。
    // 注意：这里**必须直接 .await**，不能套 `tauri::async_runtime::block_on` ——
    // 本函数已经在 tokio 运行时里，运行时内再 block_on 会直接死锁，
    // 表现为「点装核心后命令永远不返回」（实测卡到 CDP 超时也没任何错误）。
    let mirror_base = match crate::settings::get_settings().await {
        Ok(s) => crate::mirror::resolve_from(&s.mirror, s.mirror_custom.as_deref().unwrap_or("")),
        Err(_) => String::new(),
    };
    let dest = server_dir(id)?.join("server.jar");
    let urls: Vec<String> = if mirror_base.is_empty() {
        vec![url]
    } else {
        vec![
            url.clone(),
            format!("{mirror_base}{}", url.strip_prefix("https://piston-data.mojang.com").unwrap_or(&url)),
        ]
    };

    let mut last_err = String::new();
    for u in urls {
        match download_sha1(client, &u, &dest, sha1.as_deref()).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                tracing::warn!("[core] 原版核心从 {u} 下载失败: {e}");
                last_err = e;
            }
        }
    }
    Err(format!("原版核心下载失败: {last_err}"))
}

/// sha1 校验版下载（官方原版核心只提供 sha1）
async fn download_sha1(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    want: Option<&str>,
) -> Result<(), String> {
    use sha1::{Digest, Sha1};
    use std::io::{Read, Write};
    use tokio::io::AsyncWriteExt;

    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let part = dest.with_extension("part");
    for attempt in 1..=200u32 {
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let resp = client
            .get(url)
            .header("Range", format!("bytes={have}-"))
            .send()
            .await
            .map_err(|e| format!("下载失败: {e}"))?;
        let status = resp.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(format!("HTTP {status}"));
        }
        let append = status == reqwest::StatusCode::PARTIAL_CONTENT && have > 0;
        if !append && have > 0 {
            let _ = std::fs::remove_file(&part);
        }
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(append)
            .write(true)
            .open(&part)
            .await
            .map_err(|e| e.to_string())?;
        let mut resp = resp;
        let mut got = 0u64;
        loop {
            let step =
                tokio::time::timeout(std::time::Duration::from_secs(15), resp.chunk()).await;
            match step {
                Ok(Ok(Some(c))) => {
                    f.write_all(&c).await.map_err(|e| e.to_string())?;
                    got += c.len() as u64;
                }
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => break,
            }
        }
        drop(f);
        let total = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        tracing::info!("[core] 原版第 {attempt} 段 +{got}，累计 {total}");
        if got == 0 && total == 0 {
            return Err("服务器没有返回数据".to_string());
        }
        if let Some(w) = want {
            let mut h = Sha1::new();
            let mut buf = vec![0u8; 256 * 1024];
            let mut rf = std::fs::File::open(&part).map_err(|e| e.to_string())?;
            loop {
                let n = rf.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            let got_sha = format!("{:x}", h.finalize());
            if got_sha.eq_ignore_ascii_case(w) {
                std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
                return Ok(());
            }
            tracing::warn!("[core] 原版 sha1 不符，续传（{total} 字节）");
            if !append {
                let _ = std::fs::remove_file(&part);
            }
        } else {
            std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err("下载未完成".to_string())
}

/** Fabric 服务端安装器（官方 meta API） */
async fn download_fabric(
    client: &reqwest::Client,
    id: &str,
    mc_version: &str,
) -> Result<(), String> {
    let mc = meta_client().await?;
    let meta = get_json_retry(&mc, "https://meta.fabricmc.net/v2/versions/installer", 4)
        .await
        .map_err(|e| format!("获取 Fabric 版本信息失败: {e}"))?;
    let url = meta
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Fabric 安装器列表为空".to_string())?
        .to_string();
    let dir = server_dir(id)?;
    download_verified(client, &url, &dir.join("fabric-installer.jar"), None).await?;
    // 顺便写一份 server.properties 里的推荐项（Fabric 也认）
    Ok(())
}

/// 核心是否已就绪（能否启动）
pub fn core_installed(id: &str) -> bool {
    let Ok(dir) = server_dir(id) else { return false };
    dir.join("server.jar").exists() || fabric_launcher_jar(&dir).is_some()
}

fn fabric_launcher_jar(dir: &Path) -> Option<String> {
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.starts_with("fabric-server-launch") && n.ends_with(".jar") {
            return Some(n);
        }
    }
    None
}

// ── Tauri 命令：核心安装 ───────────────────────────────────────────────

/// 下载/安装服务端核心。Paper 与原版是单文件；Fabric 先下安装器（Stage 3 再接安装流程）。
#[tauri::command]
pub async fn install_hosted_server_core(id: String) -> Result<String, String> {
    let s = get_server(&id)?;
    let client = http_client().await?;
    let res = match s.core {
        ServerCore::Paper => download_paper(&client, &id, &s.mc_version).await?,
        ServerCore::Vanilla => {
            download_vanilla(&client, &id, &s.mc_version).await?;
            "server.jar".to_string()
        }
        ServerCore::Fabric => {
            download_fabric(&client, &id, &s.mc_version).await?;
            "fabric-installer.jar".to_string()
        }
        other => {
            return Err(format!(
                "{} 核心还在开发中，先用 Paper 或原版",
                other.as_str()
            ))
        }
    };
    // 核心到位后补一次 EULA/配置（端口/motd 可能刚改过）
    ensure_server_files(&id, &s)?;
    Ok(res)
}

#[tauri::command]
pub fn hosted_server_core_installed(id: String) -> bool {
    core_installed(&id)
}

// ── 服务端运行（主进程侧，:server 独立进程配合） ────────────────────────

/// 找一个可用的 JRE：优先用户装的（runtimes/），退回系统 JRE。
fn find_server_jre() -> Result<String, String> {
    let data = crate::settings::data_dir_sync()
        .ok_or_else(|| "数据目录不可用".to_string())?;
    let runtimes = PathBuf::from(&data).join("runtimes");
    if let Ok(rd) = std::fs::read_dir(&runtimes) {
        for e in rd.flatten() {
            let home = e.path();
            // 有 libjli.so 才算可用
            if home.join("lib").join("arm64").join("libjli.so").exists()
                || home.join("lib").join("libjli.so").exists()
            {
                return Ok(home.to_string_lossy().to_string());
            }
        }
    }
    // 系统 JRE
    let sys = std::env::var("JAVA_HOME").unwrap_or_default();
    if !sys.is_empty() {
        return Ok(sys);
    }
    Err("没有可用的 Java 运行时：请先在「设置 → 运行环境」里装一个 JRE".to_string())
}

/// 启动一个服务器。需要：核心已装好 + 同意 EULA。
#[tauri::command]
pub async fn start_hosted_server(id: String) -> Result<(), String> {
    let s = get_server(&id)?;

    if !core_installed(&id) {
        return Err("还没下载服务端核心".to_string());
    }
    let dir = server_dir(&id)?;

    // EULA 以**配置**为准重写一遍再检查。
    // （不能只读 eula.txt：勾选「同意」时若没同步落盘，文件里还是 eula=false，
    //   表现就是「明明勾了却还说要同意」——同一份状态存两处必然对不上。）
    if s.eula {
        std::fs::write(
            dir.join("eula.txt"),
            "# Minecraft EULA（运行服务端即表示你同意）\n# 由 QookiX 生成；请在「服务器设置」里勾选同意后改成 true\neula=true\n",
        )
        .map_err(|e| e.to_string())?;
    }
    let eula = std::fs::read_to_string(dir.join("eula.txt")).unwrap_or_default();
    if !eula.lines().any(|l| l.trim() == "eula=true") {
        return Err("请先在「服务器设置」里勾选同意 Minecraft EULA".to_string());
    }

    // 已经在跑？
    if server_runtime(&id).map(|r| r.running) == Some(true) {
        return Err("这个服务器已经在运行了".to_string());
    }

    // 每次启动前重写 RCON 配置：Minecraft 首次启动会自己把 enable-rcon 改成 false
    ensure_rcon_props(&dir, s.port)?;

    let jre_home = find_server_jre()?;
    // 核心 jar：Paper/原版是 server.jar；Fabric 是 fabric-installer.jar
    let jar = dir.join("server.jar");
    let jar = if jar.exists() { jar } else {
        // Fabric 情形
        std::fs::read_dir(&dir)
            .ok()
            .and_then(|rd| rd.flatten().find(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("fabric-server-launch") && n.ends_with(".jar")
            }).map(|e| e.path()))
            .ok_or_else(|| "找不到服务端核心 jar".to_string())?
    };

    // 手机上要留余量给主进程 + 系统：1GB 堆实测会让 :server 稳占 ~850MB，
    // 再加主进程 300MB / :tunnel 120MB 就触发 lowmemorykiller 了。
    // 上限压到 2048，且下限 512（1-2 人小服够用）。
    let mem = s.max_memory_mb.clamp(512, 2048);
    let spec = crate::server_process::LaunchSpec {
        id: id.clone(),
        jre_home,
        jar: jar.to_string_lossy().to_string(),
        xmx_mb: mem,
        work_dir: dir.to_string_lossy().to_string(),
        // Paper/原版都接受 nogui（表示不要读 stdin）
        args: vec!["nogui".to_string()],
        token: crate::server_process::new_token(),
    };

    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("launch.json"),
        serde_json::to_string_pretty(&spec).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    // 拉起 :server 独立进程（Rust → Kotlin → 新进程 → JNI → 回到 Rust 起 JVM）
    crate::android_bridge::start_server_process(&id)
}

/// 停止运行中的服务器。
///
/// 顺序很重要：**先 RCON `stop`（服务端自己存盘关世界）**，只有在 RCON 不可用
/// 或超时后才退回 IPC `System.exit(0)`。后者实测不会触发 `Saving worlds`，
/// 等于直接杀进程 —— 玩家建筑会回档。
#[tauri::command]
pub async fn stop_hosted_server(id: String) -> Result<String, String> {
    let rt = server_runtime(&id).ok_or_else(|| "这个服务器没有运行记录".to_string())?;
    if !rt.running {
        return Ok("服务器本来就没在运行".to_string());
    }
    // 拿 RCON 凭据
    let dir = server_dir(&id)?;
    let creds = crate::rcon::load_or_create_creds(&dir, get_server(&id)?.port)?;

    // runtime.json 里有 IPC 端口与 token（等 JVM 退出用）
    let (ipc_port, token) = read_runtime_io(&id).unwrap_or((0, String::new()));

    match crate::rcon::graceful_stop(ipc_port, &token, &creds, 20) {
        Ok(msg) => {
            // JVM 已自行退出，:server 进程会随之结束；这里只清通知
            let _ = crate::android_bridge::notify_server_stopped(&id);
            Ok(msg)
        }
        Err(e) => {
            tracing::warn!("[server] 优雅停服失败（{e}），退回强制停止");
            // 兜底：直接让 :server 进程里的 JVM 退出
            let forced = crate::android_bridge::force_stop_server(&id);
            match forced {
                Ok(()) => Ok(format!("{e}；已强制停止（世界可能未存盘）")),
                Err(e2) => Err(format!("{e}；强制停止也失败：{e2}")),
            }
        }
    }
}

/// 确保 `server.properties` 里的 RCON 三项是「开」的。
///
/// ## 为什么每次启动前都要重写
///
/// Minecraft **首次启动时会自己生成一份完整的 server.properties**，
/// 并把 `enable-rcon` 写成 `false`、`rcon.password` 留空。
/// 我们在「装核心」时写好的配置会被这一次覆盖掉 ——
/// 表现就是「代码里明明 enable-rcon=true，RCON 端口却连不上（Connection refused）」。
///
/// server.properties 只在服务端**启动时**读取，所以启动前改是安全且立刻生效的。
pub fn ensure_rcon_props(dir: &Path, game_port: u16) -> Result<crate::rcon::RconCreds, String> {
    let creds = crate::rcon::load_or_create_creds(dir, game_port)?;
    let props = dir.join("server.properties");
    let mut lines: Vec<String> = match std::fs::read_to_string(&props) {
        Ok(cur) => cur
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                // 先剔掉旧的 RCON 行，下面统一重写（可能有多行重复）
                !t.starts_with("enable-rcon")
                    && !t.starts_with("rcon.password")
                    && !t.starts_with("rcon.port")
            })
            .map(|l| l.to_string())
            .collect(),
        Err(_) => vec![
            "# 由 QookiX 生成".into(),
            format!("server-port={game_port}"),
            String::new(),
        ],
    };
    lines.push(String::new());
    lines.push("# RCON：由 QookiX 启用（优雅停服 / 控制台命令）".into());
    lines.push("enable-rcon=true".into());
    lines.push(format!("rcon.password={}", creds.password));
    lines.push(format!("rcon.port={}", creds.port));
    lines.push("broadcast-rcon-to-ops=false".into());
    std::fs::write(&props, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    Ok(creds)
}

/// 执行一条服务端控制台命令（RCON）。
#[tauri::command]
pub fn server_console_command(id: String, command: String) -> Result<String, String> {
    let cmd = command.trim().to_string();
    if cmd.is_empty() {
        return Ok(String::new());
    }
    let dir = server_dir(&id)?;
    let creds = crate::rcon::load_or_create_creds(&dir, get_server(&id)?.port)?;
    crate::rcon::exec(creds.port, &creds.password, &cmd)
}

/// 读 runtime.json 里的 (ipc_port, token)
fn read_runtime_io(id: &str) -> Option<(u16, String)> {
    let dir = server_dir(id).ok()?;
    let text = std::fs::read_to_string(dir.join("runtime.json")).ok()?;
    let info: crate::server_process::RuntimeInfo = serde_json::from_str(&text).ok()?;
    Some((info.ipc_port, info.token))
}

/// IPC 探活（rcon.rs 与 UI 都用）
pub fn ipc_alive(port: u16, token: &str) -> bool {
    ipc_status(port, token).is_some()
}

/// 服务器运行态（UI 轮询用）。
#[tauri::command]
pub fn hosted_server_runtime(id: String) -> Option<ServerRuntime> {
    server_runtime(&id)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerRuntime {
    pub running: bool,
    #[serde(rename = "startedAt")]
    pub started_at: u64,
    /// JVM 起来了吗（false = 还在启动中）
    #[serde(rename = "jvmUp")]
    pub jvm_up: bool,
    #[serde(rename = "exitNote")]
    pub exit_note: Option<String>,
}

/// 读 runtime.json 并探活（IPC /status 通了才算真在跑）。
pub fn server_runtime(id: &str) -> Option<ServerRuntime> {
    let dir = server_dir(id).ok()?;
    let rt = dir.join("runtime.json");
    if !rt.exists() {
        return None;
    }
    let text = std::fs::read_to_string(rt).ok()?;
    let info: crate::server_process::RuntimeInfo = serde_json::from_str(&text).ok()?;

    // 探活：连 IPC 端口问一句。进程死了连不上。
    let alive = ipc_status(info.ipc_port, &info.token).is_some();
    Some(ServerRuntime {
        running: alive,
        started_at: info.started_at,
        jvm_up: alive,
        exit_note: info.exit,
    })
}

/// GET http://127.0.0.1:{port}/status，返回 Some 表示进程活着且鉴权通过
fn ipc_status(port: u16, token: &str) -> Option<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    if port == 0 {
        return None;
    }
    let addr = format!("127.0.0.1:{port}");
    let mut s = TcpStream::connect(&addr).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_millis(2000)))
        .ok()?;
    s.set_write_timeout(Some(std::time::Duration::from_millis(2000)))
        .ok()?;
    s.write_all(
        format!("GET /status HTTP/1.1\r\nX-Token: {token}\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .ok()?;
    let mut buf = String::new();
    s.read_to_string(&mut buf).ok()?;
    // 必须确认是 200：鉴权失败/路径不对时服务端会回 404，
    // 那不代表「进程活着」，只代表「端口有人在听」。
    if !buf.starts_with("HTTP/1.1 200") {
        tracing::warn!("[server] IPC 探活返回异常: {}", &buf.lines().next().unwrap_or(""));
        return None;
    }
    Some(buf)
}

/// 读服务端日志尾部（Paper 写自己的 logs/latest.log，这里只取最后 n 行）。
#[tauri::command]
pub fn hosted_server_log(id: String, lines: Option<usize>) -> Result<Vec<String>, String> {
    let dir = server_dir(&id)?;
    // Paper 1.17+ 的日志布局
    let path = dir.join("logs").join("latest.log");
    let path = if path.exists() { path } else { dir.join("latest.log") };
    let text = std::fs::read_to_string(path).map_err(|e| format!("读日志失败: {e}"))?;
    let n = lines.unwrap_or(200).min(2000);
    let all: Vec<&str> = text.lines().collect();
    Ok(all[all.len().saturating_sub(n)..]
        .iter()
        .map(|s| s.to_string())
        .collect())
}

/// 取得本机在局域网里的地址，用于显示「朋友用这个地址连进来」。
///
/// 优先 wlan0（WiFi）；数据网（移动网络）没法让人直连，所以只给 WiFi 地址。
#[tauri::command]
pub fn hosted_server_address(id: String) -> Option<String> {
    let s = get_server(&id).ok()?;
    let ip = local_wifi_ip()?;
    Some(format!("{ip}:{}", s.port))
}

/// 读 wlan0 的 IPv4（拿不到就 None，UI 显示「未连接 WiFi」）
fn local_wifi_ip() -> Option<String> {
    // 应用沙盒里没有 `ip` 命令，必须走 JNI 问 NetworkInterface
    #[cfg(target_os = "android")]
    {
        crate::android_bridge::wifi_ipv4()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// 列出本机所有 IPv4（非 127.0.0.1），用于排查「该用哪个地址连」
#[tauri::command]
pub fn hosted_server_addresses() -> Vec<String> {
    local_wifi_ip().into_iter().collect()
}
