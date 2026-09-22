use std::collections::HashMap;
use std::path::Path;
use serde_json::Value;
use tokio::fs;
use anyhow::{Context, Result};
use crate::models::*;

/// 设置文件的键名统一使用 snake_case（与前端 `Settings` 类型一致）。
/// 早期版本写过 camelCase，这里按「snake_case 优先、camelCase 兜底」读取，保证老文件不丢设置。
pub fn raw_str(map: &HashMap<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| map.get(*k))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn raw_i64(map: &HashMap<String, Value>, keys: &[&str], default: i64) -> i64 {
    keys.iter()
        .find_map(|k| map.get(*k))
        .and_then(|v| v.as_i64())
        .unwrap_or(default)
}

fn raw_bool(map: &HashMap<String, Value>, keys: &[&str], default: bool) -> bool {
    keys.iter()
        .find_map(|k| map.get(*k))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// 同步版数据目录解析：供无法 `await` 的调用点（如只能同步构造 HTTP 客户端的地方）使用。
/// 与 [`get_data_dir`] 保持同一套包名解析规则。
pub fn data_dir_sync() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        android_package_name().map(|pkg| format!("/data/data/{pkg}/files"))
    }
    #[cfg(not(target_os = "android"))]
    {
        dirs::data_dir().map(|d| d.join("Qookix").to_string_lossy().to_string())
    }
}

