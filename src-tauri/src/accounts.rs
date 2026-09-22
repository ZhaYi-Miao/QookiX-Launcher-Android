use std::path::Path;
use tokio::fs;
use anyhow::{Context, Result};
use uuid::Uuid;
use chrono::Utc;
use serde_json::{Value, json};
use crate::models::*;
use crate::settings::get_data_dir;

const MS_AUTH_URL: &str = "https://login.live.com/oauth20_token.srf";
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";

const MS_CLIENT_ID: &str = "00000000402b5328";
// ── 微软登录全部走 **v1.0（Live SDK）端点** ──────────────────────────────
// 内置的 `00000000402b5328` 是公开的 Minecraft 启动器 Live SDK ID，它**没有**
// 在 Azure AD 里注册过，v2.0 端点会直接拒：
//   POST login.microsoftonline.com/consumers/oauth2/v2.0/devicecode
//   → 400 {"error":"unauthorized_client","error_codes":[700016]}
//   (AADSTS700016: Application with identifier ... was not found in the directory)
// 而 v1.0 的 oauth20_connect.srf / oauth20_token.srf 会正常返回设备码与令牌
// （已实测）。所以下面这几个常量必须**整体**保持 v1.0 —— 之前的 bug 正是混搭：
// 设备码走了 v1.0、轮询却打到 v2.0 端点，于是永远拿不到令牌。
const MS_DEVICE_CODE_URL: &str = "https://login.live.com/oauth20_connect.srf";
const MS_TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
/// v1.0 的固定 scope。v2.0 那套 `XboxLive.SignIn XboxLive.offline_access`
/// 在 v1.0 端点上无效。
const MS_AUTH_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

/// 轮询尚未授权时的错误码：前端捕获该精确字符串并继续轮询。
pub const ERR_AUTH_PENDING: &str = "__auth_pending__";

/// 进行中的设备码登录流程（全局单例，同一时间只允许一次登录）。
#[derive(Clone, Debug)]
struct MsFlow {
    device_code: String,
    interval: u64,
    expires_at: u64,
    client_id: String,
}

lazy_static::lazy_static! {
    static ref MS_FLOW: std::sync::Mutex<Option<MsFlow>> = std::sync::Mutex::new(None);
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 生效的 Microsoft Client ID：优先用户设置（settings.msClientId），回退内置 ID。
async fn effective_ms_client_id() -> Result<String> {
    let user_id = crate::settings::get_settings()
        .await
        .map(|s| s.ms_client_id.trim().to_string())
        .unwrap_or_default();
    if !user_id.is_empty() && user_id != "00000000-0000-0000-0000-000000000000" {
        return Ok(user_id);
    }
    if !MS_CLIENT_ID.is_empty() {
        return Ok(MS_CLIENT_ID.to_string());
    }
    Err(anyhow::anyhow!(
        "未配置 Microsoft Client ID：请在设置中填写，或用 MS_CLIENT_ID 环境变量重新构建"
    ))
}

// ---------------------------------------------------------------------------
// 微软设备码登录流（login_ms_start / login_ms_poll）
// ---------------------------------------------------------------------------

/// 第一步：申请设备码。返回用户需要在浏览器打开的验证链接与代码。
pub async fn ms_start() -> Result<Value> {
    let client_id = effective_ms_client_id().await?;
    let resp = crate::util::http_client()
        .await
        .post(MS_DEVICE_CODE_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            // v1.0（oauth20_connect.srf）**必须**带 response_type，否则 400：
            //   {"error":"invalid_request","error_description":
            //    "The provided request must include a 'response_type' input parameter."}
            // v2.0 的 devicecode 端点不需要这个参数 —— 这也是两套端点不能混用的另一个点。
            ("response_type", "device_code"),
            ("scope", MS_AUTH_SCOPE),
        ])
        .send()
        .await
        .context("请求设备码失败")?;
    let status = resp.status();
    let body: Value = resp.json().await.context("解析设备码响应失败")?;
    if !status.is_success() {
        let err = body.get("error").and_then(|v| v.as_str()).unwrap_or("");
        if err == "invalid_scope" {
            return Err(anyhow::anyhow!(
                "当前 Microsoft Client ID 未授权 Xbox Live 登录（scope {}）。\n\
                 内置的公开 Live SDK ID 只兼容 v1.0 端点，自定义 Client ID 需要在 \
                 Azure 门户注册并改用 v2.0 端点",
                MS_AUTH_SCOPE
            ));
        }
        return Err(anyhow::anyhow!("设备码请求失败 (HTTP {status}): {body}"));
    }
    let device_code = body
        .get("device_code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("响应缺少 device_code"))?
        .to_string();
    let interval = body.get("interval").and_then(|v| v.as_u64()).unwrap_or(5);
    let expires_in = body.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(900);
    *MS_FLOW.lock().unwrap() = Some(MsFlow {
        device_code,
        interval,
        expires_at: now_secs() + expires_in,
        client_id,
    });
    // 旧端点返回 verification_url，v2.0 返回 verification_uri
    let verification_uri = body
        .get("verification_uri")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("verification_url").and_then(|v| v.as_str()))
        .unwrap_or("https://microsoft.com/link");
    Ok(json!({
        "userCode": body.get("user_code").and_then(|v| v.as_str()).unwrap_or(""),
        "verificationUri": verification_uri,
        "expiresIn": expires_in,
    }))
}

