use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use anyhow::{Context, Result};
use sha1::{Sha1, Digest};
use uuid::Uuid;
use crate::models::*;

lazy_static::lazy_static! {
    static ref DOWNLOAD_TASKS: Arc<Mutex<HashMap<String, DownloadProgress>>> = Arc::new(Mutex::new(HashMap::new()));
    /// 进行中下载的取消标志。
    ///
    /// 以前 `cancel_download` 只是把进度条目从表里删掉 —— 下载本身照跑不误，
    /// 前端点了「取消」没有任何效果。现在下载循环会逐块检查这个标志。
    static ref CANCEL_FLAGS: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> = Arc::new(Mutex::new(HashMap::new()));
}

/// 进度回写的最小间隔。
///
/// 完全不回写就是「一直 0%，完成时直接跳 100%」；每块都回写则频繁抢
/// `DOWNLOAD_TASKS` 的锁。200ms 与前端刷新率匹配。
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// 单个候选源的下载失败原因。
enum StreamFail {
    /// 用户取消 —— 不再尝试下一个源，直接整体中止。
    Cancelled,
    /// 这个源不行（网络/SHA1），可以换下一个。
    Retry(anyhow::Error),
}

pub async fn download_file(
    url: &str,
    dest: &str,
    expected_sha1: Option<String>,
) -> Result<DownloadProgress> {
    download_file_with_cancel(url, dest, expected_sha1, None).await
}

/// 带「所属安装任务取消标志」的下载。
///
/// 安装流水线（如 `version::install_version`）把它自己的 `TaskCtx.cancel_flag()`
/// 传进来，用户在「下载中心」点取消时，正在传的大文件也能**立刻**停下；
/// 否则要等整个阶段跑完才响应，体验上和「没反应」一样。
pub async fn download_file_with_cancel(
    url: &str,
    dest: &str,
    expected_sha1: Option<String>,
    install_cancel: Option<Arc<AtomicBool>>,
) -> Result<DownloadProgress> {
    let task_id = format!("dl_{}", Uuid::new_v4());

    // Apply the user's mirror setting to the download URL.
    // 组装候选地址：镜像 → 官方。镜像下载失败/被中断时回退官方源，避免镜像故障导致永久卡住。
    let settings = crate::settings::get_settings().await?;
    let mirror_base = crate::mirror::resolve_from(
        &settings.mirror,
        settings.mirror_custom.as_deref().unwrap_or(""),
    );
    let mut urls: Vec<String> = Vec::new();
    if !mirror_base.is_empty() {
        let mapped = crate::mirror::map(&mirror_base, url);
        if mapped != url {
            urls.push(mapped);
        }
    }
    urls.push(url.to_string());

    // Check if file already exists and matches SHA1
    if let Some(expected) = &expected_sha1 {
        if let Ok(content) = fs::read(dest).await {
            let mut hasher = Sha1::new();
            hasher.update(&content);
            let actual_sha1 = hasher.finalize();
            if format!("{:x}", actual_sha1) == *expected {
                return Ok(DownloadProgress {
                    task_id: task_id.clone(),
                    progress: 100.0,
                    message: "Already downloaded".to_string(),
                    total_bytes: content.len() as i64,
                    downloaded_bytes: content.len() as i64,
                });
            }
        }
    }

    // Create destination directory
    if let Some(parent) = Path::new(dest).parent() {
        fs::create_dir_all(parent).await
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // 注册本次下载的取消标志
    let cancel = {
        let mut flags = CANCEL_FLAGS.lock().await;
        let flag = Arc::new(AtomicBool::new(false));
        flags.insert(task_id.clone(), flag.clone());
        flag
    };

    let client = crate::util::http_client().await;
    let dest_path = Path::new(dest).to_path_buf();
    let mut last_err: Option<anyhow::Error> = None;

    // 逐候选源下载：单个源失败（网络中断/超时/SHA1 不符）时尝试下一个
    for mapped_url in &urls {
        match stream_to_file(
            &client,
            mapped_url,
            &dest_path,
            expected_sha1.as_deref(),
            &task_id,
            &cancel,
            install_cancel.as_ref(),
        )
        .await
        {
            Ok(task) => {
                CANCEL_FLAGS.lock().await.remove(&task_id);
                DOWNLOAD_TASKS
                    .lock()
                    .await
                    .insert(task_id.clone(), task.clone());
                return Ok(task);
            }
            Err(StreamFail::Cancelled) => {
                CANCEL_FLAGS.lock().await.remove(&task_id);
                // 半截文件必须删掉：留着会被后续的 SHA1 校验/「已存在就跳过」误判成完整文件
                let _ = fs::remove_file(&dest_path).await;
                let by_install = install_cancel
                    .as_ref()
                    .is_some_and(|f| f.load(Ordering::Relaxed));
                return Err(anyhow::anyhow!(if by_install {
                    "任务已取消"
                } else {
                    "下载已取消"
                }));
            }
            Err(StreamFail::Retry(e)) => {
                last_err = Some(e);
                continue;
            }
        }
    }

    CANCEL_FLAGS.lock().await.remove(&task_id);
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Failed to download: {}", url)))
}

/// 下载**单个**源：边下边写边算 SHA1，并节流回写进度。
///
/// 三个修复（对比旧实现）：
///   1. 旧实现 `response.bytes()` 把整个响应读进内存 —— `assets` 里单个文件可以几百 MB，
///      手机上直接 OOM；现在按块 `chunk()` 流式落盘。
///   2. 旧实现校验 SHA1 时又 `fs::read` 把整个文件读**第二遍**；现在哈希在写的同时累计。
///   3. 旧实现只在**完成时**写一次进度（前端永远是 0% → 100%）；现在每 200ms 回写一次。
async fn stream_to_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    task_id: &str,
    cancel: &Arc<AtomicBool>,
    install_cancel: Option<&Arc<AtomicBool>>,
) -> std::result::Result<DownloadProgress, StreamFail> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| StreamFail::Retry(anyhow::anyhow!("Failed to download {url}: {e}")))?;

    let total_size = response.content_length().unwrap_or(0) as i64;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| StreamFail::Retry(anyhow::anyhow!("Failed to create directory: {e}")))?;
    }
    let mut file = fs::File::create(dest)
        .await
        .map_err(|e| StreamFail::Retry(anyhow::anyhow!("Failed to create {}: {e}", dest.display())))?;

    let mut hasher = Sha1::new();
    let mut downloaded: i64 = 0;
    let mut last_report = std::time::Instant::now();

    loop {
        // 两个取消来源都要看：本次下载自己的标志（`cancel_download`），
        // 以及所属安装任务的标志（前端「下载中心」的取消按钮）。
        if cancel.load(Ordering::Relaxed)
            || install_cancel.is_some_and(|f| f.load(Ordering::Relaxed))
        {
            return Err(StreamFail::Cancelled);
        }
        let chunk = match response.chunk().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => {
                return Err(StreamFail::Retry(anyhow::anyhow!(
                    "Download interrupted for {url}: {e}"
                )))
            }
        };
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| StreamFail::Retry(anyhow::anyhow!("Failed to write file: {e}")))?;
        downloaded += chunk.len() as i64;

        if last_report.elapsed() >= PROGRESS_INTERVAL {
            last_report = std::time::Instant::now();
            write_progress(task_id, total_size, downloaded).await;
        }
    }

    file.flush()
        .await
        .map_err(|e| StreamFail::Retry(anyhow::anyhow!("Failed to flush file: {e}")))?;
    drop(file);

    if let Some(expected) = expected_sha1 {
        let actual = format!("{:x}", hasher.finalize());
        if actual != expected {
            let _ = fs::remove_file(dest).await;
            return Err(StreamFail::Retry(anyhow::anyhow!(
                "SHA1 mismatch from {url}: expected {expected}, got {actual}"
            )));
        }
    }

    let progress = if total_size > 0 {
        (downloaded as f32 / total_size as f32) * 100.0
    } else {
        100.0
    };
    let task = DownloadProgress {
        task_id: task_id.to_string(),
        progress,
        message: format!("Downloaded {downloaded} bytes"),
        total_bytes: total_size,
        downloaded_bytes: downloaded,
    };
    write_progress_full(task_id, task.clone()).await;
    Ok(task)
}

