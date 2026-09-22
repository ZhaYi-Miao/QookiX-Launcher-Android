//! 游玩时长统计：实例 `total_play_time` 累计总时长，`playtime.json`
//! 按天（东八区）累计每日时长，供统计页渲染总览与近 30 天曲线。

use crate::models::{PlaytimeByDay, PlaytimeByInstance, PlaytimeStats};
use crate::settings::get_data_dir;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

/// 记录新增的游玩秒数（调用方已按实际游戏会话时长折算）。
pub async fn add_daily_play_time(secs: u64) {
    if secs == 0 {
        return;
    }
    let day = today_day();
    let Ok(data_dir) = get_data_dir().await else {
        return;
    };
    let path = Path::new(&data_dir).join("playtime.json");
    let mut map: HashMap<String, u64> = fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let e = map.entry(day.to_string()).or_insert(0);
    *e = e.saturating_add(secs);
    if let Ok(json) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(&path, json).await;
    }
}

/// 东八区按天切分：`(unix + 8h) / 86400`
fn today_day() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    (now + 8 * 3600) / 86400
}

/// 读取按天游玩时长（day 字符串 → 秒）
pub async fn daily_play_time() -> HashMap<String, u64> {
    let Ok(data_dir) = get_data_dir().await else {
        return HashMap::new();
    };
    let path = Path::new(&data_dir).join("playtime.json");
    fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 聚合统计：按实例总时长 + 最近 30 天曲线。
pub async fn playtime_stats() -> PlaytimeStats {
    let mut by_instance: Vec<PlaytimeByInstance> = Vec::new();
    if let Ok(instances) = crate::instances::list_instances().await {
        for i in instances {
            by_instance.push(PlaytimeByInstance {
                id: i.id,
                name: i.name,
                icon: i.icon,
                seconds: i.total_play_time,
                last_played: i.last_played,
            });
        }
    }
    by_instance.sort_by(|a, b| b.seconds.cmp(&a.seconds));
    let total: i64 = by_instance.iter().map(|i| i.seconds).sum();

    let daily = daily_play_time().await;
    let today = today_day();
    let by_day: Vec<PlaytimeByDay> = (0..30)
        .rev()
        .map(|offset| {
            let day = today - offset;
            let secs = daily.get(&day.to_string()).copied().unwrap_or(0) as i64;
            PlaytimeByDay { day, seconds: secs }
        })
        .collect();

    PlaytimeStats {
        total_seconds: total,
        by_instance,
        by_day,
    }
}
