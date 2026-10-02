//! 内容中心翻译：自建翻译服务 / 自定义 OpenAI 兼容接口，带本地磁盘缓存。
//!
//! 「百度网页」那条不在这里 —— 它由前端用系统浏览器打开翻译页，后端不参与。
//!
//! 缓存落在 `<data>/cache/translations.json`，整个文件是一个 JSON 对象：
//! `{ "<service>:<slug>:zh": { "text": "…", "ts": <unix 秒> } }`。
//!
//! 缓存键里的 `service` 是**服务来源标记**：走自建服务时用平台名
//! （`modrinth` / `curseforge`），走自定义接口时统一用 `custom` ——
//! 免得同一份描述在两个服务之间互相覆盖。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// 自建翻译服务地址。
const BASE: &str = "https://trans.zhayi.cc";
const LANG: &str = "zh";
/// 缓存有效期：7 天。
const CACHE_TTL: u64 = 7 * 24 * 3600;
/// 服务端批量接口单次上限。
const BATCH: usize = 5;
/// 正文超过这个长度就截断（与服务端上限对齐），避免请求被拒。
const MAX_BODY_CHARS: usize = 12000;

/// 自定义接口的系统提示词：只输出译文，避免模型加解释或引号。
const SYSTEM_PROMPT: &str = "你是 Minecraft 模组内容的翻译器。把用户提供的英文内容准确翻译成简体中文：\
保留模组名、专有名词与技术术语；保留 Markdown 结构；只输出译文本身，不要任何解释、引号或前后缀。";

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ==================== 配置 ====================

struct Cfg {
    /// `default`（自建服务）| `custom`（OpenAI 兼容接口）| `baidu_web`（前端处理）
    service: String,
    api_base: String,
    api_key: String,
    api_model: String,
}

impl Cfg {
    fn is_custom(&self) -> bool {
        self.service == "custom"
    }

    /// 缓存里用来区分服务来源的标记。
    fn tag(&self, provider: &str) -> String {
        if self.is_custom() {
            "custom".to_string()
        } else {
            provider.to_string()
        }
    }

    /// 是否支持正文翻译（百度网页模式不支持）。
    fn supports_body(&self) -> bool {
        matches!(self.service.as_str(), "default" | "custom")
    }
}

/// 读翻译配置。缺省走自建服务，与设置页默认值一致。
async fn cfg() -> Cfg {
    let m = crate::settings::load_raw_settings().await;
    Cfg {
        service: crate::settings::raw_str(&m, &["translate_provider"])
            .unwrap_or_else(|| "default".into()),
        api_base: crate::settings::raw_str(&m, &["translate_api_base"]).unwrap_or_default(),
        api_key: crate::settings::raw_str(&m, &["translate_api_key"]).unwrap_or_default(),
        api_model: crate::settings::raw_str(&m, &["translate_api_model"]).unwrap_or_default(),
    }
}

// ==================== 缓存 ====================

async fn cache_file() -> Option<PathBuf> {
    let dir = crate::settings::get_data_dir().await.ok()?;
    Some(Path::new(&dir).join("cache").join("translations.json"))
}

async fn load_cache() -> BTreeMap<String, Value> {
    let Some(path) = cache_file().await else {
        return BTreeMap::new();
    };
    match tokio::fs::read_to_string(&path).await {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => BTreeMap::new(),
    }
}

async fn save_cache(map: &BTreeMap<String, Value>) {
    let Some(path) = cache_file().await else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    if let Ok(text) = serde_json::to_string(map) {
        let _ = tokio::fs::write(&path, text).await;
    }
}

fn cache_key(service_tag: &str, slug: &str) -> String {
    format!("{service_tag}:{slug}:{LANG}")
}

/// 正文缓存键带格式版本号：正文渲染方式变了（比如换 Markdown 解析）时，
/// 升版号即可让老缓存整体失效，不用写迁移。
fn body_cache_key(provider: &str, slug: &str, service_tag: &str) -> String {
    format!("{provider}:{slug}:{LANG}:body:v2:{service_tag}")
}

