//! 皮肤中心：本地皮肤目录（`skins/`）管理、离线皮肤保存、以及通过
//! Mojang 官方 API 查询 / 上传皮肤与披风。

use crate::accounts;
use crate::settings::get_data_dir;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn client() -> reqwest::Client {
    crate::util::http_client_blocking()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize, Clone)]
pub struct SkinEntry {
    pub name: String,
    pub filename: String,
    pub path: String,
    pub size: u64,
    pub modified: u64,
}

async fn skins_dir() -> Result<PathBuf, String> {
    let data_dir = get_data_dir().await.map_err(|e| e.to_string())?;
    Ok(Path::new(&data_dir).join("skins"))
}

/// List all `.png` skins in the `skins` directory of the data root.
pub async fn list_skins() -> Result<Vec<SkinEntry>, String> {
    let dir = skins_dir().await?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建皮肤目录失败: {e}"))?;
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("读取皮肤目录失败: {e}"))?;
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("png")) != Some(true) {
            continue;
        }
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        out.push(SkinEntry {
            name,
            filename,
            path: path.to_string_lossy().to_string(),
            size: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

/// 读取皮肤文件并返回 data URL。
///
/// 入参两种形式：
///   - 裸文件名：只允许在 `<data>/skins/` 内解析；
///   - 路径（含 `/` 或 `\`）：**必须落在应用自己的目录里**（`<data>/` 或 `<data>/../cache/`，
///     后者是安卓端选择器把 `content://` 复制进来的 `<cache>/picked/`）。
///
/// 安全修复：以前只要参数里出现斜杠就 `PathBuf::from(filename)` 直接读，
/// 等于**任意文件读取** —— 传 `accounts/<uuid>.json` 就能把含 `msa_refresh_token`
/// 的账号文件当 base64 读回 WebView，传 `../../` 还能读到应用目录之外。
/// 现在额外要求内容是合法 PNG（皮肤本来也只能是 PNG）。
pub async fn read_skin_data_url(filename: String) -> Result<String, String> {
    let data_dir = get_data_dir().await.map_err(|e| e.to_string())?;
    let data_root = PathBuf::from(&data_dir);
    let skins = data_root.join("skins");
    std::fs::create_dir_all(&skins).map_err(|e| format!("创建皮肤目录失败: {e}"))?;

    let has_separator = filename.contains('/') || filename.contains('\\');
    let path = if has_separator {
        let canonical = PathBuf::from(&filename)
            .canonicalize()
            .map_err(|_| "皮肤文件不存在".to_string())?;

        // 允许的根目录：应用私有 files/ 与 cache/
        let mut allowed_roots: Vec<PathBuf> = vec![data_root.clone()];
        if let Some(parent) = data_root.parent() {
            allowed_roots.push(parent.join("cache"));
        }
        let permitted = allowed_roots.iter().any(|root| {
            root.canonicalize()
                .map(|r| canonical.starts_with(&r))
                .unwrap_or(false)
        });
        if !permitted {
            return Err("不允许读取应用目录之外的文件".into());
        }
        canonical
    } else {
        if !crate::util::is_safe_filename(&filename) {
            return Err("非法文件名".into());
        }
        crate::fsutil::resolve_in_dir(&skins, &filename, "皮肤")?
    };

    if !path.is_file() {
        return Err("皮肤文件不存在".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("读取皮肤文件失败: {e}"))?;
    if bytes.len() < 8 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("文件不是有效的 PNG".into());
    }
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}
fn safe_skin_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .collect()
}

/// Save a skin PNG (base64) to the skins directory.
pub async fn save_skin_from_data(name: String, data: String) -> Result<SkinEntry, String> {
    let raw = data.trim();
    let b64 = raw.strip_prefix("data:image/png;base64,").unwrap_or(raw);
    let bytes = STANDARD.decode(b64).map_err(|e| format!("解析皮肤数据失败: {e}"))?;
    if bytes.len() < 8 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("文件不是有效的 PNG".into());
    }
    let safe_name = safe_skin_name(&name);
    if safe_name.is_empty() {
        return Err("皮肤名称不能为空".into());
    }
    let dir = skins_dir().await?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建皮肤目录失败: {e}"))?;
    let path = dir.join(format!("{}.png", safe_name));
    std::fs::write(&path, &bytes).map_err(|e| format!("写入皮肤文件失败: {e}"))?;
    let meta = std::fs::metadata(&path).map_err(|e| format!("读取皮肤元信息失败: {e}"))?;
    Ok(SkinEntry {
        name: safe_name,
        filename: path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string(),
        path: path.to_string_lossy().to_string(),
        size: meta.len(),
        modified: now_secs(),
    })
}

