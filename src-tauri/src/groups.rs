//! 实例分组：读写数据根目录下的 `instance_groups.json`。
//! 实例上的 `group` 字段存分组 id，删除分组时同步清空引用。

use crate::models::InstanceGroup;
use crate::settings::get_data_dir;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use uuid::Uuid;

const GROUPS_FILE: &str = "instance_groups.json";

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn groups_path() -> Result<std::path::PathBuf, String> {
    let data_dir = get_data_dir().await.map_err(|e| e.to_string())?;
    Ok(Path::new(&data_dir).join(GROUPS_FILE))
}

async fn load_all() -> HashMap<String, InstanceGroup> {
    let Ok(path) = groups_path().await else {
        return HashMap::new();
    };
    fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<InstanceGroup>>(&s).ok())
        .map(|list| {
            list.into_iter()
                .map(|g| (g.id.clone(), g))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
}

async fn save_all(map: &HashMap<String, InstanceGroup>) -> Result<(), String> {
    let path = groups_path().await?;
    let mut list: Vec<&InstanceGroup> = map.values().collect();
    list.sort_by_key(|g| g.created);
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    fs::write(&path, json).await.map_err(|e| e.to_string())
}

/// 分组列表（按创建时间排序）
pub async fn list_groups() -> Result<Vec<InstanceGroup>, String> {
    let map = load_all().await;
    let mut list: Vec<InstanceGroup> = map.into_values().collect();
    list.sort_by_key(|g| g.created);
    Ok(list)
}

/// 新建分组
pub async fn create_group(name: String, color: Option<String>) -> Result<InstanceGroup, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("分组名称不能为空".into());
    }
    let mut map = load_all().await;
    let group = InstanceGroup {
        id: Uuid::new_v4().to_string(),
        name,
        color: color.filter(|c| !c.trim().is_empty()),
        created: now_secs(),
    };
    map.insert(group.id.clone(), group.clone());
    save_all(&map).await?;
    Ok(group)
}

/// 重命名 / 改色分组
pub async fn rename_group(
    id: String,
    name: String,
    color: Option<String>,
) -> Result<InstanceGroup, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("分组名称不能为空".into());
    }
    let mut map = load_all().await;
    let group = map.get_mut(&id).ok_or_else(|| "分组不存在".to_string())?;
    group.name = name;
    group.color = color.filter(|c| !c.trim().is_empty());
    let out = group.clone();
    save_all(&map).await?;
    Ok(out)
}

/// 删除分组，同时清空所有实例上的 group 引用
pub async fn delete_group(id: String) -> Result<(), String> {
    let mut map = load_all().await;
    if map.remove(&id).is_none() {
        return Err("分组不存在".into());
    }
    save_all(&map).await?;

    // 清空实例上的分组引用
    let mut instances = crate::instances::list_instances().await.map_err(|e| e.to_string())?;
    let mut changed = false;
    for inst in instances.iter_mut() {
        if inst.group.as_deref() == Some(&id) {
            inst.group = None;
            changed = true;
        }
    }
    if changed {
        for inst in &instances {
            crate::instances::write_instance(inst).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 按给定顺序重排分组（返回新列表）
pub async fn reorder_groups(ids: Vec<String>) -> Result<Vec<InstanceGroup>, String> {
    let map = load_all().await;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(g) = map.get(&id) {
            out.push(g.clone());
        }
    }
    // 补上未出现在 ids 中的分组（保持稳定）
    let mut rest: Vec<InstanceGroup> = map
        .into_values()
        .filter(|g| !out.iter().any(|o| o.id == g.id))
        .collect();
    rest.sort_by_key(|g| g.created);
    out.extend(rest);
    save_all(
        &out.iter()
            .map(|g| (g.id.clone(), g.clone()))
            .collect::<HashMap<_, _>>(),
    )
    .await?;
    Ok(out)
}