/// 第二步：轮询令牌端点直到用户授权。成功后保存并返回新账号。
pub async fn ms_poll() -> Result<Account> {
    let flow = MS_FLOW
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("没有进行中的设备码登录"))?;
    if now_secs() > flow.expires_at {
        *MS_FLOW.lock().unwrap() = None;
        return Err(anyhow::anyhow!("设备码已过期，请重新开始登录"));
    }
    let resp = crate::util::http_client()
        .await
        .post(MS_TOKEN_URL)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", flow.client_id.as_str()),
            ("device_code", flow.device_code.as_str()),
        ])
        .send()
        .await
        .context("轮询失败")?;
    let status = resp.status();
    let body: Value = resp.json().await.context("解析令牌响应失败")?;
    if !status.is_success() {
        let err = body.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
        return Err(anyhow::anyhow!(match err {
            "authorization_pending" | "slow_down" => ERR_AUTH_PENDING.to_string(),
            "authorization_declined" => "用户拒绝了授权".to_string(),
            "expired_token" => "设备码已过期，请重新开始登录".to_string(),
            other => format!("登录失败: {other}"),
        }));
    }
    let refresh_token = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("缺少 refresh_token"))?
        .to_string();
    *MS_FLOW.lock().unwrap() = None;
    let account = perform_microsoft_login(&refresh_token, &flow.client_id).await?;
    add_account_to_disk(&account).await?;
    Ok(account)
}

async fn add_account_to_disk(account: &Account) -> Result<()> {
    let data_dir = get_data_dir().await?;
    let accounts_dir = Path::new(&data_dir).join("accounts");
    fs::create_dir_all(&accounts_dir).await.context("创建账号目录失败")?;
    let file_path = accounts_dir.join(format!("{}.json", account.uuid));
    let content = serde_json::to_string_pretty(account)?;
    fs::write(&file_path, content).await.context("写入账号文件失败")
}

pub async fn list_accounts() -> Result<Vec<Account>> {
    let data_dir = get_data_dir().await?;
    let accounts_dir = Path::new(&data_dir).join("accounts");

    if !accounts_dir.exists() {
        fs::create_dir_all(&accounts_dir).await.ok();
        return Ok(Vec::new());
    }

    let mut accounts = Vec::new();

    let mut entries = fs::read_dir(&accounts_dir).await
        .context("Failed to read accounts directory")?;
    
    while let Some(entry) = entries.next_entry().await
        .context("Failed to read next entry")? {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            let content = match fs::read_to_string(&entry.path()).await {
                Ok(c) => c,
                Err(_) => continue,
            };
            match serde_json::from_str::<Account>(&content) {
                Ok(account) => accounts.push(account),
                Err(e) => {
                    eprintln!("Skipping invalid account file {:?}: {}", entry.path(), e);
                    continue;
                }
            }
        }
    }

    Ok(accounts)
}

pub async fn get_account_by_uuid(uuid: &str) -> Result<Account> {
    let accounts = list_accounts().await?;
    accounts
        .into_iter()
        .find(|a| a.uuid == uuid)
        .ok_or_else(|| anyhow::anyhow!("Account not found: {}", uuid))
}