/// Download a skin PNG from a URL and save it to the skins directory.
pub async fn download_skin_from_url(name: String, url: String) -> Result<SkinEntry, String> {
    let resp = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载皮肤失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载皮肤失败: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("读取皮肤数据失败: {e}"))?;
    if bytes.len() < 8 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("下载的内容不是有效的 PNG".into());
    }
    let safe_name = safe_skin_name(&name);
    if safe_name.is_empty() {
        return Err("皮肤名称不能为空".into());
    }
    let dir = skins_dir().await?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建皮肤目录失败: {e}"))?;
    let path = dir.join(format!("{}.png", safe_name));
    std::fs::write(&path, &bytes).map_err(|e| format!("写入皮肤文件失败: {e}"))?;
    let meta = std::fs::metadata(&path).map_err(|e| format!("读取皮肤元信息失败: {e}"))?;
    Ok(SkinEntry {
        name: safe_name,
        filename: path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string(),
        path: path.to_string_lossy().to_string(),
        size: meta.len(),
        modified: now_secs(),
    })
}

/// Delete a skin file by filename in the skins directory.
pub async fn delete_skin(filename: String) -> Result<(), String> {
    if !crate::util::is_safe_filename(&filename) {
        return Err("非法文件名".into());
    }
    let path = skins_dir().await?.join(&filename);
    if !path.exists() {
        return Err("皮肤文件不存在".into());
    }
    std::fs::remove_file(&path).map_err(|e| format!("删除皮肤失败: {e}"))
}

#[derive(Serialize)]
pub struct PlayerSkinResult {
    pub data_url: String,
    pub model: String,
    pub cape_data_url: Option<String>,
}

/// Fetch any URL as a PNG data URL (proxy for <img> 直连受限场景).
pub async fn fetch_image_data_url(url: String) -> Result<String, String> {
    let resp = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("读取失败: {e}"))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}

