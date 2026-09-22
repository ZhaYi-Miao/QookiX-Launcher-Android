//! Minecraft Java 版服务器实时状态查询（Server List Ping 协议）。
//! 纯 TCP + JSON，不依赖外部 API。

use crate::models::ServerStatus;
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// DNS-over-HTTPS 兜底端点（用 IP 直连，因此解析它们自身不需要系统 DNS）。
/// 代理环境（Clash / v2ray 的 TUN 模式）下原生 `getaddrinfo` 经常直接失败，
/// 此时改用走 HTTP 代理的 DoH 查询拿到真实 IP 再连。
const DOH_ENDPOINTS: [&str; 3] = [
    "https://223.5.5.5/resolve",
    "https://1.12.12.12/resolve",
    "https://doh.pub/dns-query",
];

/// 解析主机名为可连接的地址列表：优先系统解析，失败回退 DoH。
async fn resolve_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    if let Ok(iter) = tokio::net::lookup_host((host, port)).await {
        let list: Vec<SocketAddr> = iter.collect();
        if !list.is_empty() {
            return Ok(list);
        }
    }
    let ips = resolve_via_doh(host).await?;
    if ips.is_empty() {
        return Err("域名解析失败".into());
    }
    Ok(ips.into_iter().map(|ip| SocketAddr::new(ip, port)).collect())
}

/// 通过 SOCKS5 代理建立到 `host:port` 的连接。
///
/// `proxy` 形如 `socks5://127.0.0.1:1080`；为兼容 Clash 的 mixed 端口，
/// `http://127.0.0.1:7890` 这类地址也允许尝试（握手失败会由调用方回退直连）。
async fn connect_via_socks5(proxy: &str, host: &str, port: u16) -> Result<TcpStream, String> {
    let rest = proxy
        .trim()
        .trim_start_matches("socks5h://")
        .trim_start_matches("socks5://")
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let rest = rest.trim_end_matches('/');
    let (phost, pport) = match rest.rfind(':') {
        Some(i) => (
            rest[..i].to_string(),
            rest[i + 1..].parse::<u16>().map_err(|_| "代理端口无效".to_string())?,
        ),
        None => (rest.to_string(), 1080u16),
    };
    if phost.is_empty() {
        return Err("代理地址无效".into());
    }

    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect((phost.as_str(), pport)),
    )
    .await
    .map_err(|_| "连接代理超时".to_string())?
    .map_err(|e| format!("连接代理失败: {e}"))?;
    let _ = stream.set_nodelay(true);

    // 1) 方法协商：只支持「无认证」
    stream
        .write_all(&[0x05u8, 0x01, 0x00])
        .await
        .map_err(|e| format!("代理握手失败: {e}"))?;
    let mut resp = [0u8; 2];
    read_exact_timeout(&mut stream, &mut resp, Duration::from_secs(5)).await?;
    if resp[0] != 0x05 || resp[1] != 0x00 {
        return Err("代理不支持免认证 SOCKS5".into());
    }

    // 2) CONNECT 请求：域名为 0x03，IPv4 为 0x01
    let mut req = vec![0x05u8, 0x01, 0x00];
    match host.parse::<Ipv4Addr>() {
        Ok(ip) => {
            req.push(0x01);
            req.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            if host.len() > 255 {
                return Err("域名过长".into());
            }
            req.push(0x03);
            req.push(host.len() as u8);
            req.extend_from_slice(host.as_bytes());
        }
    }
    req.extend_from_slice(&port.to_be_bytes());
    stream
        .write_all(&req)
        .await
        .map_err(|e| format!("代理请求失败: {e}"))?;

    // 3) 应答：VER REP RSV ATYP [BND.ADDR] BND.PORT
    let mut head = [0u8; 4];
    read_exact_timeout(&mut stream, &mut head, Duration::from_secs(5)).await?;
    if head[0] != 0x05 || head[1] != 0x00 {
        return Err(format!("代理拒绝连接 (0x{:02x})", head[1]));
    }
    let skip = match head[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            read_exact_timeout(&mut stream, &mut len, Duration::from_secs(5)).await?;
            len[0] as usize
        }
        _ => return Err("代理应答格式异常".into()),
    };
    let mut tail = vec![0u8; skip + 2];
    read_exact_timeout(&mut stream, &mut tail, Duration::from_secs(5)).await?;

    Ok(stream)
}

