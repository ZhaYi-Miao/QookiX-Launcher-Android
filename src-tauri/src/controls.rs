//! 控制布局里「按键透传」的读取与修改。
//!
//! 透传 = `ControlData.passThruEnabled`：按住这个键时，手指滑动会转发给游戏画面，
//! 也就是「按住跳跃也能拖视角」。默认布局里只有跳跃是开着的，其余的让用户自己选。
//!
//! 改的是**当前布局文件**（pojav 偏好 `defaultCtrl` 指向的那份，默认
//! `<files>/controlmap/default.json`），游戏下次启动读取时生效。

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlButtonInfo {
    /// 在 `mControlDataList` 里的下标（写回时用它定位）
    pub index: usize,
    pub name: String,
    pub pass_thru: bool,
}

/// 当前生效的控制布局文件；找不到就报错（前端显示为「还没生成布局」）。
fn layout_path() -> Result<PathBuf, String> {
    // pojav 偏好里记着用户选的布局（「选择默认控制布局」会写它）
    if let Ok(json) = crate::android_bridge::read_pojav_prefs() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(path) = value.get("defaultCtrl").and_then(|v| v.as_str()) {
                let path = PathBuf::from(path);
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }

    let data_dir = crate::settings::data_dir_sync().ok_or_else(|| "拿不到数据目录".to_string())?;
    let fallback = Path::new(&data_dir).join("controlmap/default.json");
    if fallback.is_file() {
        Ok(fallback)
    } else {
        Err("还没生成控制布局文件，先进一次游戏".to_string())
    }
}

fn read_layout() -> Result<(PathBuf, serde_json::Value), String> {
    let path = layout_path()?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读取布局失败：{e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("布局不是合法 JSON：{e}"))?;
    Ok((path, value))
}

/// 列出布局里的按钮（顺序与游戏内一致）。
#[tauri::command]
pub async fn get_control_buttons() -> Result<Vec<ControlButtonInfo>, String> {
    let (_, value) = read_layout()?;
    Ok(collect_buttons(&value))
}

/// 改一个按钮的透传开关，返回改完后的完整列表。
#[tauri::command]
pub async fn set_control_button_passthru(
    index: usize,
    enabled: bool,
) -> Result<Vec<ControlButtonInfo>, String> {
    let (path, mut value) = read_layout()?;
    let list = value
        .get_mut("mControlDataList")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| "布局里没有按钮列表".to_string())?;
    let entry = list
        .get_mut(index)
        .ok_or_else(|| format!("按钮下标 {index} 不存在"))?;
    let obj = entry
        .as_object_mut()
        .ok_or_else(|| "按钮数据格式不对".to_string())?;
    obj.insert(
        "passThruEnabled".to_string(),
        serde_json::Value::Bool(enabled),
    );

    let text = serde_json::to_string(&value).map_err(|e| format!("写回失败：{e}"))?;
    std::fs::write(&path, text).map_err(|e| format!("保存布局失败：{e}"))?;
    Ok(collect_buttons(&value))
}

fn collect_buttons(value: &serde_json::Value) -> Vec<ControlButtonInfo> {
    value
        .get("mControlDataList")
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .enumerate()
                .map(|(index, item)| ControlButtonInfo {
                    index,
                    name: item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("未命名")
                        .to_string(),
                    pass_thru: item
                        .get("passThruEnabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}