/// Fetch a player's skin by Minecraft username via Mojang API.
pub async fn fetch_player_skin(username: String) -> Result<PlayerSkinResult, String> {
    let trimmed = username.trim();
    if trimmed.is_empty() {
        return Err("玩家名不能为空".into());
    }
    let c = client();
    let profile_url = format!("https://api.mojang.com/users/profiles/minecraft/{}", trimmed);
    let profile: Value = c
        .get(&profile_url)
        .send()
        .await
        .map_err(|e| format!("查询玩家失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析玩家信息失败: {e}"))?;
    let uuid = profile
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("未找到该玩家（可能不存在或为离线账号）")?
        .to_string();
    let session_url = format!("https://sessionserver.mojang.com/session/minecraft/profile/{}", uuid);
    let session: Value = c
        .get(&session_url)
        .send()
        .await
        .map_err(|e| format!("获取会话信息失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析会话信息失败: {e}"))?;
    let props = session.get("properties").and_then(|v| v.as_array()).ok_or("玩家无皮肤信息")?;
    let mut skin_url: Option<String> = None;
    let mut skin_model = "classic".to_string();
    let mut cape_url: Option<String> = None;
    for p in props {
        if p.get("name").and_then(|v| v.as_str()) == Some("textures") {
            let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let decoded = STANDARD.decode(value).map_err(|e| format!("解码 textures 失败: {e}"))?;
            let json_str = String::from_utf8(decoded).map_err(|e| format!("textures 不是 UTF-8: {e}"))?;
            let tex: Value = serde_json::from_str(&json_str).map_err(|e| format!("解析 textures JSON 失败: {e}"))?;
            if let Some(url) = tex.pointer("/textures/SKIN/url").and_then(|v| v.as_str()) {
                skin_url = Some(url.to_string());
            }
            if let Some(model) = tex.pointer("/textures/SKIN/metadata/model").and_then(|v| v.as_str()) {
                skin_model = model.to_string();
            }
            if let Some(url) = tex.pointer("/textures/CAPE/url").and_then(|v| v.as_str()) {
                cape_url = Some(url.to_string());
            }
        }
    }
    let skin_url = skin_url.ok_or("该玩家未设置自定义皮肤")?;
    let bytes = c
        .get(&skin_url)
        .send()
        .await
        .map_err(|e| format!("下载皮肤图片失败: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("读取皮肤图片失败: {e}"))?;

    let cape_data_url = if let Some(cu) = cape_url {
        match c.get(&cu).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(cb) if cb.len() >= 8 && &cb[0..8] == b"\x89PNG\r\n\x1a\n" => {
                    Some(format!("data:image/png;base64,{}", STANDARD.encode(&cb)))
                }
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    };

    Ok(PlayerSkinResult {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(&bytes)),
        model: skin_model,
        cape_data_url,
    })
}

#[derive(Serialize)]
pub struct CapeInfo {
    pub id: String,
    pub name: String,
    pub data_url: String,
    pub active: bool,
}

/// Fetch all capes owned by a Microsoft account via Mojang API.
pub async fn fetch_player_capes(account_uuid: String) -> Result<Vec<CapeInfo>, String> {
    let account = accounts::get_account_by_uuid(&account_uuid)
        .await
        .map_err(|e| format!("账号不存在: {e}"))?;
    if account.account_type != crate::models::AccountType::Microsoft {
        return Ok(Vec::new());
    }
    let mc_token = accounts::microsoft_access_token(&account).await.map_err(|e| e.to_string())?;
    let resp = client()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_token)
        .send()
        .await
        .map_err(|e| format!("获取披风列表失败: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("获取披风列表失败 (HTTP {status}): {body}"));
    }
    let json: Value = resp.json().await.map_err(|e| format!("解析披风列表失败: {e}"))?;
    let raw_capes = json.get("capes").and_then(|v| v.as_array()).ok_or("披风列表格式异常")?;
    let mut result = Vec::new();
    for c in raw_capes {
        let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = c.get("alias").and_then(|v| v.as_str()).unwrap_or("未命名披风").to_string();
        let active = c
            .get("state")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("ACTIVE"))
            .unwrap_or(false);
        let url = match c.get("url").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => continue,
        };
        let bytes = match client().get(&url).send().await {
            Ok(r) if r.status().is_success() => match r.bytes().await {
                Ok(b) if b.len() >= 8 && &b[0..8] == b"\x89PNG\r\n\x1a\n" => b,
                _ => continue,
            },
            _ => continue,
        };
        result.push(CapeInfo {
            id,
            name,
            active,
            data_url: format!("data:image/png;base64,{}", STANDARD.encode(&bytes)),
        });
    }
    Ok(result)
}

