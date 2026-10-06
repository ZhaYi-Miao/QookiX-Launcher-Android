use tokio::fs;
use anyhow::{Context, Result};
use uuid::Uuid;
use chrono::Utc;
use crate::models::*;

pub async fn list_instances() -> Result<Vec<MinecraftProfile>> {
    let instances_dir = crate::settings::instances_root().await?;

    if !instances_dir.exists() {
        fs::create_dir_all(&instances_dir).await.ok();
        return Ok(Vec::new());
    }

    let mut instances = Vec::new();

    let mut entries = fs::read_dir(&instances_dir).await
        .context("Failed to read instances directory")?;
    
    while let Some(entry) = entries.next_entry().await
        .context("Failed to read next entry")? {
        let file_path = entry.path().join("instance.json");
        if file_path.exists() {
            let content = fs::read_to_string(&file_path).await
                .context("Failed to read instance file")?;
            let instance: MinecraftProfile = serde_json::from_str(&content)
                .context("Failed to parse instance")?;
            instances.push(instance);
        }
    }

    // Sort by last played
    instances.sort_by(|a, b| {
        b.last_played.unwrap_or(0)
            .cmp(&a.last_played.unwrap_or(0))
    });

    Ok(instances)
}

pub async fn get_instance(instance_id: &str) -> Result<MinecraftProfile> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let file_path = crate::settings::instance_dir(instance_id)
        .await?
        .join("instance.json");

    let content = fs::read_to_string(&file_path).await
        .context("Failed to read instance")?;

    Ok(serde_json::from_str(&content)?)
}

/// 将实例元数据写回 instance.json（保持目录结构不变）
pub async fn write_instance(instance: &MinecraftProfile) -> Result<()> {
    crate::fsutil::validate_id(&instance.id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let file_path = crate::settings::instance_dir(&instance.id)
        .await?
        .join("instance.json");
    let content = serde_json::to_string_pretty(instance)?;
    fs::write(&file_path, content).await
        .with_context(|| format!("Failed to write instance file: {}", file_path.display()))?;
    Ok(())
}

pub async fn create_instance(config: serde_json::Value) -> Result<MinecraftProfile> {
    let instances_root = crate::settings::instances_root().await?;
    let instance_id = Uuid::new_v4().to_string();
    let instance_dir = instances_root.join(&instance_id);

    fs::create_dir_all(&instance_dir).await
        .context("Failed to create instance directory")?;

    let mc_version = config["mcVersion"].as_str().unwrap_or("").to_string();
    // 名称留空是允许的（创建页输入框的提示就是「留空则自动用版本号命名」）。
    // 这里兜底成游戏版本号，否则会建出一个名字为空的实例，列表里看着像空白项。
    let name = match config["name"].as_str().unwrap_or("").trim() {
        "" => mc_version.clone(),
        given => given.to_string(),
    };

    let instance = MinecraftProfile {
        id: instance_id.clone(),
        name,
        mc_version,
        loader: match config["loader"].as_str().unwrap_or("vanilla") {
            "fabric" => Loader::Fabric,
            "quilt" => Loader::Quilt,
            "forge" => Loader::Forge,
            "neoforge" => Loader::NeoForge,
            _ => Loader::Vanilla,
        },
        loader_version: config["loaderVersion"].as_str().map(|s| s.to_string()),
        alias: None,
        created: Utc::now().timestamp(),
        last_played: None,
        total_play_time: 0,
        game_dir: instance_dir.to_string_lossy().to_string(),
        java_dir: config["javaDir"].as_str().unwrap_or("").to_string(),
        java_args: config["javaArgs"].as_str().map(|s| s.to_string()),
        game_args: config["gameArgs"].as_str().map(|s| s.to_string()),
        resolution: if let (Some(w), Some(h)) = (config["resolution"][0].as_i64(), config["resolution"][1].as_i64()) {
            Some((w as i32, h as i32))
        } else {
            Some((854, 480))
        },
        max_memory_mb: config["maxMemoryMb"].as_i64().map(|v| v as i32),
        memory_mode: config["memoryMode"].as_str().map(|s| s.to_string()),
        account_id: config["accountId"].as_str().map(|s| s.to_string()),
        icon: config["icon"].as_str().map(|s| s.to_string()),
        mods: Vec::new(),
        resource_packs: Vec::new(),
        shaders: Vec::new(),
        group: config["group"].as_str().map(|s| s.to_string()),
        is_symlink: config["isSymlink"].as_bool(),
        source_path: config["sourcePath"].as_str().map(|s| s.to_string()),
        installed: false,
        // 渲染器默认「自动」：按 MC 版本挑（26.x → MobileGlues，其余 → GL4ES），
        // 见 launch::resolve_renderer。创建实例时也允许直接指定。
        renderer: config["renderer"].as_str().map(|s| s.to_string()),
        // 默认开启：启动时检查文件完整性、缺了自动补全。
        check_files_on_launch: config["checkFilesOnLaunch"].as_bool(),
    };

    let file_path = instance_dir.join("instance.json");
    let content = serde_json::to_string_pretty(&instance)?;
    fs::write(&file_path, content).await
        .context("Failed to write instance file")?;

    Ok(instance)
}

pub async fn delete_instance(instance_id: &str, keep_dir: bool) -> Result<()> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let instance_dir = crate::settings::instance_dir(instance_id).await?;

    if !keep_dir && instance_dir.exists() {
        fs::remove_dir_all(&instance_dir).await
            .context("Failed to remove instance directory")?;
    }

    Ok(())
}