async fn resolve_via_doh(host: &str) -> Result<Vec<IpAddr>, String> {
    let client = crate::util::http_client().await;
    for base in DOH_ENDPOINTS {
        let url = format!("{base}?name={}&type=A", urlencoding::encode(host));
        let resp = match client
            .get(&url)
            .header("accept", "application/dns-json")
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !resp.status().is_success() {
            continue;
        }
        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mut ips = Vec::new();
        if let Some(answers) = body.get("Answer").and_then(|a| a.as_array()) {
            for a in answers {
                if a.get("type").and_then(|t| t.as_i64()) != Some(1) {
                    continue;
                }
                if let Some(data) = a.get("data").and_then(|d| d.as_str()) {
                    if let Ok(ip) = data.parse::<Ipv4Addr>() {
                        ips.push(IpAddr::V4(ip));
                    }
                }
            }
        }
        if !ips.is_empty() {
            return Ok(ips);
        }
    }
    Err("域名解析失败（系统解析与 DoH 均不可用）".into())
}

/// 入口：统一返回 ServerStatus，网络/解析失败也会返回一个 offline 状态，绝不抛错
pub async fn ping_server(raw_addr: &str) -> ServerStatus {
    let addr = raw_addr.trim().to_string();
    match do_ping(&addr).await {
        Ok(s) => s,
        Err(e) => ServerStatus {
            online: false,
            address: addr,
            name: None,
            version: None,
            players_online: None,
            players_max: None,
            motd: None,
            favicon: None,
            latency_ms: None,
            error: Some(e),
        },
    }
}

/// 解析 "host"、"host:port" 或 "[ipv6]:port" 为 (host, port)
fn parse_addr(addr: &str) -> Result<(String, u16), String> {
    let addr = addr.trim();
    if addr.starts_with('[') {
        let end = addr.find(']').ok_or("无效的 IPv6 地址")?;
        let host = addr[1..end].to_string();
        let rest = &addr[end + 1..];
        let port = if let Some(stripped) = rest.strip_prefix(':') {
            stripped.parse::<u16>().map_err(|_| "端口无效".to_string())?
        } else {
            25565
        };
        return Ok((host, port));
    }
    if let Some(idx) = addr.rfind(':') {
        let (h, p) = addr.split_at(idx);
        if h.is_empty() {
            return Err("地址无效".into());
        }
        let port = p[1..].parse::<u16>().map_err(|_| "端口无效".to_string())?;
        Ok((h.to_string(), port))
    } else {
        Ok((addr.to_string(), 25565))
    }
}