/// Apply a cape to a Microsoft account. `cape_id` = None hides the cape.
pub async fn apply_cape_to_account(account_uuid: String, cape_id: Option<String>) -> Result<(), String> {
    let account = accounts::get_account_by_uuid(&account_uuid)
        .await
        .map_err(|e| format!("账号不存在: {e}"))?;
    if account.account_type != crate::models::AccountType::Microsoft {
        return Err("离线账号无法应用披风".into());
    }
    let mc_token = accounts::microsoft_access_token(&account).await.map_err(|e| e.to_string())?;
    let resp = if let Some(cid) = cape_id {
        client()
            .put("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .bearer_auth(&mc_token)
            .header("Content-Type", "application/json")
            .body(json!({ "capeId": cid }).to_string())
            .send()
            .await
            .map_err(|e| format!("应用披风失败: {e}"))?
    } else {
        client()
            .delete("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .bearer_auth(&mc_token)
            .send()
            .await
            .map_err(|e| format!("隐藏披风失败: {e}"))?
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("应用披风失败 (HTTP {status}): {body}"));
    }
    Ok(())
}

/// Upload a skin PNG to the player's Mojang account.
pub async fn apply_skin_to_account(account_uuid: String, skin_data: String, variant: String) -> Result<(), String> {
    let account = accounts::get_account_by_uuid(&account_uuid)
        .await
        .map_err(|e| format!("账号不存在: {e}"))?;
    if account.account_type != crate::models::AccountType::Microsoft {
        return Err("离线账号无法上传皮肤，仅支持正版账号".into());
    }
    let mc_token = accounts::microsoft_access_token(&account).await.map_err(|e| e.to_string())?;
    let raw = skin_data.trim();
    let b64 = raw.strip_prefix("data:image/png;base64,").unwrap_or(raw);
    let bytes = STANDARD.decode(b64).map_err(|e| format!("解析皮肤数据失败: {e}"))?;
    if bytes.len() < 8 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("文件不是有效的 PNG".into());
    }
    let v = if variant == "slim" { "slim" } else { "classic" };
    let file_part = reqwest::multipart::Part::bytes(bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| format!("构造上传数据失败: {e}"))?;
    let form = reqwest::multipart::Form::new()
        .text("variant", v.to_string())
        .part("file", file_part);
    let resp = client()
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(&mc_token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("上传皮肤失败: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("上传皮肤失败 (HTTP {status}): {body}"));
    }
    Ok(())
}

/// Save the offline skin PNG (injected into the version jar at launch time).
pub async fn apply_skin_offline(skin_data: String, variant: String, uuid: String) -> Result<(), String> {
    // uuid 未校验时 `format!("{uuid}.png")` 可以被 `../../x` 穿越写出 offline 目录
    crate::fsutil::validate_id(&uuid, "UUID")?;
    let raw = skin_data.trim();
    let b64 = raw.strip_prefix("data:image/png;base64,").unwrap_or(raw);
    let bytes = STANDARD.decode(b64).map_err(|e| format!("解析皮肤数据失败: {e}"))?;
    if bytes.len() < 8 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("文件不是有效的 PNG".into());
    }
    let dir = skins_dir().await?.join("offline");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建皮肤目录失败: {e}"))?;
    std::fs::write(dir.join(format!("{uuid}.png")), &bytes).map_err(|e| format!("保存皮肤失败: {e}"))?;
    let variant = if variant == "slim" { "slim" } else { "classic" };
    let meta = json!({ "variant": variant });
    let meta_str = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| r#"{"variant":"classic"}"#.to_string());
    std::fs::write(dir.join(format!("{uuid}.json")), meta_str).map_err(|e| format!("保存皮肤变体失败: {e}"))?;
    Ok(())
}

/// Read back a saved offline skin (PNG as a base64 data URL) plus its variant.
pub async fn get_offline_skin(uuid: String) -> Result<Option<Value>, String> {
    crate::fsutil::validate_id(&uuid, "UUID")?;
    let dir = skins_dir().await?.join("offline");
    let png_path = dir.join(format!("{uuid}.png"));
    if !png_path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&png_path).map_err(|e| format!("读取皮肤失败: {e}"))?;
    let src = format!("data:image/png;base64,{}", STANDARD.encode(&bytes));
    let variant = std::fs::read_to_string(dir.join(format!("{uuid}.json")))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("variant").and_then(|s| s.as_str()).map(String::from))
        .filter(|v| v == "slim" || v == "classic");
    Ok(Some(json!({ "src": src, "variant": variant })))
}