/// 命中且未过期才返回。
fn cache_get(map: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    let entry = map.get(key)?;
    let ts = entry.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
    let text = entry.get("text").and_then(|v| v.as_str()).unwrap_or("");
    if text.is_empty() || now_secs().saturating_sub(ts) >= CACHE_TTL {
        return None;
    }
    Some(text.to_string())
}

fn cache_put(map: &mut BTreeMap<String, Value>, key: &str, text: &str) {
    map.insert(key.to_string(), json!({ "text": text, "ts": now_secs() }));
}

// ==================== 翻译服务 ====================

/// 自建服务：批量翻译描述（返回 译文表 / 被限流）。
async fn builtin_descriptions(
    provider: &str,
    slugs: &[String],
) -> Result<(BTreeMap<String, String>, bool), String> {
    let client = crate::util::http_client().await;
    let mut out = BTreeMap::new();
    for group in slugs.chunks(BATCH) {
        let body = json!({ "platform": provider, "lang": LANG, "mod_ids": group });
        let resp = client
            .post(format!("{BASE}/translate/mods"))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("请求翻译服务失败: {e}"))?;
        let status = resp.status().as_u16();
        if status == 429 {
            // 限流：本组与剩余都不再试，交给前端提示「稍后再试」
            return Ok((out, true));
        }
        if status != 200 {
            let msg = resp.text().await.unwrap_or_default();
            return Err(format!("翻译服务返回 HTTP {status}: {msg}"));
        }
        let parsed: Value = resp
            .json()
            .await
            .map_err(|e| format!("解析翻译响应失败: {e}"))?;
        if let Some(results) = parsed.get("results").and_then(|r| r.as_object()) {
            for slug in group {
                if let Some(text) = results
                    .get(slug)
                    .and_then(|e| e.get("text"))
                    .and_then(|t| t.as_str())
                {
                    if !text.is_empty() {
                        out.insert(slug.clone(), text.to_string());
                    }
                }
            }
        }
    }
    Ok((out, false))
}

/// 自建服务：翻译单个资源的正文。
async fn builtin_body(provider: &str, slug: &str) -> Result<String, String> {
    let client = crate::util::http_client().await;
    let req = json!({ "platform": provider, "lang": LANG, "mod_id": slug, "include_body": true });
    let resp = client
        .post(format!("{BASE}/translate/mod"))
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("请求翻译服务失败: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    if status != 200 {
        return Err(format!("翻译服务返回 HTTP {status}: {text}"));
    }
    let parsed: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析翻译响应失败: {e}"))?;
    if let Some(err) = parsed.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }
    parsed
        .get("body")
        .and_then(|b| b.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| "服务未返回正文译文".to_string())
}

/// 自定义 OpenAI 兼容接口：一次翻一段文本。
async fn chat_translate(cfg: &Cfg, text: &str) -> Result<String, String> {
    if cfg.api_base.trim().is_empty() {
        return Err("自定义翻译服务未配置接口地址".into());
    }
    if cfg.api_model.trim().is_empty() {
        return Err("自定义翻译服务未配置模型名".into());
    }
    let url = format!("{}/chat/completions", cfg.api_base.trim_end_matches('/'));
    let body = json!({
        "model": cfg.api_model,
        "temperature": 0.2,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": text },
        ],
    });
    let resp = crate::util::http_client()
        .await
        .post(url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err(format!("鉴权失败（HTTP {status}），请检查 API Key"));
    }
    if status == 429 {
        return Err("自定义服务限流（429）".into());
    }
    if !(200..300).contains(&status) {
        return Err(format!("自定义翻译服务返回 HTTP {status}"));
    }
    let v: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {e}"))?;
    v.pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "自定义服务未返回翻译内容".to_string())
}