/// 同步读取设置文件原始 JSON（不存在或损坏时返回空表）。
pub fn load_raw_settings_sync() -> HashMap<String, Value> {
    let Some(dir) = data_dir_sync() else {
        return HashMap::new();
    };
    let settings_path = Path::new(&dir).join("settings.json");
    match std::fs::read_to_string(&settings_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// 读取设置文件原始 JSON（不存在或损坏时返回空表）。
pub async fn load_raw_settings() -> HashMap<String, Value> {
    let Ok(data_dir) = get_data_dir().await else {
        return HashMap::new();
    };
    let settings_path = Path::new(&data_dir).join("settings.json");
    if !settings_path.exists() {
        return HashMap::new();
    }
    match fs::read_to_string(&settings_path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// 写入设置文件原始 JSON。
pub async fn save_raw_settings(map: &HashMap<String, Value>) -> Result<()> {
    let data_dir = get_data_dir().await?;
    let settings_path = Path::new(&data_dir).join("settings.json");
    let content = serde_json::to_string_pretty(map)?;
    fs::write(&settings_path, content)
        .await
        .with_context(|| format!("Failed to write settings file: {}", settings_path.display()))?;
    // 代理/镜像等设置改变后让缓存的 HTTP 客户端立即重建
    crate::util::reset_http_client();
    Ok(())
}

pub async fn get_settings() -> Result<Settings> {
    let data_dir = get_data_dir().await?;
    let settings_path = Path::new(&data_dir).join("settings.json");

    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path).await
            .context("Failed to read settings file")?;
        let settings: HashMap<String, Value> = serde_json::from_str(&content)
            .context("Failed to parse settings")?;

        Ok(Settings {
            data_dir: data_dir.clone(),
            java_path: raw_str(&settings, &["java_path", "javaPath"]),
            max_memory_mb: raw_i64(&settings, &["max_memory_mb", "maxMemoryMb"], 2048) as i32,
            min_memory_mb: raw_i64(&settings, &["min_memory_mb", "minMemoryMb"], 512) as i32,
            memory_mode: raw_str(&settings, &["memory_mode", "memoryMode"]).unwrap_or_else(|| "auto".into()),
            jvm_args: raw_str(&settings, &["jvm_args", "jvmArgs"]).unwrap_or_default(),
            game_args: raw_str(&settings, &["game_args", "gameArgs"]).unwrap_or_default(),
            download_threads: raw_i64(&settings, &["download_threads", "downloadThreads"], 4) as i32,
            download_chunk_threads: raw_i64(&settings, &["download_chunk_threads", "downloadChunkThreads"], 2) as i32,
            curseforge_api_key: raw_str(&settings, &["curseforge_api_key", "curseforgeApiKey"]),
            theme: raw_str(&settings, &["theme"]).unwrap_or_else(|| "dark".into()),
            theme_color: raw_str(&settings, &["theme_color", "themeColor"]).unwrap_or_else(|| "#E89A4B".into()),
            close_behavior: raw_str(&settings, &["close_behavior", "closeBehavior"]).unwrap_or_else(|| "minimize".into()),
            auto_launch: raw_bool(&settings, &["auto_launch", "autoLaunch"], false),
            keep_open: raw_bool(&settings, &["keep_open", "keepOpen"], true),
            ms_client_id: raw_str(&settings, &["ms_client_id", "msClientId"]).unwrap_or_else(|| "00000000402b5328".into()),
            selected_account: raw_str(&settings, &["selected_account", "selectedAccount"]),
            proxy_mode: raw_str(&settings, &["proxy_mode", "proxyMode"]).unwrap_or_else(|| "system".into()),
            proxy: raw_str(&settings, &["proxy"]),
            mirror: raw_str(&settings, &["mirror"]).unwrap_or_else(|| "bmclapi".into()),
            mirror_custom: raw_str(&settings, &["mirror_custom", "mirrorCustom"]),
            background_image: raw_str(&settings, &["background_image", "backgroundImage"]),
            background_blur: raw_i64(&settings, &["background_blur", "backgroundBlur"], 0) as i32,
            background_dim: raw_i64(&settings, &["background_dim", "backgroundDim"], 0) as i32,
            glass_blur: raw_i64(&settings, &["glass_blur", "glassBlur"], 0) as i32,
            ui_scale: raw_i64(&settings, &["ui_scale", "uiScale"], 100).clamp(60, 200) as i32,
            show_home_hero: raw_bool(&settings, &["show_home_hero", "showHomeHero"], true),
            show_sidebar_collapse_btn: raw_bool(&settings, &["show_sidebar_collapse_btn", "showSidebarCollapseBtn"], true),
            show_news: raw_bool(&settings, &["show_news", "showNews"], true),
            dismissed_update_version: raw_str(&settings, &["dismissed_update_version", "dismissedUpdateVersion"]),
            auto_update: raw_bool(&settings, &["auto_update", "autoUpdate"], true),
            update_source: raw_str(&settings, &["update_source", "updateSource"]).unwrap_or_else(|| "bucket".into()),
            orientation: raw_str(&settings, &["orientation"]).unwrap_or_else(|| "landscape".into()),
            // 归一化：只认 "left"，其余（含写坏的值）都当 "bottom"，
            // 免得前端拿到一个不认识的值后两套布局都不生效。
            nav_position: match raw_str(&settings, &["nav_position", "navPosition"])
                .unwrap_or_else(|| "bottom".into())
                .as_str()
            {
                "left" => "left".into(),
                _ => "bottom".into(),
            },
            nav_offset: raw_i64(&settings, &["nav_offset", "navOffset"], 0).clamp(0, 120) as i32,
            nav_gap_top: raw_i64(&settings, &["nav_gap_top", "navGapTop"], 0).clamp(0, 800) as i32,
            nav_gap_h: raw_i64(&settings, &["nav_gap_h", "navGapH"], 0).clamp(0, 240) as i32,
            // 归一化：只认五种合法值，其余（含写坏的值）都回落 "auto"，
            // 免得前端拿到不认识的类型后避让逻辑整段失效。
            nav_cutout: match raw_str(&settings, &["nav_cutout", "navCutout"])
                .unwrap_or_else(|| "auto".into())
                .as_str()
            {
                "none" | "center" | "topleft" | "topright" | "notch" => {
                    raw_str(&settings, &["nav_cutout", "navCutout"]).unwrap()
                }
                _ => "auto".into(),
            },
            // 触控目标档位：同样只认三个合法值，写坏的值一律回落默认的紧凑档
            touch_target: match raw_str(&settings, &["touch_target", "touchTarget"])
                .unwrap_or_else(|| "compact".into())
                .as_str()
            {
                "standard" | "large" => raw_str(&settings, &["touch_target", "touchTarget"]).unwrap(),
                _ => "compact".into(),
            },
        })
    } else {
        // Default settings
        Ok(Settings {
            data_dir: data_dir.clone(),
            java_path: None,
            max_memory_mb: 2048,
            min_memory_mb: 512,
            memory_mode: "auto".to_string(),
            jvm_args: String::new(),
            game_args: String::new(),
            download_threads: 4,
            download_chunk_threads: 2,
            curseforge_api_key: None,
            theme: "dark".to_string(),
            theme_color: "#E89A4B".to_string(),
            close_behavior: "minimize".to_string(),
            auto_launch: false,
            keep_open: true,
            ms_client_id: "00000000402b5328".to_string(),
            selected_account: None,
            proxy_mode: "system".to_string(),
            proxy: None,
            mirror: "bmclapi".to_string(),
            mirror_custom: None,
            background_image: None,
            background_blur: 0,
            background_dim: 0,
            glass_blur: 0,
            ui_scale: 100,
            show_home_hero: true,
            show_sidebar_collapse_btn: true,
            show_news: true,
            dismissed_update_version: None,
            auto_update: true,
            update_source: "bucket".to_string(),
            orientation: "landscape".to_string(),
            nav_position: "bottom".to_string(),
            nav_offset: 0,
            nav_gap_top: 0,
            nav_gap_h: 0,
            nav_cutout: "auto".to_string(),
            touch_target: "compact".to_string(),
        })
    }
}

pub async fn update_settings(settings: Settings) -> Result<()> {
    let data_dir = get_data_dir().await?;
    let settings_path = Path::new(&data_dir).join("settings.json");

    let mut map = HashMap::new();
    map.insert("java_path".to_string(), Value::String(settings.java_path.unwrap_or_default()));
    map.insert("max_memory_mb".to_string(), Value::Number(settings.max_memory_mb.into()));
    map.insert("min_memory_mb".to_string(), Value::Number(settings.min_memory_mb.into()));
    map.insert("memory_mode".to_string(), Value::String(settings.memory_mode));
    map.insert("jvm_args".to_string(), Value::String(settings.jvm_args));
    map.insert("game_args".to_string(), Value::String(settings.game_args));
    map.insert("download_threads".to_string(), Value::Number(settings.download_threads.into()));
    map.insert("download_chunk_threads".to_string(), Value::Number(settings.download_chunk_threads.into()));
    map.insert("curseforge_api_key".to_string(), Value::String(settings.curseforge_api_key.unwrap_or_default()));
    map.insert("theme".to_string(), Value::String(settings.theme));
    map.insert("theme_color".to_string(), Value::String(settings.theme_color));
    map.insert("close_behavior".to_string(), Value::String(settings.close_behavior));
    map.insert("auto_launch".to_string(), Value::Bool(settings.auto_launch));
    map.insert("keep_open".to_string(), Value::Bool(settings.keep_open));
    map.insert("ms_client_id".to_string(), Value::String(settings.ms_client_id));
    map.insert("selected_account".to_string(), Value::String(settings.selected_account.unwrap_or_default()));
    map.insert("proxy_mode".to_string(), Value::String(settings.proxy_mode));
    map.insert("proxy".to_string(), Value::String(settings.proxy.unwrap_or_default()));
    map.insert("mirror".to_string(), Value::String(settings.mirror));
    map.insert("mirror_custom".to_string(), Value::String(settings.mirror_custom.unwrap_or_default()));
    map.insert("background_image".to_string(), Value::String(settings.background_image.unwrap_or_default()));
    map.insert("background_blur".to_string(), Value::Number(settings.background_blur.into()));
    map.insert("background_dim".to_string(), Value::Number(settings.background_dim.into()));
    map.insert("glass_blur".to_string(), Value::Number(settings.glass_blur.into()));
    map.insert("show_home_hero".to_string(), Value::Bool(settings.show_home_hero));
    map.insert("show_sidebar_collapse_btn".to_string(), Value::Bool(settings.show_sidebar_collapse_btn));
    map.insert("show_news".to_string(), Value::Bool(settings.show_news));
    map.insert("dismissed_update_version".to_string(), Value::String(settings.dismissed_update_version.unwrap_or_default()));
    map.insert("auto_update".to_string(), Value::Bool(settings.auto_update));
    map.insert("update_source".to_string(), Value::String(settings.update_source));
    map.insert("orientation".to_string(), Value::String(settings.orientation));

    let content = serde_json::to_string_pretty(&map)?;
    fs::write(&settings_path, content).await
        .with_context(|| format!("Failed to write settings file: {}", settings_path.display()))?;
    crate::util::reset_http_client();

    Ok(())
}

/// 以 patch 模式更新设置：读取现有设置 → 合并 patch → 写回
pub async fn update_settings_patch(patch: Value) -> Result<()> {
    let data_dir = get_data_dir().await?;
    let settings_path = Path::new(&data_dir).join("settings.json");

    // 读取现有设置（JSON map）
    let mut map: HashMap<String, Value> = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path).await
            .context("Failed to read settings file")?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // 合并 patch
    if let Some(obj) = patch.as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.clone());
        }
    }

    let content = serde_json::to_string_pretty(&map)?;
    fs::write(&settings_path, content).await
        .with_context(|| format!("Failed to write settings file: {}", settings_path.display()))?;
    crate::util::reset_http_client();

    Ok(())
}