pub async fn add_account(username: &str, account_type: &str, refresh_token: Option<String>) -> Result<Account> {
    let data_dir = get_data_dir().await?;
    let accounts_dir = Path::new(&data_dir).join("accounts");

    fs::create_dir_all(&accounts_dir).await
        .context("Failed to create accounts directory")?;

    let account = if account_type == "microsoft" {
        if let Some(token) = refresh_token {
            let client_id = effective_ms_client_id().await?;
            perform_microsoft_login(&token, &client_id).await?
        } else {
            return Err(anyhow::anyhow!("Refresh token required for Microsoft account"));
        }
    } else {
        // Offline account：UUID 取 v3("OfflinePlayer:<name>")，与官方启动器 / Java 端一致，
        // 否则联机时服务器算出的 UUID 会对不上。
        let name = username.trim();
        if name.is_empty() {
            return Err(anyhow::anyhow!("用户名不能为空"));
        }
        if name.chars().count() > 16 {
            return Err(anyhow::anyhow!("用户名不能超过 16 个字符"));
        }
        Account {
            uuid: Uuid::new_v3(&Uuid::nil(), format!("OfflinePlayer:{name}").as_bytes()).to_string(),
            username: name.to_string(),
            created: Utc::now().timestamp(),
            account_type: AccountType::Offline,
            msa_refresh_token: None,
            xuid: None,
            expires_at: None,
            skin_face_base64: None,
        }
    };

    let file_path = accounts_dir.join(format!("{}.json", account.uuid));
    let content = serde_json::to_string_pretty(&account)?;
    fs::write(&file_path, content).await
        .context("Failed to write account file")?;

    Ok(account)
}

pub async fn remove_account(account_id: &str) -> Result<()> {
    let data_dir = get_data_dir().await?;
    let file_path = Path::new(&data_dir)
        .join("accounts")
        .join(format!("{}.json", account_id));

    if file_path.exists() {
        fs::remove_file(&file_path).await
            .context("Failed to remove account file")?;
    }

    Ok(())
}

pub async fn select_account(account_id: &str) -> Result<()> {
    // 键名必须是 snake_case：settings::get_settings 按该键名回读，
    // 否则重启后「当前账号」会丢失。
    let mut settings = crate::settings::load_raw_settings().await;
    settings.insert("selected_account".to_string(), Value::String(account_id.to_string()));
    crate::settings::save_raw_settings(&settings).await
}

/// 用已保存的 refresh token 重新走一遍 MS 登录流程，返回带最新访问令牌的账号。
/// 失败时返回原账号（保持调用方逻辑可用）。
pub async fn refresh_microsoft(account: &Account) -> Result<Account> {
    let Some(token) = account.msa_refresh_token.as_deref() else {
        return Ok(account.clone());
    };
    let client_id = effective_ms_client_id().await?;
    match perform_microsoft_login(token, &client_id).await {
        Ok(fresh) => Ok(fresh),
        Err(_) => Ok(account.clone()),
    }
}

/// 返回微软账号的 Minecraft access token（用于披风/皮肤等官方 API）。
pub async fn microsoft_access_token(account: &Account) -> Result<String> {
    if account.account_type != AccountType::Microsoft {
        return Err(anyhow::anyhow!("不是微软账号"));
    }
    let token = account
        .msa_refresh_token
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("账号缺少刷新令牌"))?;
    // client_id 必须与当初签发 refresh token 时一致
    let client_id = effective_ms_client_id().await?;
    ms_access_token(token, &client_id).await
}