/// 自定义接口：逐条取英文描述再翻译。返回（译文表 / 失败列表 / 错误信息）。
async fn custom_descriptions(
    cfg: &Cfg,
    provider: &str,
    slugs: &[String],
) -> (BTreeMap<String, String>, Vec<String>, Option<String>) {
    let mut out = BTreeMap::new();
    let mut failed = Vec::new();
    let mut error = None;
    for slug in slugs {
        let desc = match crate::browse::project_info(provider, slug).await {
            Ok(info) => info.description.trim().to_string(),
            Err(e) => {
                error = Some(e.to_string());
                failed.push(slug.clone());
                continue;
            }
        };
        if desc.is_empty() {
            failed.push(slug.clone());
            continue;
        }
        match chat_translate(cfg, &desc).await {
            Ok(text) => {
                out.insert(slug.clone(), text);
            }
            Err(e) => {
                let fatal = e.contains("鉴权失败") || e.contains("429");
                error = Some(e);
                failed.push(slug.clone());
                // 鉴权/限流是配置或配额问题，后面几条必然同样失败，不再打无谓的请求
                if fatal {
                    break;
                }
            }
        }
    }
    (out, failed, error)
}

// ==================== 对外命令 ====================

/// 批量翻译项目描述（命中缓存的不再请求服务）。
pub async fn translate_descriptions(provider: &str, slugs: Vec<String>) -> Result<Value, String> {
    let cfg = cfg().await;
    let tag = cfg.tag(provider);
    let mut cache = load_cache().await;

    let mut translations = BTreeMap::new();
    let mut pending = Vec::new();
    for slug in &slugs {
        match cache_get(&cache, &cache_key(&tag, slug)) {
            Some(text) => {
                translations.insert(slug.clone(), text);
            }
            None => pending.push(slug.clone()),
        }
    }

    let mut failed: Vec<String> = Vec::new();
    let mut rate_limited = false;
    let mut error: Option<String> = None;

    if !pending.is_empty() {
        if cfg.is_custom() {
            let (out, f, e) = custom_descriptions(&cfg, provider, &pending).await;
            for (slug, text) in &out {
                cache_put(&mut cache, &cache_key(&tag, slug), text);
                translations.insert(slug.clone(), text.clone());
            }
            failed = f;
            error = e;
        } else {
            match builtin_descriptions(provider, &pending).await {
                Ok((out, limited)) => {
                    rate_limited = limited;
                    for (slug, text) in &out {
                        cache_put(&mut cache, &cache_key(&tag, slug), text);
                        translations.insert(slug.clone(), text.clone());
                    }
                    for slug in &pending {
                        if !out.contains_key(slug) {
                            failed.push(slug.clone());
                        }
                    }
                }
                Err(e) => {
                    failed = pending.clone();
                    error = Some(e);
                }
            }
        }
        save_cache(&cache).await;
    }

    Ok(json!({
        "translations": translations,
        "failed": failed,
        "rateLimited": rate_limited,
        "error": error,
    }))
}

/// 取正文并（可选）翻译。`translate=false` 时只回原文，不调翻译服务。
pub async fn translate_body(provider: &str, slug: &str, translate: bool) -> Result<Value, String> {
    let cfg = cfg().await;
    let info = crate::browse::project_info(provider, slug)
        .await
        .map_err(|e| e.to_string())?;
    let original = info.body;
    let supported = cfg.supports_body();

    if !translate || !supported {
        return Ok(json!({
            "body": Value::Null,
            "bodyCached": false,
            "original": original,
            "supported": supported,
            "error": Value::Null,
        }));
    }

    let tag = cfg.tag(provider);
    let key = body_cache_key(provider, slug, &tag);
    let mut cache = load_cache().await;
    if let Some(text) = cache_get(&cache, &key) {
        return Ok(json!({
            "body": text,
            "bodyCached": true,
            "original": original,
            "supported": true,
            "error": Value::Null,
        }));
    }

    let result = if cfg.is_custom() {
        // 正文动辄上万字，先截断再喂接口（与自建服务的上限对齐）
        let clipped: String = original.chars().take(MAX_BODY_CHARS).collect();
        chat_translate(&cfg, &clipped).await
    } else {
        builtin_body(provider, slug).await
    };

    match result {
        Ok(text) if !text.is_empty() => {
            cache_put(&mut cache, &key, &text);
            save_cache(&cache).await;
            Ok(json!({
                "body": text,
                "bodyCached": false,
                "original": original,
                "supported": true,
                "error": Value::Null,
            }))
        }
        Ok(_) => Ok(json!({
            "body": Value::Null,
            "bodyCached": false,
            "original": original,
            "supported": true,
            "error": "服务未返回正文译文",
        })),
        Err(e) => Ok(json!({
            "body": Value::Null,
            "bodyCached": false,
            "original": original,
            "supported": true,
            "error": e,
        })),
    }
}