async fn do_ping(addr: &str) -> Result<ServerStatus, String> {
    let (host, port) = parse_addr(addr)?;

    let mut stream: Option<TcpStream> = None;
    let mut last_err = String::from("连接失败");

    // 配了代理时优先走 SOCKS5：TUN/代理环境下到境外服务器的直连通常被阻断，
    // 只有借道代理才能测出真实延迟。Clash 这类客户端的 mixed 端口同时支持
    // HTTP 与 SOCKS5，因此 `http://127.0.0.1:7890` 也能按 SOCKS5 试一次。
    let proxy = crate::util::resolved_proxy().await;
    if !proxy.is_empty() {
        match connect_via_socks5(&proxy, &host, port).await {
            Ok(s) => stream = Some(s),
            Err(e) => last_err = e,
        }
    }

    if stream.is_none() {
        let candidates = resolve_addrs(&host, port).await?;
        for sa in candidates {
            match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(sa)).await {
                Ok(Ok(s)) => {
                    stream = Some(s);
                    break;
                }
                Ok(Err(e)) => last_err = format!("连接失败: {e}"),
                Err(_) => last_err = "连接超时".into(),
            }
        }
    }
    let stream = stream.ok_or(last_err)?;
    let _ = stream.set_nodelay(true);
    let (mut reader, mut writer) = stream.into_split();

    // 1) Handshake: 协议版本(VarInt) + 地址(VarInt len + utf8) + 端口(u16) + nextState(1)
    let mut payload = Vec::new();
    write_varint(&mut payload, 767);
    write_varint(&mut payload, host.len() as i32);
    payload.extend_from_slice(host.as_bytes());
    payload.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut payload, 1); // next state = status

    let mut packet = Vec::new();
    packet.push(0x00); // packet id
    packet.extend_from_slice(&payload);
    let mut full = Vec::new();
    write_varint(&mut full, packet.len() as i32);
    full.extend_from_slice(&packet);
    write_all_timeout(&mut writer, &full, Duration::from_secs(5)).await?;

    // 2) Status Request: 仅 packet id 0x00
    let mut req = Vec::new();
    write_varint(&mut req, 1);
    req.push(0x00);
    write_all_timeout(&mut writer, &req, Duration::from_secs(5)).await?;

    // 3) 读取 Status Response
    let t0 = Instant::now();
    let len = read_varint_timeout(&mut reader, Duration::from_secs(6)).await?;
    if len <= 0 || len > 10_000_000 {
        return Err("状态响应过大或无效".into());
    }
    let mut buf = vec![0u8; len as usize];
    read_exact_timeout(&mut reader, &mut buf, Duration::from_secs(6)).await?;
    let elapsed = t0.elapsed();
    let mut pos = 1usize;
    let json_len = read_varint_at(&buf, &mut pos)? as usize;
    if pos + json_len > buf.len() {
        return Err("状态响应格式错误".into());
    }
    let json = String::from_utf8_lossy(&buf[pos..pos + json_len]).to_string();
    let v: Value = serde_json::from_str(&json).map_err(|e| format!("JSON 解析失败: {e}"))?;

    // 4) Ping / Pong 用于测量更精确的延迟
    let latency_ms: Option<i64> = match send_ping_pong(&mut reader, &mut writer, Duration::from_secs(5)).await {
        Ok(rtt) => Some(rtt),
        Err(_) => Some(elapsed.as_millis() as i64),
    };

    let version = v
        .get("version")
        .and_then(|x| x.get("name"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let players_online = v
        .get("players")
        .and_then(|x| x.get("online"))
        .and_then(|x| x.as_i64());
    let raw_max = v
        .get("players")
        .and_then(|x| x.get("max"))
        .and_then(|x| x.as_i64());
    let players_max = match (players_online, raw_max) {
        (Some(o), Some(m)) if m >= o && m > 0 => Some(m),
        _ => None,
    };
    let favicon = v.get("favicon").and_then(|x| x.as_str()).map(|s| s.to_string());
    let motd = v.get("description").map(flatten_chat);

    Ok(ServerStatus {
        online: true,
        address: addr.to_string(),
        name: None,
        version,
        players_online,
        players_max,
        motd,
        favicon,
        latency_ms,
        error: None,
    })
}

/// 发送 0x01 Ping（payload = 时间戳），读取 0x01 Pong，返回往返耗时(ms)
async fn send_ping_pong<R, W>(reader: &mut R, writer: &mut W, timeout: Duration) -> Result<i64, String>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let payload = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut body = Vec::new();
    body.push(0x01u8);
    body.extend_from_slice(&payload.to_be_bytes());
    let mut packet = Vec::new();
    write_varint(&mut packet, body.len() as i32);
    packet.extend_from_slice(&body);
    write_all_timeout(writer, &packet, timeout).await?;
    let t0 = Instant::now();
    let len = read_varint_timeout(reader, timeout).await?;
    if len != 9 {
        return Err("pong 长度异常".into());
    }
    let mut buf = [0u8; 9];
    read_exact_timeout(reader, &mut buf, timeout).await?;
    if buf[0] != 0x01 {
        return Err("pong 类型异常".into());
    }
    Ok(t0.elapsed().as_millis() as i64)
}

fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut temp = (value & 0b0111_1111) as u8;
        value >>= 7;
        if value != 0 {
            temp |= 0b1000_0000;
        }
        buf.push(temp);
        if value == 0 {
            break;
        }
    }
}

async fn write_all_timeout<W: AsyncWriteExt + Unpin>(w: &mut W, buf: &[u8], timeout: Duration) -> Result<(), String> {
    match tokio::time::timeout(timeout, w.write_all(buf)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(format!("发送失败: {e}")),
        Err(_) => Err("发送超时".into()),
    }
}

async fn read_varint_timeout<R: AsyncReadExt + Unpin>(r: &mut R, timeout: Duration) -> Result<i32, String> {
    let mut num_read = 0;
    let mut result = 0i32;
    loop {
        let mut byte = [0u8; 1];
        read_exact_timeout(r, &mut byte, timeout).await?;
        let b = byte[0];
        result |= ((b & 0b0111_1111) as i32) << (7 * num_read);
        num_read += 1;
        if num_read > 5 {
            return Err("VarInt 过长".into());
        }
        if (b & 0b1000_0000) == 0 {
            break;
        }
    }
    Ok(result)
}

async fn read_exact_timeout<R: AsyncReadExt + Unpin>(r: &mut R, buf: &mut [u8], timeout: Duration) -> Result<(), String> {
    match tokio::time::timeout(timeout, r.read_exact(buf)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("读取失败: {e}")),
        Err(_) => Err("读取超时".into()),
    }
}

fn read_varint_at(buf: &[u8], pos: &mut usize) -> Result<i32, String> {
    let mut num_read = 0;
    let mut result = 0i32;
    loop {
        if *pos >= buf.len() {
            return Err("VarInt 越界".into());
        }
        let b = buf[*pos];
        *pos += 1;
        result |= ((b & 0b0111_1111) as i32) << (7 * num_read);
        num_read += 1;
        if num_read > 5 {
            return Err("VarInt 过长".into());
        }
        if (b & 0b1000_0000) == 0 {
            break;
        }
    }
    Ok(result)
}

/// 把 MC 的聊天组件压平为带 § 颜色码的纯文本
fn flatten_chat(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => {
            let mut out = String::new();
            if let Some(Value::String(c)) = o.get("color") {
                if let Some(code) = mc_color_code(c) {
                    out.push('§');
                    out.push(code);
                }
            }
            for f in ["bold", "italic", "underlined", "strikethrough", "obfuscated"] {
                if o.get(f).and_then(|x| x.as_bool()).unwrap_or(false) {
                    if let Some(code) = mc_color_code(f) {
                        out.push('§');
                        out.push(code);
                    }
                }
            }
            if let Some(Value::String(t)) = o.get("text") {
                out.push_str(t);
            }
            if let Some(Value::Array(extra)) = o.get("extra") {
                for e in extra {
                    out.push_str(&flatten_chat(e));
                }
            }
            out
        }
        Value::Array(a) => a.iter().map(flatten_chat).collect::<String>(),
        _ => String::new(),
    }
}

/// MC 颜色/格式名 -> § 颜色码
fn mc_color_code(name: &str) -> Option<char> {
    Some(match name {
        "black" => '0',
        "dark_blue" => '1',
        "dark_green" => '2',
        "dark_aqua" => '3',
        "dark_red" => '4',
        "dark_purple" => '5',
        "gold" => '6',
        "gray" => '7',
        "dark_gray" => '8',
        "blue" => '9',
        "green" => 'a',
        "aqua" => 'b',
        "red" => 'c',
        "light_purple" => 'd',
        "yellow" => 'e',
        "white" => 'f',
        "obfuscated" => 'k',
        "bold" => 'l',
        "italic" => 'o',
        "underlined" => 'n',
        "strikethrough" => 'm',
        "reset" => 'r',
        _ => return None,
    })
}
