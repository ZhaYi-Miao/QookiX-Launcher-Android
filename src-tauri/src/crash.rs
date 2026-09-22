//! 崩溃日志：列出实例的崩溃报告（crash-reports/*.txt）与 JVM 错误日志
//! （hs_err_pid*.log），并对文本做规则化的诊断分析。

use crate::models::{CrashCause, CrashDetail, CrashDiagnosis, CrashLogEntry};
use crate::settings::get_data_dir;
use std::path::{Path, PathBuf};
use tokio::fs;

/// 游戏非零退出时记录崩溃报告（供 launch 流程调用）。
pub async fn report_crash(instance_id: &str, exit_code: i32) -> anyhow::Result<()> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let data_dir = get_data_dir().await?;
    // 必须写进 `instances/<id>/crash-reports/` —— 也就是 list_crash_logs()
    // 实际扫描的目录（Minecraft 自己也把崩溃报告写在这里）。
    // 以前写的是 `<data>/crash-reports/`，于是「详细崩溃分析」里永远看不到
    // 启动器生成的这份退出报告。
    let crash_dir = Path::new(&data_dir)
        .join("instances")
        .join(instance_id)
        .join("crash-reports");
    fs::create_dir_all(&crash_dir).await?;

    let timestamp = chrono::Utc::now();
    // 注意命名与格式：list_crash_logs() 只收 `crash-*.txt`，崩溃分析也按纯文本读。
    // 上一轮把目录修对了，却把文件名写成了 `launcher-exit-*.json` ——
    // 结果这份报告仍然一条都列不出来（问题从「目录错」变成了「前缀/后缀错」）。
    let filename = format!("crash-launcher-{}.txt", timestamp.format("%Y-%m-%d_%H-%M-%S"));
    let crash_file = crash_dir.join(filename);

    let content = format!(
        "QookiX 启动器退出报告\n\
         ====================\n\
         时间: {}\n\
         实例: {}\n\
         退出码: {}\n\n\
         说明: 这是游戏进程退出时由启动器写入的记录。\n\
         如果这是崩溃，Minecraft 自身通常还会在同一目录下写一份 `crash-*.txt`，请一并查看。\n",
        timestamp.to_rfc3339(),
        instance_id,
        exit_code
    );
    fs::write(&crash_file, content).await?;
    Ok(())
}

/// 列出实例目录下的崩溃报告与 JVM 错误日志
pub async fn list_crash_logs(instance_id: &str) -> Result<Vec<CrashLogEntry>, String> {
    crate::fsutil::validate_id(instance_id, "实例")?;
    let data_dir = get_data_dir().await.map_err(|e| e.to_string())?;
    let inst_dir = Path::new(&data_dir).join("instances").join(instance_id);
    let mut out: Vec<CrashLogEntry> = Vec::new();

    let crash_dir = inst_dir.join("crash-reports");
    if let Ok(mut entries) = fs::read_dir(&crash_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("crash-") && name.ends_with(".txt") {
                if let Ok(meta) = fs::metadata(entry.path()).await {
                    out.push(CrashLogEntry {
                        modified: meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                        size: meta.len(),
                        filename: name,
                        kind: "crash".into(),
                    });
                }
            }
        }
    }

    if let Ok(mut entries) = fs::read_dir(&inst_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("hs_err_pid") && name.ends_with(".log") {
                if let Ok(meta) = fs::metadata(entry.path()).await {
                    out.push(CrashLogEntry {
                        modified: meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                        size: meta.len(),
                        filename: name,
                        kind: "jvm".into(),
                    });
                }
            }
        }
    }

    out.sort_by_key(|e| std::cmp::Reverse(e.modified));
    Ok(out)
}

/// 解析崩溃报告路径（crash-reports 下的 crash-*.txt 或实例根下的 hs_err_pid*.log）
async fn resolve_report_path(instance_id: &str, filename: &str) -> Result<PathBuf, String> {
    crate::fsutil::validate_id(instance_id, "实例")?;
    crate::fsutil::validate_name(filename)?;
    if !crate::util::is_safe_filename(filename) {
        return Err("非法的文件名".into());
    }
    let data_dir = get_data_dir().await.map_err(|e| e.to_string())?;
    let inst_dir = Path::new(&data_dir).join("instances").join(instance_id);
    let path = if filename.starts_with("crash-") {
        inst_dir.join("crash-reports").join(filename)
    } else {
        inst_dir.join(filename)
    };
    if !path.is_file() {
        return Err("崩溃报告文件不存在".into());
    }
    Ok(path)
}