/// 反馈某条翻译已过期；服务端确认重译后作废本地缓存。
pub async fn report_stale(provider: &str, slug: &str) -> Result<String, String> {
    let body = json!({ "platform": provider, "mod_id": slug, "lang": LANG });
    let resp = crate::util::http_client()
        .await
        .post(format!("{BASE}/feedback/stale"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("反馈失败: HTTP {}", resp.status().as_u16()));
    }
    let v: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {e}"))?;
    let status = v
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("received")
        .to_string();
    if status == "updated" {
        let cfg = cfg().await;
        let mut cache = load_cache().await;
        cache.remove(&cache_key(&cfg.tag(provider), slug));
        save_cache(&cache).await;
    }
    Ok(status)
}

/// 反馈翻译质量。`issue_type` 只认四个值，其它一律拒绝。
pub async fn report_quality(
    provider: &str,
    slug: &str,
    issue_type: &str,
    user_suggestion: Option<String>,
    user_comment: Option<String>,
) -> Result<String, String> {
    const TYPES: [&str; 4] = ["wrong_translation", "unnatural", "missing", "other"];
    if !TYPES.contains(&issue_type) {
        return Err("未知的问题类型".into());
    }
    let mut body = json!({
        "platform": provider,
        "mod_id": slug,
        "lang": LANG,
        "issue_type": issue_type,
    });
    if let Some(s) = user_suggestion.filter(|s| !s.trim().is_empty()) {
        body["user_suggestion"] = json!(s);
    }
    if let Some(c) = user_comment.filter(|c| !c.trim().is_empty()) {
        body["user_comment"] = json!(c);
    }
    let resp = crate::util::http_client()
        .await
        .post(format!("{BASE}/feedback/quality"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("反馈失败: HTTP {}", resp.status().as_u16()));
    }
    let v: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {e}"))?;
    Ok(v.get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("received")
        .to_string())
}

/// 清空翻译缓存。`service` 为空表示全部清空，返回释放的字节数（估算）。
pub async fn clear_cache(service: Option<String>) -> Result<u64, String> {
    let Some(path) = cache_file().await else {
        return Err("无法定位缓存目录".into());
    };
    if !path.exists() {
        return Ok(0);
    }
    // 全部清空：直接删文件，省得逐条算
    if service.is_none() {
        let len = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(len);
    }
    let want_custom = service.as_deref() == Some("custom");
    let matches = |key: &str| {
        let is_custom = key.starts_with("custom:") || key.ends_with(":custom");
        is_custom == want_custom
    };
    let mut map = load_cache().await;
    let mut freed = 0u64;
    let keys: Vec<String> = map.keys().filter(|k| matches(k)).cloned().collect();
    for key in keys {
        if let Some(v) = map.remove(&key) {
            freed += serde_json::to_string(&v).map(|s| s.len() as u64).unwrap_or(0);
        }
    }
    save_cache(&map).await;
    Ok(freed)
}

/// 测试自定义接口连通性。`key` 留空表示用设置里已保存的那把。
pub async fn test_api(base: String, key: String, model: String) -> Result<(), String> {
    let saved = cfg().await;
    let key = if key.trim().is_empty() {
        saved.api_key
    } else {
        key
    };
    let test = Cfg {
        service: "custom".into(),
        api_base: base,
        api_key: key,
        api_model: model,
    };
    chat_translate(&test, "Hello, this is a test.").await.map(|_| ())
}