/// 用 refresh token 换 OAuth access token（perform_microsoft_login 的第一步）。
/// `client_id` 必须与申请 refresh token 时一致，否则会 `invalid_grant`。
async fn ms_access_token(refresh_token: &str, client_id: &str) -> Result<String> {
    let client = crate::util::http_client().await;
    let resp = client
        .post(MS_AUTH_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("client_id", client_id),
            ("scope", "service::user.auth.xboxlive.com::MBI_SSL"),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .context("Failed to get access token")?;
    let body: Value = resp.json().await.context("Failed to parse token response")?;
    body["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("未获取到访问令牌"))
}

/// 取启动游戏要用的 **Minecraft access token**。
///
/// 为什么需要：`Account` 模型里只有 `msa_refresh_token`（没有 access token 字段），
/// 而启动参数 `--accessToken` 要的是 Minecraft 的 access token —— 以前直接把
/// refresh token 塞进去，游戏能起来但一连正版服务器就 401/掉线。
/// 这里每次启动现换一次，换不出来就把「正版会话已过期」明确报出去。
pub async fn minecraft_access_token(account: &Account) -> Result<String> {
    let Some(token) = account.msa_refresh_token.as_deref() else {
        return Err(anyhow::anyhow!("账号缺少刷新令牌，请重新登录正版账号"));
    };
    let client_id = effective_ms_client_id().await?;
    let (token, _uhs) = exchange_minecraft_token(token, &client_id).await?;
    Ok(token)
}

/// MSA → XBL → XSTS → Minecraft 四步。
///
/// 返回 `(Minecraft access token, uhs)` —— uhs 是 XSTS 里的用户哈希，
/// 落库时当 `xuid` 用，所以要和 token 一起带出来。
/// 单独抽出来是为了「只要 token、不要重新拉 profile」的启动路径复用。
async fn exchange_minecraft_token(refresh_token: &str, client_id: &str) -> Result<(String, String)> {
    let client = crate::util::http_client().await;

    // Step 1: Get Access Token
    let access_token_str = ms_access_token(refresh_token, client_id).await?;

    // Step 2: Get XBL Token
    let xbl_response = client
        .post(XBL_AUTH_URL)
        .header("Content-Type", "application/json")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                // v1.0 的 MSA access token 必须带 `d=` 前缀，漏了 XBL 会直接 400
                "RpsTicket": format!("d={access_token_str}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .context("Failed to get XBL token")?;

    let xbl: Value = xbl_response.json().await
        .context("Failed to parse XBL response")?;
    let xbl_token = xbl["Token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No XBL token in response"))?;

    // Step 3: Get XSTS Token
    let xsts_response = client
        .post(XSTS_AUTH_URL)
        .header("Content-Type", "application/json")
        .json(&json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .context("Failed to get XSTS token")?;

    let xsts: Value = xsts_response.json().await
        .context("Failed to parse XSTS response")?;
    let xsts_token = xsts["Token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No XSTS token in response"))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No UHS in XSTS response"))?;

    // Step 4: Get Minecraft Token
    let mc_response = client
        .post(MC_LOGIN_URL)
        .header("Content-Type", "application/json")
        .json(&json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token),
            "ensureLegacy": true
        }))
        .send()
        .await
        .context("Failed to get Minecraft token")?;

    let mc: Value = mc_response.json().await
        .context("Failed to parse Minecraft token response")?;
    let mc_token = mc["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No Minecraft token in response"))?
        .to_string();

    Ok((mc_token, uhs.to_string()))
}

/// 完整登录：换 token + 拉 profile，落成一个 `Account`。
async fn perform_microsoft_login(refresh_token: &str, client_id: &str) -> Result<Account> {
    let client = crate::util::http_client().await;
    let (mc_token, uhs) = exchange_minecraft_token(refresh_token, client_id).await?;

    // Step 5: Get Profile
    let profile_response = client
        .get(MC_PROFILE_URL)
        .header("Authorization", format!("Bearer {}", mc_token))
        .send()
        .await
        .context("Failed to get profile")?;

    let profile: Value = profile_response.json().await
        .context("Failed to parse profile response")?;

    let uuid = profile["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No UUID in profile"))?;
    let username = profile["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No username in profile"))?;

    // Check game ownership
    let _ = client
        .get(MC_ENTITLEMENTS_URL)
        .header("Authorization", format!("Bearer {}", mc_token))
        .send()
        .await
        .context("Failed to check entitlements")?;

    // Create account
    Ok(Account {
        uuid: uuid.to_string(),
        username: username.to_string(),
        created: Utc::now().timestamp(),
        account_type: AccountType::Microsoft,
        msa_refresh_token: Some(refresh_token.to_string()),
        xuid: Some(uhs.to_string()),
        expires_at: Some(Utc::now().timestamp() + 3600), // 1 hour
        skin_face_base64: None,
    })
}