/// Android 上运行进程对应的包名（`/proc/self/cmdline` 的首段，形如 `com.zhayi.qookix`）。
/// debug 变体带 `.debug` 后缀，因此不能把包名写死。
#[cfg(target_os = "android")]
fn android_package_name() -> Option<String> {
    let raw = std::fs::read("/proc/self/cmdline").ok()?;
    let first = raw.split(|b| *b == 0).next()?;
    let name = String::from_utf8_lossy(first).to_string();
    let name = name.split(':').next().unwrap_or("").to_string();
    let valid = !name.is_empty()
        && name.contains('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
    if valid {
        Some(name)
    } else {
        None
    }
}

pub async fn get_data_dir() -> Result<String> {
    // Use platform-specific data directory
    #[cfg(target_os = "android")]
    {
        // 应用私有目录 /data/data/<package>/files。
        // 包名动态解析，保证 release / debug(带 .debug 后缀) 变体各写各的目录。
        let dir = match android_package_name() {
            Some(pkg) => format!("/data/data/{pkg}/files"),
            None => "/data/data/com.zhayi.qookix/files".to_string(),
        };
        if !Path::new(&dir).exists() {
            fs::create_dir_all(&dir).await.ok();
        }
        Ok(dir)
    }
    #[cfg(not(target_os = "android"))]
    {
        // On desktop, use the user's data directory
        let dirs = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?;
        Ok(dirs.join("Qookix").to_string_lossy().to_string())
    }
}