/// 从 patch 里按键名取值，**同时兼容两种前端命名**。
///
/// Windows 侧前端与后端约定 camelCase（`maxMemoryMb`），Android 侧前端跟随实例
/// JSON 的 snake_case（`max_memory_mb`）。只认一种就会出现「改了没保存」——
/// 界面显示已保存、盘上还是旧值，用户完全看不出来。
fn pick<'a>(patch: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    keys.iter().find_map(|k| patch.get(*k))
}

pub async fn update_instance(patch: serde_json::Value) -> Result<MinecraftProfile> {
    let instance_id = patch.get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing instance id in patch"))?;
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;

    let file_path = crate::settings::instance_dir(instance_id)
        .await?
        .join("instance.json");

    let content = fs::read_to_string(&file_path).await
        .context("Failed to read instance")?;
    let mut instance: MinecraftProfile = serde_json::from_str(&content)?;

    // Apply patch fields
    if let Some(v) = pick(&patch, &["name"]).and_then(|v| v.as_str()) {
        instance.name = v.to_string();
    }
    // 实例别名（`qookix://launch/<别名>` 用）。与桌面版 `instances.rs` 同一套校验：
    // 小写、仅字母数字与 `-` / `_`、全局唯一、空串表示清除。
    // 这里以前**完全没有这段** —— 设置页的「保存」是静默丢弃，用户以为改好了。
    if let Some(v) = pick(&patch, &["alias"]) {
        let raw = v.as_str().unwrap_or("").trim().to_ascii_lowercase();
        if raw.is_empty() {
            instance.alias = None;
        } else {
            if !raw
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(anyhow::anyhow!("别名只能包含英文字母、数字、- 和 _"));
            }
            // 冲突检查（排除自己）
            for other in list_instances().await.unwrap_or_default() {
                if other.id != instance.id && other.alias.as_deref() == Some(raw.as_str()) {
                    return Err(anyhow::anyhow!("别名已被实例「{}」占用", other.name));
                }
            }
            instance.alias = Some(raw);
        }
    }
    if let Some(v) = pick(&patch, &["icon"]).and_then(|v| v.as_str()) {
        instance.icon = Some(v.to_string());
    }
    if let Some(v) = pick(&patch, &["group"]) {
        instance.group = v.as_str().map(|s| s.to_string());
    }
    if let Some(v) = pick(&patch, &["accountId", "account_id"]).and_then(|v| v.as_str()) {
        instance.account_id = Some(v.to_string());
    }
    if let Some(v) = pick(&patch, &["javaDir", "java_path", "javaPath"]).and_then(|v| v.as_str()) {
        instance.java_dir = v.to_string();
    }
    if let Some(v) = pick(&patch, &["javaArgs", "jvm_args"]).and_then(|v| v.as_str()) {
        instance.java_args = Some(v.to_string());
    }
    if let Some(v) = pick(&patch, &["gameArgs", "game_args"]).and_then(|v| v.as_str()) {
        instance.game_args = Some(v.to_string());
    }
    if let Some(v) = pick(&patch, &["maxMemoryMb", "max_memory_mb"]).and_then(|v| v.as_i64()) {
        instance.max_memory_mb = Some(v as i32);
    }
    if let Some(v) = pick(&patch, &["memoryMode", "memory_mode"]).and_then(|v| v.as_str()) {
        instance.memory_mode = Some(v.to_string());
    }
    // 分辨率：传 `[宽, 高]` 表示设置，传 null 表示清空（回到启动时的默认值）。
    if let Some(v) = pick(&patch, &["resolution"]) {
        instance.resolution = match (v.as_array(), v.is_null()) {
            (Some(arr), _) if arr.len() == 2 => match (arr[0].as_i64(), arr[1].as_i64()) {
                (Some(w), Some(h)) => Some((w as i32, h as i32)),
                _ => instance.resolution,
            },
            _ => None,
        };
    }
    // 渲染器：`auto` / `global` / `opengles2` / `mobileglues` / `vulkan_zink`。
    // 传 null 表示回到「自动」（旧实例文件本来就没有这个键）。
    if let Some(v) = pick(&patch, &["renderer"]) {
        instance.renderer = v.as_str().map(|s| s.to_string());
    }
    // 启动前检查文件完整性。前端只发 true/false，这里固定存 Some(bool)，
    // 避免写入 null 后又落到「旧实例 → 默认开启」的分支上，关不掉。
    if let Some(v) = pick(&patch, &["checkFilesOnLaunch", "check_files_on_launch"]).and_then(|v| v.as_bool()) {
        instance.check_files_on_launch = Some(v);
    }

    let content = serde_json::to_string_pretty(&instance)?;
    fs::write(&file_path, content).await
        .context("Failed to write instance file")?;

    Ok(instance)
}

