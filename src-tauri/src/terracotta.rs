//! 陶瓦联机（Terracotta）隧道客户端 —— **只走 HTTP，不链接那个 .so**。
//!
//! ## 为什么要分进程 + HTTP
//!
//! Terracotta 是 AGPL-3.0-or-later，但它 README 里给了例外：只要「打包未修改的二进制
//! 且不静态/动态链接」，或「通过 IPC 与未修改的程序交互并在界面署名」，就不被 AGPL 涵盖。
//! 我们走第二条（也正是桌面版 `Terracotta.exe` + localhost HTTP 的做法）：
//! `.so` 只在 `:tunnel` 独立进程里加载（见 `TerracottaTunnelService.kt`），
//! 主进程**只发 HTTP 请求**，因此启动器保持 GPL-3.0。署名见「设置 → 关于 → 第三方声明」。
//!
//! 端口由 `:tunnel` 进程写进 `filesDir/tunnel_port`（同一应用的数据目录跨进程共享），
//! 主进程读文件即可，无需新增 JNI 调用。

use std::path::PathBuf;

/// 隧道 HTTP 端口文件名（与 Kotlin 侧约定一致）
const PORT_FILE: &str = "tunnel_port";

/// 读隧道端口；服务没起来时返回 None
fn tunnel_port() -> Option<u16> {
    let dir = crate::settings::data_dir_sync()?;
    let p = PathBuf::from(dir).join(PORT_FILE);
    let s = std::fs::read_to_string(p).ok()?;
    s.trim().parse::<u16>().ok()
}

/// 隧道是否已就绪（HTTP 服务在监听）
#[tauri::command]
pub async fn terracotta_ping() -> Result<bool, String> {
    let Some(port) = tunnel_port() else {
        return Ok(false);
    };
    match get_text(&format!("http://127.0.0.1:{port}/ping")).await {
        Ok(body) => Ok(body.contains("\"ready\":true")),
        Err(_) => Ok(false),
    }
}

/// 调隧道的一个接口（原样返回 JSON 字符串）。
/// `path` 形如 `/state`、`/host?player=名字`。
///
/// 注意：这里**只做转发**，不含任何 Terracotta 逻辑，也不加载它的库。
#[tauri::command]
pub async fn terracotta_request(path: String) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err("接口路径必须以 / 开头".to_string());
    }
    let port = tunnel_port().ok_or_else(|| {
        "隧道服务还没启动，请完全退出应用后重试".to_string()
    })?;
    let url = format!("http://127.0.0.1:{port}{path}");
    // 全部按 POST 处理：Terracotta 侧的接口都是「动作」，GET 只用于 /state、/ping、/logs，
    // 而这些用 POST 调也没问题，省得分支。
    post_text(&url).await
}

async fn get_text(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    resp.text().await.map_err(|e| e.to_string())
}

async fn post_text(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    resp.text().await.map_err(|e| e.to_string())
}
