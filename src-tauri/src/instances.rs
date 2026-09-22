use std::path::Path;
use tokio::fs;
use anyhow::{Context, Result};
use uuid::Uuid;
use chrono::Utc;
use crate::models::*;

pub async fn list_instances() -> Result<Vec<MinecraftProfile>> {
    let data_dir = crate::settings::get_data_dir().await?;
    let instances_dir = Path::new(&data_dir).join("instances");

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
    let data_dir = crate::settings::get_data_dir().await?;
    let file_path = Path::new(&data_dir)
        .join("instances")
        .join(instance_id)
        .join("instance.json");

    let content = fs::read_to_string(&file_path).await
        .context("Failed to read instance")?;

    Ok(serde_json::from_str(&content)?)
}

/// 将实例元数据写回 instance.json（保持目录结构不变）
pub async fn write_instance(instance: &MinecraftProfile) -> Result<()> {
    crate::fsutil::validate_id(&instance.id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let data_dir = crate::settings::get_data_dir().await?;
    let file_path = Path::new(&data_dir)
        .join("instances")
        .join(&instance.id)
        .join("instance.json");
    let content = serde_json::to_string_pretty(instance)?;
    fs::write(&file_path, content).await
        .with_context(|| format!("Failed to write instance file: {}", file_path.display()))?;
    Ok(())
}

pub async fn create_instance(config: serde_json::Value) -> Result<MinecraftProfile> {
    let data_dir = crate::settings::get_data_dir().await?;
    let instance_id = Uuid::new_v4().to_string();
    let instance_dir = Path::new(&data_dir)
        .join("instances")
        .join(&instance_id);

    fs::create_dir_all(&instance_dir).await
        .context("Failed to create instance directory")?;

    let instance = MinecraftProfile {
        id: instance_id.clone(),
        name: config["name"].as_str().unwrap_or("New Instance").to_string(),
        mc_version: config["mcVersion"].as_str().unwrap_or("").to_string(),
        loader: match config["loader"].as_str().unwrap_or("vanilla") {
            "fabric" => Loader::Fabric,
            "quilt" => Loader::Quilt,
            "forge" => Loader::Forge,
            "neoforge" => Loader::NeoForge,
            _ => Loader::Vanilla,
        },
        loader_version: config["loaderVersion"].as_str().map(|s| s.to_string()),
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
    };

    let file_path = instance_dir.join("instance.json");
    let content = serde_json::to_string_pretty(&instance)?;
    fs::write(&file_path, content).await
        .context("Failed to write instance file")?;

    Ok(instance)
}

pub async fn delete_instance(instance_id: &str, keep_dir: bool) -> Result<()> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let data_dir = crate::settings::get_data_dir().await?;
    let instance_dir = Path::new(&data_dir)
        .join("instances")
        .join(instance_id);

    if !keep_dir && instance_dir.exists() {
        fs::remove_dir_all(&instance_dir).await
            .context("Failed to remove instance directory")?;
    }

    Ok(())
}

pub async fn update_instance(patch: serde_json::Value) -> Result<MinecraftProfile> {
    let data_dir = crate::settings::get_data_dir().await?;
    let instance_id = patch.get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing instance id in patch"))?;
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;

    let file_path = Path::new(&data_dir)
        .join("instances")
        .join(instance_id)
        .join("instance.json");

    let content = fs::read_to_string(&file_path).await
        .context("Failed to read instance")?;
    let mut instance: MinecraftProfile = serde_json::from_str(&content)?;

    // Apply patch fields
    if let Some(v) = patch.get("name").and_then(|v| v.as_str()) {
        instance.name = v.to_string();
    }
    if let Some(v) = patch.get("icon").and_then(|v| v.as_str()) {
        instance.icon = Some(v.to_string());
    }
    if patch.get("group").is_some() {
        instance.group = patch.get("group").and_then(|v| v.as_str()).map(|s| s.to_string());
    }
    if let Some(v) = patch.get("accountId").and_then(|v| v.as_str()) {
        instance.account_id = Some(v.to_string());
    }
    if let Some(v) = patch.get("javaDir").and_then(|v| v.as_str()) {
        instance.java_dir = v.to_string();
    }
    if let Some(v) = patch.get("javaArgs").and_then(|v| v.as_str()) {
        instance.java_args = Some(v.to_string());
    }
    if let Some(v) = patch.get("gameArgs").and_then(|v| v.as_str()) {
        instance.game_args = Some(v.to_string());
    }
    if let Some(v) = patch.get("maxMemoryMb").and_then(|v| v.as_i64()) {
        instance.max_memory_mb = Some(v as i32);
    }
    if let Some(v) = patch.get("memoryMode").and_then(|v| v.as_str()) {
        instance.memory_mode = Some(v.to_string());
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

    let data_dir = crate::settings::get_data_dir().await.map_err(|e| e.to_string())?;
    let dir = Path::new(&data_dir)
        .join("instances")
        .join(instance_id)
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