// ==================== Installed Content Tracking ====================

/// 内容目录名（mods / resourcepacks / shaderpacks）
pub fn kind_folder(kind: &str) -> &'static str {
    match kind {
        "shader" => "shaderpacks",
        "resourcepack" => "resourcepacks",
        _ => "mods",
    }
}

/// 读取某实例某类内容的记录列表（与磁盘目录无关的元数据）。
pub async fn list_content_records(instance_id: &str, kind: &str) -> Vec<InstalledContent> {
    let Ok(inst) = get_instance(instance_id).await else {
        return Vec::new();
    };
    match kind {
        "resourcepack" => inst.resource_packs,
        "shader" => inst.shaders,
        _ => inst.mods,
    }
}

/// 新增/覆盖一条内容记录（同名覆盖）。
pub async fn add_content(instance_id: &str, kind: &str, record: InstalledContent) -> Result<(), String> {
    crate::fsutil::validate_id(instance_id, "实例")?;
    let mut inst = get_instance(instance_id).await.map_err(|e| e.to_string())?;
    let list = content_list_mut(&mut inst, kind);
    list.retain(|c| c.filename != record.filename);
    list.push(record);
    write_instance(&inst).await.map_err(|e| e.to_string())
}

/// 批量新增/覆盖内容记录。
pub async fn add_content_batch(
    instance_id: &str,
    kind: &str,
    records: Vec<InstalledContent>,
) -> Result<(), String> {
    let mut inst = get_instance(instance_id).await.map_err(|e| e.to_string())?;
    let list = content_list_mut(&mut inst, kind);
    for rec in records {
        list.retain(|c| c.filename != rec.filename);
        list.push(rec);
    }
    write_instance(&inst).await.map_err(|e| e.to_string())
}

/// 按文件名删除一条内容记录（不删除磁盘文件）。
pub async fn remove_content(instance_id: &str, kind: &str, filename: &str) -> Result<(), String> {
    crate::fsutil::validate_id(instance_id, "实例")?;
    crate::fsutil::validate_name(filename)?;
    let mut inst = get_instance(instance_id).await.map_err(|e| e.to_string())?;
    let list = content_list_mut(&mut inst, kind);
    list.retain(|c| c.filename != filename);
    write_instance(&inst).await.map_err(|e| e.to_string())
}

/// 启用/禁用内容：重命名文件为 `name.disabled` 并同步记录。
pub async fn set_content_enabled(
    instance_id: &str,
    kind: &str,
    filename: &str,
    enabled: bool,
) -> Result<(), String> {
    if !crate::util::is_safe_filename(filename) {
        return Err("非法文件名".into());
    }
    let mut inst = get_instance(instance_id).await.map_err(|e| e.to_string())?;
    let item = content_list_mut(&mut inst, kind)
        .iter_mut()
        .find(|c| c.filename == filename)
        .ok_or_else(|| "内容记录不存在".to_string())?;

    let dir = crate::settings::instance_dir(instance_id)
        .await
        .map_err(|e| e.to_string())?
        .join(kind_folder(kind));
    let active = dir.join(filename);
    let disabled = dir.join(format!("{filename}.disabled"));
    if enabled {
        if !active.is_file() && disabled.is_file() {
            std::fs::rename(&disabled, &active).map_err(|e| e.to_string())?;
        }
    } else if active.is_file() {
        std::fs::rename(&active, &disabled).map_err(|e| e.to_string())?;
    }
    item.enabled = enabled;
    write_instance(&inst).await.map_err(|e| e.to_string())
}

fn content_list_mut<'a>(inst: &'a mut MinecraftProfile, kind: &str) -> &'a mut Vec<InstalledContent> {
    match kind {
        "resourcepack" => &mut inst.resource_packs,
        "shader" => &mut inst.shaders,
        _ => &mut inst.mods,
    }
}
