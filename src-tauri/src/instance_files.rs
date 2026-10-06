//! 实例文件完整性检查（支撑 UI 的「补全游戏文件」功能）。
//!
//! # 背景
//!
//! 遇到过两个真实问题：
//! 1. 用户的 26.3 NeoForge 实例只有 `instance.json` + `options.txt`，
//!    界面显示「这里还是空的」、下载页没任务、启动直接黑屏 —— **完全没有补救入口**。
//! 2. `loader_version` 为空时（该游戏版本没有对应版本的加载器），
//!    依赖根本解析不出来，但界面不会告诉用户「是这个原因」。
//!
//! 所以这里提供一个**纯诊断**命令：不下载任何东西，只说清楚
//! 「缺什么、为什么缺、能不能补」。

use serde_json::Value;

/// 检查结果里的一项
#[derive(Debug, Clone, serde::Serialize)]
pub struct MissingItem {
    /// 机器可读的类别：version / libraries / assets / loader / runtime
    pub kind: String,
    /// 人类可读的中文说明
    pub detail: String,
    /// 是否能通过「补全游戏文件」修好
    pub fixable: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstanceFileReport {
    /// 5 个关键项里完整了几个（0-5）
    pub ok: usize,
    pub total: usize,
    pub missing: Vec<MissingItem>,
    /// 缺文件时给用户的一句人话建议
    pub advice: String,
    /// 实例是否具备启动条件（关键项全齐）
    pub can_launch: bool,
}

async fn data_dir() -> Result<std::path::PathBuf, String> {
    let d = crate::settings::get_data_dir()
        .await
        .map_err(|e| e.to_string())?;
    Ok(std::path::PathBuf::from(d))
}

/// 判断某个游戏版本的 json 是否已下载
fn version_json_present(dir: &std::path::Path, mc_version: &str) -> bool {
    dir.join("versions")
        .join(mc_version)
        .join("version.json")
        .exists()
        || dir
            .join("versions")
            .join(mc_version)
            .join(format!("{mc_version}.json"))
            .exists()
}

/// 数一个目录下的文件数（用于判断 assets/libraries 是否下过）
fn count_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().count())
        .unwrap_or(0)
}

/// 检查一个实例缺哪些关键文件。
///
/// **只读，不下载** —— UI 用它决定要不要显示「补全游戏文件」按钮，
/// 以及告诉用户到底缺什么（避免「点了没反应」或「下完还是黑屏」）。
#[tauri::command]
pub async fn check_instance_files(instance_id: String) -> Result<InstanceFileReport, String> {
    crate::fsutil::validate_id(&instance_id, "实例")?;
    let dir = data_dir().await?;
    let inst_dir = crate::settings::instance_dir(&instance_id)
        .await
        .map_err(|e| e.to_string())?;
    let json_path = inst_dir.join("instance.json");
    if !json_path.exists() {
        return Err("实例不存在".to_string());
    }
    let text = std::fs::read_to_string(&json_path).map_err(|e| e.to_string())?;
    let inst: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mc_version = inst["mc_version"]
        .as_str()
        .or_else(|| inst["mcVersion"].as_str())
        .unwrap_or("")
        .to_string();
    let loader = inst["loader"]
        .as_str()
        .unwrap_or("vanilla")
        .to_lowercase();
    let loader_version = inst["loader_version"]
        .as_str()
        .or_else(|| inst["loaderVersion"].as_str())
        .unwrap_or("")
        .to_string();

    let mut missing: Vec<MissingItem> = Vec::new();

    // 1) 版本 json
    if !version_json_present(&dir, &mc_version) {
        missing.push(MissingItem {
            kind: "version".into(),
            detail: format!("缺少 {mc_version} 的版本信息（version.json）"),
            fixable: true,
        });
    }

    // 2) libraries
    if count_files(&dir.join("libraries")) == 0 {
        missing.push(MissingItem {
            kind: "libraries".into(),
            detail: "缺少游戏依赖库（libraries）".into(),
            fixable: true,
        });
    }

    // 3) assets（游戏本体资源，占绝大部分体积）
    if count_files(&dir.join("assets")) <= 1 {
        missing.push(MissingItem {
            kind: "assets".into(),
            detail: "缺少游戏素材（assets）".into(),
            fixable: true,
        });
    }

    // 4) 加载器本体
    if loader != "vanilla" {
        if loader_version.is_empty() {
            // 这是「创建时就选不到版本」的情况，补全也救不回来 —— 单独提示
            let pretty = match loader.as_str() {
                "neoforge" => "NeoForge",
                "forge" => "Forge",
                "fabric" => "Fabric",
                other => other,
            };
            missing.push(MissingItem {
                kind: "loader".into(),
                detail: format!(
                    "{pretty} 版本号为空：{mc_version} 可能还没有对应的 {pretty} 版本，\
                     建议换一个游戏版本或先装原版"
                ),
                fixable: false,
            });
        } else {
            // 必须和 `browse::install_loader` 用同一份路径规则：
            // 早先这里查的是 `jars/<loader>-<版本>.jar`，而安装落盘在
            // `libraries/`，于是装完仍被判成「缺本体」。
            let jar = crate::browse::loader_jar_path(
                &dir.join("libraries"),
                &loader,
                &mc_version,
                &loader_version,
            );
            if !jar.exists() {
                missing.push(MissingItem {
                    kind: "loader".into(),
                    detail: format!("缺少 {loader} {loader_version} 本体"),
                    fixable: true,
                });
            }
        }
    }

    // 5) Java 运行时
    let has_jre = std::fs::read_dir(dir.join("runtimes"))
        .map(|rd| rd.flatten().next().is_some())
        .unwrap_or(false);
    if !has_jre {
        missing.push(MissingItem {
            kind: "runtime".into(),
            detail: "缺少 Java 运行环境".into(),
            fixable: true,
        });
    }

    let total = 5;
    let ok = total - missing.len();
    // 不可修的项（loader 版本号为空）会直接导致启动失败，必须在 advice 里点出来
    let has_blocker = missing.iter().any(|m| !m.fixable);
    let advice = if missing.is_empty() {
        "文件完整，可以直接启动".to_string()
    } else if has_blocker {
        let blocked: Vec<String> = missing
            .iter()
            .filter(|m| !m.fixable)
            .map(|m| m.detail.clone())
            .collect();
        format!(
            "{}。这个问题「补全文件」解决不了，需要换一个游戏版本或加载器。",
            blocked.join("；")
        )
    } else if ok == 0 {
        "实例几乎是空的，点「补全游戏文件」重新下载一遍即可".to_string()
    } else {
        format!("缺 {} 项，点「补全游戏文件」补齐即可", missing.len())
    };

    Ok(InstanceFileReport {
        ok,
        total,
        can_launch: missing.is_empty(),
        missing,
        advice,
    })
}

/// 补全（下载）缺失文件。**直接复用现成的 `browse::install_game`** ——
/// 它就是「下载游戏本体」的完整实现（version.json / libraries / assets / loader），
/// 不另写一套下载逻辑。
#[tauri::command]
pub async fn repair_instance_files(instance_id: String) -> Result<Value, String> {
    crate::fsutil::validate_id(&instance_id, "实例")?;
    crate::browse::install_game(&instance_id)
        .await
        .map_err(|e| e.to_string())
}