/// 读取崩溃报告原文
pub async fn get_report_content(instance_id: &str, filename: &str) -> Result<String, String> {
    let path = resolve_report_path(instance_id, filename).await?;
    fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取崩溃报告失败: {e}"))
}

/// 对崩溃报告文本做规则化诊断
pub async fn analyze_report(instance_id: &str, filename: &str) -> Result<CrashDiagnosis, String> {
    let content = get_report_content(instance_id, filename).await?;
    let mut d = analyze_text(&content, None);
    d.crash_report = Some(filename.to_string());
    Ok(d)
}

/// 纯文本诊断引擎：正则规则集 + 堆栈关键词。
pub fn analyze_text(content: &str, exit_code: Option<i32>) -> CrashDiagnosis {
    let mut causes: Vec<CrashCause> = Vec::new();
    let mut affected_mods: Vec<String> = Vec::new();
    let mut stacktrace: Vec<String> = Vec::new();
    let mut excerpt = String::new();

    // 从报告里提取受影响模组（Mixin / Mod 名）
    let re_mod = regex::Regex::new(r"(?m)^\s*(?:--|at)\s+([\w.\-]+Mod|[\w.\-]+mod\.[\w.]+)")
        .unwrap_or_else(|_| regex::Regex::new("").unwrap());
    for cap in re_mod.captures_iter(content) {
        let m = cap[1].to_string();
        if !affected_mods.contains(&m) {
            affected_mods.push(m);
        }
    }

    // 提取堆栈帧（at 行），保留 30 条
    let re_frame = regex::Regex::new(r"(?m)^\s+at\s+(.+)")
        .unwrap_or_else(|_| regex::Regex::new("").unwrap());
    for cap in re_frame.captures_iter(content).take(30) {
        stacktrace.push(cap[1].trim().to_string());
    }

    // 提取描述段
    if let Some(desc) = content.split("\n-- Head").next() {
        let lines: Vec<&str> = desc.lines().filter(|l| !l.trim().is_empty()).collect();
        if !lines.is_empty() {
            excerpt = lines[0].trim().chars().take(300).collect();
        }
    }

    let lower = content.to_lowercase();

    let push_cause = |causes: &mut Vec<CrashCause>,
                          id: &str,
                          severity: &str,
                          title: &str,
                          reason: &str,
                          advice: &str,
                          evidence: String,
                          confidence: u32| {
        causes.push(CrashCause {
            id: id.to_string(),
            severity: severity.to_string(),
            title: title.to_string(),
            reason: reason.to_string(),
            advice: advice.to_string(),
            evidence: evidence.chars().take(240).collect(),
            confidence,
        });
    };

    // 规则集：按严重程度与置信度组织
    if lower.contains("outofmemoryerror") || lower.contains("heap space") {
        push_cause(&mut causes, 
            "oom",
            "oom",
            "内存不足 (OutOfMemoryError)",
            "JVM 堆内存耗尽，通常是分配的内存过小或模组加载了过多资源。",
            "在设置中增大最大内存，或减少同时加载的模组 / 资源包。",
            find_evidence(content, &["OutOfMemoryError", "Java heap space"]),
            95,
        );
    }
    if lower.contains("unsatisfiedlinkerror") {
        push_cause(&mut causes, 
            "jvm",
            "jvm",
            "本地库加载失败 (UnsatisfiedLinkError)",
            "无法加载某个 native 库，常见于渲染器 / 平台库与当前系统不匹配。",
            "尝试更换渲染后端，或重装游戏依赖的 native 库。",
            find_evidence(content, &["UnsatisfiedLinkError"]),
            80,
        );
    }
    if lower.contains("lwjgl") && (lower.contains("glfw") || lower.contains("display")) {
        push_cause(&mut causes, 
            "lwjgl",
            "lwjgl",
            "LWJGL 窗口初始化失败",
            "游戏窗口创建失败，通常与显卡驱动或显示环境有关。",
            "更新显卡驱动，或尝试关闭垂直同步 / 降低渲染设置。",
            find_evidence(content, &["GLFW", "LWJGL"]),
            75,
        );
    }
    if lower.contains("java.lang.noclassdeffounderror")
        || lower.contains("nosuchmethoderror")
    {
        push_cause(&mut causes, 
            "jvm",
            "jvm",
            "类加载错误",
            "缺少类或方法，常见于模组与游戏版本 / 加载器版本不匹配。",
            "检查模组是否与当前 MC 版本、加载器版本兼容，升级或移除冲突模组。",
            find_evidence(content, &["NoClassDefFoundError", "NoSuchMethodError"]),
            85,
        );
    }
    if lower.contains("unsupportedclassversionerror") {
        push_cause(&mut causes, 
            "java_ver",
            "java_ver",
            "Java 版本不匹配 (UnsupportedClassVersionError)",
            "游戏或模组需要更高版本的 Java。",
            "为实例安装更高版本的 Java 运行时。",
            find_evidence(content, &["UnsupportedClassVersionError"]),
            90,
        );
    }
    if lower.contains("opengl") || lower.contains("gl_error") || lower.contains("pixel format") {
        push_cause(&mut causes, 
            "gl",
            "gl",
            "OpenGL 错误",
            "OpenGL 上下文创建或渲染失败，常见于驱动不支持所需的 GL 版本。",
            "更新显卡驱动，尝试在设置中切换渲染器（如 Sodium/VulkanMod）。",
            find_evidence(content, &["OpenGL", "pixel format"]),
            70,
        );
    }
    if lower.contains("error loading mods") || lower.contains("missing mods") {
        push_cause(&mut causes, 
            "mod",
            "mod",
            "模组加载失败",
            "有模组缺失或无法加载。",
            "在内容管理中检查缺失模组并重新安装。",
            find_evidence(content, &["Error loading mods", "Missing mods"]),
            85,
        );
    }
    if lower.contains("fabricloader") && lower.contains("mixinextraservice") {
        push_cause(&mut causes, 
            "mod",
            "mod",
            "Mixin 初始化失败",
            "某个模组的 Mixin 注入失败，通常由模组冲突或版本不兼容引起。",
            "逐个禁用最近安装的模组排查冲突。",
            find_evidence(content, &["Mixin"]),
            80,
        );
    }

    // 未命中任何规则
    if causes.is_empty() {
        push_cause(&mut causes, 
            "unknown",
            "unknown",
            "未能定位主因",
            "未能从报告文本中识别出明确的崩溃原因。",
            "查看完整崩溃报告原文，或将报告内容搜索到社区求助。",
            content.lines().next().unwrap_or("").trim().to_string(),
            0,
        );
    }

    causes.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    let confidence = causes[0].confidence;

    // 环境信息
    let mut details: Vec<CrashDetail> = Vec::new();
    for key in [
        "Minecraft Version",
        "Java Version",
        "Operating System",
        "CPU",
        "Memory",
        "Mod Launcher",
        "Fabric Loader",
        "Forge",
    ] {
        if let Some(val) = extract_key_value(content, key) {
            details.push(CrashDetail {
                key: key.to_string(),
                value: val,
            });
        }
    }

    CrashDiagnosis {
        severity: causes[0].severity.clone(),
        title: causes[0].title.clone(),
        reason: causes[0].reason.clone(),
        advice: causes[0].advice.clone(),
        excerpt,
        exit_code,
        crash_report: None,
        affected_mods,
        causes,
        stacktrace,
        details,
        confidence,
    }
}

fn find_evidence(content: &str, needles: &[&str]) -> String {
    for needle in needles {
        if let Some(idx) = content.find(needle) {
            let start = content[..idx].rfind('\n').map(|i| i + 1).unwrap_or(idx);
            let end = content[idx..]
                .find('\n')
                .map(|i| idx + i)
                .unwrap_or(content.len());
            let line = &content[start..end];
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    String::new()
}

fn extract_key_value(content: &str, key: &str) -> Option<String> {
    let pattern = format!("{}: ", key);
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(&pattern) {
            let val = rest.trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}
