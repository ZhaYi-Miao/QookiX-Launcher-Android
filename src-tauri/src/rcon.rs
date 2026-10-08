//! Minecraft RCON 客户端 —— 用来**优雅停服**和执行控制台命令。
//!
//! # 为什么必须是 RCON，不能靠杀进程
//!
//! `System.exit(0)`（`JvmLauncher::shutdown` 那条路）能让 JVM 退出，但实测
//! 服务端日志里**没有** `Stopping the server / Saving worlds` —— 世界数据
//! 可能没存盘，玩家建筑就会回档。
//!
//! RCON 是官方控制台通道：`stop` 命令会让服务端自己走正常的关闭流程
//! （存盘 → 关世界 → 关监听 → 退出）。这是唯一能保证数据安全的停服方式。
//!
//! # 协议（很简短，共 3 种包）
//!
//! ```text
//! 包 = [int32 LE 长度][int32 LE requestId][int32 LE type][body 以 \0\0 结尾]
//! type: 3 = 鉴权, 2 = 命令, 0 = 响应值, 2 = 鉴权结果
//! ```
//!
//! 注意「长度」字段算的是 **id+type+body** 的字节数，不含长度字段本身。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

/// RCON 凭据（密码存在 `files/servers/{id}/rcon.json`，不进版本库）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RconCreds {
    pub port: u16,
    pub password: String,
}

const TYPE_AUTH: i32 = 3;
const TYPE_CMD: i32 = 2;
const TYPE_AUTH_RESPONSE: i32 = 2;

/// 读凭据；没有就生成一个并落盘
pub fn load_or_create_creds(dir: &Path, game_port: u16) -> Result<RconCreds, String> {
    let path = dir.join("rcon.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(c) = serde_json::from_str::<RconCreds>(&text) {
            return Ok(c);
        }
    }
    // RCON 端口不能和游戏端口相同（否则两个监听撞在一起）
    let port = if game_port == 25575 { 25576 } else { 25575 };
    let creds = RconCreds {
        port,
        password: random_password(),
    };
    let _ = std::fs::write(&path, serde_json::to_string_pretty(&creds).map_err(|e| e.to_string())?);
    Ok(creds)
}

/// 12 位随机密码（只用便于手输的字符集，便于用户查看/修改）
fn random_password() -> String {
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut s = String::new();
    for i in 0..12 {
        if i == 6 {
            s.push('-');
        }
        // 简单混合，避免首字节为 0
        let n = (std::process::id() as usize)
            .wrapping_mul(31)
            .wrapping_add(i * 7)
            .wrapping_add(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as usize)
                .unwrap_or(0));
        s.push(CHARS[n % CHARS.len()] as char);
    }
    s
}

fn write_packet(s: &mut TcpStream, id: i32, ty: i32, body: &str) -> std::io::Result<()> {
    let mut payload: Vec<u8> = Vec::with_capacity(body.len() + 12);
    payload.extend_from_slice(&id.to_le_bytes());
    payload.extend_from_slice(&ty.to_le_bytes());
    payload.extend_from_slice(body.as_bytes());
    payload.push(0);
    payload.push(0);
    let mut head = (payload.len() as i32).to_le_bytes().to_vec();
    head.extend_from_slice(&payload);
    s.write_all(&head)?;
    s.flush()
}

/// 读一个包，返回 (id, type, body)
fn read_packet(s: &mut TcpStream) -> Result<(i32, i32, String), String> {
    let mut len_buf = [0u8; 4];
    s.read_exact(&mut len_buf).map_err(|e| format!("读长度失败: {e}"))?;
    let len = i32::from_le_bytes(len_buf);
    if len < 10 || len > 4 * 1024 * 1024 {
        return Err(format!("RCON 包长度异常: {len}"));
    }
    let mut buf = vec![0u8; len as usize];
    s.read_exact(&mut buf).map_err(|e| format!("读包体失败: {e}"))?;
    let id = i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let ty = i32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let mut body = String::from_utf8_lossy(&buf[8..]).to_string();
    while body.ends_with('\0') {
        body.pop();
    }
    Ok((id, ty, body))
}

/// 鉴权 + 发送一条命令，返回服务端输出
pub fn exec(port: u16, password: &str, command: &str) -> Result<String, String> {
    let addr = format!("127.0.0.1:{port}");
    // 服务端刚起时 RCON 监听可能慢，超时给足 5 秒
    let mut s = TcpStream::connect(&addr)
        .map_err(|e| format!("连不上 RCON（{addr}）：{e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;

    // 1) 鉴权
    write_packet(&mut s, 1, TYPE_AUTH, password).map_err(|e| format!("鉴权发送失败: {e}"))?;
    let (id, ty, _body) = read_packet(&mut s)?;
    if ty != TYPE_AUTH_RESPONSE {
        return Err(format!("鉴权响应类型异常: type={ty}"));
    }
    if id == -1 {
        return Err("RCON 密码不正确（服务端拒绝）".to_string());
    }

    // 2) 执行命令
    write_packet(&mut s, 2, TYPE_CMD, command).map_err(|e| format!("命令发送失败: {e}"))?;
    let (_id2, _ty2, out) = read_packet(&mut s)?;
    Ok(out)
}

/**
 * 优雅停服：发 `stop` 并等 JVM 真正退出。
 *
 * 流程是「RCON stop → 轮询 IPC 等待 → 超时才退回 System.exit」：
 * RCON 让服务端自己存盘关世界（安全），而 Android 随时可能把进程杀掉
 * （lowmemorykiller），所以等待窗口有限（默认 20 秒）。
 */
pub fn graceful_stop(
    ipc_port: Option<u16>,
    token: &str,
    rcon: &RconCreds,
    wait_secs: u64,
) -> Result<String, String> {
    // 先试 RCON
    let rcon_res = exec(rcon.port, &rcon.password, "stop");
    match &rcon_res {
        Ok(_) => {}
        Err(e) => {
            // RCON 不可用不算失败：可能用户关掉了 enable-rcon，或服务端还没起完。
            // 这种情况退回 System.exit，但要如实告诉调用方「这次不是优雅停服」。
            tracing::warn!("[server] RCON 停服不可用（{e}），退回 System.exit");
            return Err(format!("RCON 不可用：{e}"));
        }
    }

    // 等 JVM 退出。
    //
    // **没有可用的 IPC 端口，就不能说「已经退出了」**。调用方原来用
    // `read_runtime_io(...).unwrap_or((0, ""))` 兜底成端口 0，而 `ipc_alive(0, ..)`
    // 必然立刻返回 false —— 于是「RCON stop 刚发出去」就直接被判定成
    // 「已优雅停服（世界已存盘）」。如果 runtime.json 丢了或损坏（进程被系统杀过就会），
    // 服务器其实可能还在跑，用户却以为已经安全关闭了。
    // 这两种情况都如实降级措辞，让界面/用户自己去确认。
    let Some(port) = ipc_port.filter(|p| *p != 0) else {
        return Ok("已发送 stop，但缺少运行记录，无法确认 JVM 是否已退出".to_string());
    };
    if token.is_empty() {
        return Ok("已发送 stop，但缺少校验 token，无法确认 JVM 是否已退出".to_string());
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(wait_secs);
    while std::time::Instant::now() < deadline {
        if !crate::servers::ipc_alive(port, token) {
            return Ok("已优雅停服（RCON stop，世界已存盘）".to_string());
        }
        std::thread::sleep(Duration::from_millis(700));
    }
    Err(format!("RCON 已发 stop，但 {wait_secs} 秒内 JVM 仍未退出"))
}