async fn write_progress(task_id: &str, total: i64, done: i64) {
    let progress = if total > 0 {
        (done as f32 / total as f32) * 100.0
    } else {
        0.0
    };
    write_progress_full(
        task_id,
        DownloadProgress {
            task_id: task_id.to_string(),
            progress,
            message: format!("Downloaded {done} bytes"),
            total_bytes: total,
            downloaded_bytes: done,
        },
    )
    .await;
}

async fn write_progress_full(task_id: &str, task: DownloadProgress) {
    DOWNLOAD_TASKS
        .lock()
        .await
        .insert(task_id.to_string(), task);
}

/// 取消下载：把标志置 true，下载循环下一块就会停下并删掉半截文件。
pub async fn cancel_download(task_id: &str) -> Result<()> {
    let flag = CANCEL_FLAGS.lock().await.get(task_id).cloned();
    match flag {
        Some(f) => {
            f.store(true, Ordering::Relaxed);
            Ok(())
        }
        // 找不到就说明已经结束（或 id 不对）——告诉调用方，别让前端静默以为取消了
        None => Err(anyhow::anyhow!("没有进行中的下载任务: {task_id}")),
    }
}

pub async fn get_progress(task_id: &str) -> Result<DownloadProgress> {
    let tasks = DOWNLOAD_TASKS.lock().await;
    tasks.get(task_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))
}

pub async fn download_libraries(
    libraries: &[crate::version::Library],
    data_dir: &str
) -> Result<()> {
    let libraries_dir = Path::new(data_dir).join("libraries");

    for lib in libraries {
        if let Some(downloads) = &lib.downloads {
            if let Some(artifact) = &downloads.artifact {
                let lib_path = libraries_dir.join(&artifact.path);
                
                if let Some(parent) = lib_path.parent() {
                    fs::create_dir_all(parent).await.ok();
                }

                // Skip if already exists
                if lib_path.exists() {
                    continue;
                }

                download_file(
                    &artifact.url,
                    &lib_path.to_string_lossy(),
                    Some(artifact.sha1.clone())
                ).await
                .with_context(|| format!("Failed to download library: {}", lib.name))?;
            }
        }
    }

    Ok(())
}
