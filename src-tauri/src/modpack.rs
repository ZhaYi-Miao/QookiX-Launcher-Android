use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use anyhow::{Context, Result};
use serde_json::Value;
use uuid::Uuid;
use chrono::Utc;
use crate::models::*;
use crate::settings::get_data_dir;

const MODRINTH_API: &str = "https://api.modrinth.com/api/v2";
const CURSEFORGE_API: &str = "https://api.cfaddon.com/v1";

// ==================== Modrinth API ====================

#[derive(Debug, Clone)]
pub struct ModrinthMod {
    pub slug: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub authors: Vec<String>,
    pub downloads: i64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub project_type: String,
    pub date_created: String,
    pub date_modified: String,
}

#[derive(Debug, Clone)]
pub struct ModrinthVersion {
    pub id: String,
    pub version_number: String,
    pub changelog: String,
    pub download_url: String,
    pub file_name: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
}

pub async fn search_modrinth(query: &str, limit: i32, offset: i32, project_type: Option<&str>) -> Result<Vec<ModrinthMod>> {
    let client = crate::util::http_client().await;
    
    let mut url = format!("{}/search", MODRINTH_API);
    let params = format!(
        "?query={}&limit={}&offset={}&indexed=false",
        urlencoding::encode(query),
        limit,
        offset
    );
    url = format!("{}{}", url, params);
    
    if let Some(pt) = project_type {
        url = format!("{}&project_type={}", url, pt);
    }

    let response = client.get(&url).send().await
        .context("Failed to search Modrinth")?;

    let result: Value = response.json().await
        .context("Failed to parse Modrinth response")?;

    let hits = result["hits"].as_array()
        .ok_or_else(|| anyhow::anyhow!("No hits in response"))?;

    let mut mods = Vec::new();
    for hit in hits {
        mods.push(ModrinthMod {
            slug: hit["slug"].as_str().unwrap_or("").to_string(),
            id: hit["project_id"].as_str().unwrap_or("").to_string(),
            title: hit["title"].as_str().unwrap_or("").to_string(),
            description: hit["description"].as_str().unwrap_or("").to_string(),
            authors: hit["authors"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
                .collect(),
            downloads: hit["downloads"].as_i64().unwrap_or(0) as i64,
            icon_url: hit["icon_url"].as_str().map(|s| s.to_string()),
            categories: hit["categories"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|c| c.as_str().map(|s| s.to_string()))
                .collect(),
            project_type: hit["project_type"].as_str().unwrap_or("").to_string(),
            date_created: hit["date_created"].as_str().unwrap_or("").to_string(),
            date_modified: hit["date_modified"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(mods)
}

pub async fn get_modrinth_mod(mod_id: &str) -> Result<ModrinthMod> {
    let client = crate::util::http_client().await;
    let response = client.get(format!("{}/project/{}", MODRINTH_API, mod_id))
        .send().await.context("Failed to get Modrinth mod")?;
    let result: Value = response.json().await.context("Failed to parse Modrinth response")?;
    Ok(ModrinthMod {
        slug: result["slug"].as_str().unwrap_or("").to_string(),
        id: result["id"].as_str().unwrap_or("").to_string(),
        title: result["title"].as_str().unwrap_or("").to_string(),
        description: result["description"].as_str().unwrap_or("").to_string(),
        authors: result["authors"].as_array().unwrap_or(&vec![])
            .iter().filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
            .collect(),
        downloads: result["downloads"].as_i64().unwrap_or(0) as i64,
        icon_url: result["icon_url"].as_str().map(|s| s.to_string()),
        categories: result["categories"].as_array().unwrap_or(&vec![])
            .iter().filter_map(|c| c.as_str().map(|s| s.to_string()))
            .collect(),
        project_type: result["project_type"].as_str().unwrap_or("").to_string(),
        date_created: result["date_created"].as_str().unwrap_or("").to_string(),
        date_modified: result["date_modified"].as_str().unwrap_or("").to_string(),
    })
}

pub async fn get_modrinth_versions(project_id: &str) -> Result<Vec<ModrinthVersion>> {
    let client = crate::util::http_client().await;
    let response = client.get(format!("{}/project/{}/version", MODRINTH_API, project_id))
        .send().await.context("Failed to get Modrinth versions")?;
    let result: Value = response.json().await.context("Failed to parse Modrinth versions response")?;
    let versions = result.as_array().ok_or_else(|| anyhow::anyhow!("No versions in response"))?;
    let mut mod_versions = Vec::new();
    for version in versions {
        let downloads = version["files"].as_array().and_then(|f| f.first())
            .and_then(|f| f["url"].as_str()).unwrap_or("").to_string();
        mod_versions.push(ModrinthVersion {
            id: version["id"].as_str().unwrap_or("").to_string(),
            version_number: version["version_number"].as_str().unwrap_or("").to_string(),
            changelog: version["changelog"].as_str().unwrap_or("").to_string(),
            download_url: downloads,
            file_name: version["files"].as_array().and_then(|f| f.first())
                .and_then(|f| f["filename"].as_str()).unwrap_or("").to_string(),
            game_versions: version["game_versions"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
            loaders: version["loaders"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|l| l["name"].as_str().map(|s| s.to_string())).collect(),
            date_published: version["date_published"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(mod_versions)
}

// ==================== CurseForge API ====================

#[derive(Debug, Clone)]
pub struct CurseforgeMod {
    pub id: u64,
    pub name: String,
    pub summary: String,
    pub authors: Vec<String>,
    pub downloads: i64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub date_created: String,
    pub date_modified: String,
}

#[derive(Debug, Clone)]
pub struct CurseforgeFile {
    pub id: u64,
    pub file_name: String,
    pub download_url: String,
    pub game_versions: Vec<String>,
    pub release_type: String,
    pub date_created: String,
}

pub async fn search_curseforge(query: &str, game_id: u32, category_id: Option<u32>, page_size: i32, page: i32) -> Result<Vec<CurseforgeMod>> {
    let client = crate::util::http_client().await;
    let mut body = HashMap::new();
    body.insert("gameId", Value::Number(game_id.into()));
    body.insert("searchFilter", Value::String(query.to_string()));
    body.insert("pageSize", Value::Number(page_size.into()));
    body.insert("page", Value::Number(page.into()));
    body.insert("sort", Value::String("Featured".to_string()));
    if let Some(cat_id) = category_id {
        body.insert("classId", Value::Number(cat_id.into()));
    }
    let response = client.post(format!("{}/mods/search", CURSEFORGE_API)).json(&body)
        .send().await.context("Failed to search CurseForge")?;
    let result: Value = response.json().await.context("Failed to parse CurseForge response")?;
    let data = result["data"].as_array().ok_or_else(|| anyhow::anyhow!("No data in response"))?;
    let mut mods = Vec::new();
    for item in data {
        mods.push(CurseforgeMod {
            id: item["id"].as_i64().unwrap_or(0) as u64,
            name: item["name"].as_str().unwrap_or("").to_string(),
            summary: item["summary"].as_str().unwrap_or("").to_string(),
            authors: item["authors"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
                .collect(),
            downloads: item["downloadCount"].as_i64().unwrap_or(0) as i64,
            icon_url: item["logo"].as_object().and_then(|v| v["thumbnailUrl"].as_str()).map(|s| s.to_string()),
            categories: item["categories"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|c| c["name"].as_str().map(|s| s.to_string()))
                .collect(),
            date_created: item["dateCreated"].as_str().unwrap_or("").to_string(),
            date_modified: item["dateModified"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(mods)
}

pub async fn get_curseforge_mod(mod_id: u64) -> Result<CurseforgeMod> {
    let client = crate::util::http_client().await;
    let response = client.get(format!("{}/mod/{}", CURSEFORGE_API, mod_id))
        .send().await.context("Failed to get CurseForge mod")?;
    let result: Value = response.json().await.context("Failed to parse CurseForge response")?;
    Ok(CurseforgeMod {
        id: result["id"].as_i64().unwrap_or(0) as u64,
        name: result["name"].as_str().unwrap_or("").to_string(),
        summary: result["summary"].as_str().unwrap_or("").to_string(),
        authors: result["authors"].as_array().unwrap_or(&vec![])
            .iter().filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
            .collect(),
        downloads: result["downloadCount"].as_i64().unwrap_or(0) as i64,
        icon_url: result["logo"].as_object().and_then(|v| v["thumbnailUrl"].as_str()).map(|s| s.to_string()),
        categories: result["categories"].as_array().unwrap_or(&vec![])
            .iter().filter_map(|c| c["name"].as_str().map(|s| s.to_string()))
            .collect(),
        date_created: result["dateCreated"].as_str().unwrap_or("").to_string(),
        date_modified: result["dateModified"].as_str().unwrap_or("").to_string(),
    })
}

pub async fn get_curseforge_files(mod_id: u64) -> Result<Vec<CurseforgeFile>> {
    let client = crate::util::http_client().await;
    let response = client.get(format!("{}/mod/{}/files", CURSEFORGE_API, mod_id))
        .send().await.context("Failed to get CurseForge files")?;
    let result: Value = response.json().await.context("Failed to parse CurseForge files response")?;
    let files = result["data"].as_array().ok_or_else(|| anyhow::anyhow!("No files in response"))?;
    let mut curseforge_files = Vec::new();
    for file in files {
        let download_url = file["downloadUrl"].as_str().unwrap_or("").to_string();
        curseforge_files.push(CurseforgeFile {
            id: file["id"].as_i64().unwrap_or(0) as u64,
            file_name: file["fileName"].as_str().unwrap_or("").to_string(),
            download_url: if download_url.is_empty() {
                format!("{}/mod/{}/file/{}/download", CURSEFORGE_API, mod_id, file["id"].as_i64().unwrap_or(0))
            } else { download_url },
            game_versions: file["gameVersions"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
            release_type: file["releaseType"].as_i64().unwrap_or(0).to_string(),
            date_created: file["fileDate"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(curseforge_files)
}

// ==================== Modpack Import ====================

/// 解析整合包的加载器与版本，兼容两种 manifest：
///
/// - **CurseForge**：`minecraft.modLoaders = [{"id": "forge-47.2.0", "primary": true}]`
///   —— 是**对象数组**。旧代码按字符串数组读（`loaders[0].as_str()`）永远得到 None，
///   于是**所有整合包都被识别成 Vanilla**。
/// - **Modrinth**：`dependencies = {"forge": "47.2.0"}` / `{"fabric-loader": "0.15.0"}`。
fn parse_loader(info: &Value) -> (Loader, Option<String>) {
    if let Some(list) = info["minecraft"]["modLoaders"].as_array() {
        for entry in list {
            let id = entry
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| entry["id"].as_str().map(|s| s.to_string()));
            let Some(id) = id else { continue };
            if id.is_empty() {
                continue;
            }
            let lower = id.to_ascii_lowercase();
            let (loader, prefix) = if lower.starts_with("neoforge") {
                (Loader::NeoForge, "neoforge")
            } else if lower.starts_with("forge") {
                (Loader::Forge, "forge")
            } else if lower.starts_with("fabric") {
                (Loader::Fabric, "fabric")
            } else if lower.starts_with("quilt") {
                (Loader::Quilt, "quilt")
            } else {
                continue;
            };
            let version = id[prefix.len()..].trim_start_matches(['-', '_']).trim();
            return (
                loader,
                if version.is_empty() { None } else { Some(version.to_string()) },
            );
        }
    }

    if let Some(deps) = info["dependencies"].as_object() {
        for (key, value) in deps {
            let loader = match key.as_str() {
                "forge" => Loader::Forge,
                "neoforge" => Loader::NeoForge,
                "fabric-loader" => Loader::Fabric,
                "quilt-loader" => Loader::Quilt,
                _ => continue,
            };
            let version = value.as_str().unwrap_or("").trim().to_string();
            return (loader, if version.is_empty() { None } else { Some(version) });
        }
    }

    (Loader::Vanilla, None)
}
/// 整合包清单的常见文件名，**按优先级**尝试。
///
/// 以前只认 `modpack.json` —— 那只是本项目自定义的格式。真实整合包用的是模组平台的
/// 标准清单，于是「导入真实整合包必失败」：
///   - Modrinth   → `modrinth.index.json`
///   - CurseForge → `manifest.json`
const MODPACK_MANIFESTS: [&str; 3] = ["modrinth.index.json", "manifest.json", "modpack.json"];

/// 清单里一条需要联网下载的文件（Modrinth `files[]` 的形态）。
struct PackFile {
    /// 包内相对路径，同时也是落盘位置（如 `mods/xxx.jar`）
    rel_path: String,
    url: String,
    sha1: Option<String>,
}

/// 把 zip 里 `prefix` 下的条目**流式**解压到实例目录，返回文件数。
///
/// 同步实现，由 `spawn_blocking` 调用（原因见 `import_modpack` 里那段注释）。
fn extract_overrides_sync(
    zip_path: &Path,
    instance_dir: &Path,
    prefix: &str,
) -> Result<usize, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("打开整合包失败: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("解析整合包失败: {e}"))?;

    let mut extracted = 0usize;
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let entry_name = entry.name().to_string();
        let Some(rel) = entry_name.strip_prefix(prefix) else {
            continue;
        };
        let rel = rel.trim_start_matches('/');
        if rel.is_empty() {
            continue;
        }

        let dest_path = match crate::fsutil::resolve_in_dir(instance_dir, rel, "实例") {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("跳过不安全的整合包条目 {entry_name}: {e}");
                continue;
            }
        };

        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&dest_path);
        } else {
            if let Some(parent) = dest_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut out = std::fs::File::create(&dest_path)
                .map_err(|e| format!("创建文件失败 {}: {e}", dest_path.display()))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("写入文件失败: {e}"))?;
            extracted += 1;
        }
    }
    Ok(extracted)
}

pub async fn import_modpack(file_path: &str) -> Result<MinecraftProfile> {
    use std::io::Read;
    let data_dir = get_data_dir().await?;

    // 兜底：安卓文件选择器给的是 SAF 的 `content://` URI，对它 `File::open` 必然 ENOENT。
    // 正常路径上前端已用 `filePicker` 把内容落地成真实文件，这里只保证万一漏掉时
    // 报错是看得懂的，而不是一句 "Failed to open modpack file"。
    if file_path.starts_with("content://") {
        anyhow::bail!("整合包路径未被落地为真实文件（收到 content:// URI，请从应用内选择文件）");
    }

    let file = std::fs::File::open(file_path).context("Failed to open modpack file")?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to open zip archive")?;

    // 按优先级找清单
    let mut json_content = String::new();
    let mut manifest_name = "";
    for name in MODPACK_MANIFESTS {
        let Ok(mut entry) = archive.by_name(name) else {
            continue;
        };
        if entry.read_to_string(&mut json_content).is_ok() && !json_content.trim().is_empty() {
            manifest_name = name;
            break;
        }
        json_content.clear();
    }
    if manifest_name.is_empty() {
        anyhow::bail!(
            "无法识别整合包格式：包内没有 modrinth.index.json / manifest.json / modpack.json"
        );
    }
    tracing::info!("整合包清单：{manifest_name}");

    let modpack_info: Value = serde_json::from_str(&json_content)
        .with_context(|| format!("解析 {manifest_name} 失败"))?;
    let instance_name = modpack_info["name"]
        .as_str()
        .or_else(|| modpack_info["title"].as_str())
        .unwrap_or("Modpack")
        .to_string();
    // 三种格式的字段位置都覆盖：
    //   Modrinth   → `dependencies.minecraft`
    //   CurseForge → `minecraft.version`
    //   本项目格式 → 两者之一
    let mc_version = modpack_info["minecraft"]["version"]
        .as_str()
        .or_else(|| modpack_info["dependencies"]["minecraft"].as_str())
        .unwrap_or("1.20.1")
        .to_string();
    let (loader, loader_version) = parse_loader(&modpack_info);
    let instance_id = Uuid::new_v4().to_string();
    let instance_dir = Path::new(&data_dir).join("instances").join(&instance_id);
    fs::create_dir_all(&instance_dir).await.context("Failed to create instance directory")?;
    // `overrides` 字段是**目录名**（默认 "overrides"），不是相对路径，所以前缀是 `{名字}/`。
    // 旧代码拼成 "overrides/overrides"，只靠后面 `starts_with("overrides/")` 兜底才歪打正着。
    let overrides_dir = modpack_info["overrides"]
        .as_str()
        .unwrap_or("overrides")
        .trim_matches('/');
    let prefix = format!(
        "{}/",
        if overrides_dir.is_empty() { "overrides" } else { overrides_dir }
    );

    // ── 解析 `files[]`（以前完全没有这一步 → 装出来是个空整合包）──────────
    // Modrinth 的条目带真实直链（`downloads[]`）；
    // CurseForge 的条目只有 projectID/fileID，换直链需要官方 API 且第三方客户端受限，
    // 那部分只能统计数量后**如实提示**，不能假装装好了。
    let mut pack_files: Vec<PackFile> = Vec::new();
    let mut no_url_files = 0usize;
    if let Some(arr) = modpack_info["files"].as_array() {
        for it in arr {
            let rel = it["path"].as_str().unwrap_or("").trim().to_string();
            if rel.is_empty() {
                continue;
            }
            let url = it["downloads"]
                .as_array()
                .and_then(|a| a.iter().find_map(|u| u.as_str()))
                .or_else(|| it["downloadUrl"].as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if url.is_empty() {
                no_url_files += 1;
                continue;
            }
            // 相对路径同样要防穿越：json 里的 path 和 zip 条目名一样可能是 `../../x`
            if crate::fsutil::resolve_in_dir(&instance_dir, &rel, "实例").is_err() {
                tracing::warn!("跳过不安全的整合包条目 {rel}");
                continue;
            }
            let sha1 = it["hashes"]["sha1"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| it["sha1"].as_str().map(|s| s.to_string()));
            pack_files.push(PackFile {
                rel_path: rel,
                url,
                sha1,
            });
        }
    }

    // 解压 overrides。三件事：
    //   1. **zip-slip**：目标路径经 `fsutil::resolve_in_dir` 校验，条目名里的 `../..`
    //      不能再把文件写到实例目录之外（`overrides/../../settings.json`）；
    //   2. **内存**：`std::io::copy` 流式落盘，不把所有条目先 read_to_end 进 Vec
    //      （整合包动辄几百 MB，那样手机上直接 OOM）；
    //   3. **不阻塞**：整段是同步 IO，挪进 `spawn_blocking` —— 以前它占着 tokio worker，
    //      解压期间前端连进度都刷不动。
    // 这里重新打开一次 zip 而不是把 `archive` move 进闭包：闭包要求 `Send`，
    // 而重开只读一次目录头，代价可忽略。
    let zip_path = std::path::PathBuf::from(file_path);
    let instance_dir_for_extract = instance_dir.clone();
    let prefix_for_extract = prefix.clone();
    let extracted = tokio::task::spawn_blocking(move || {
        extract_overrides_sync(&zip_path, &instance_dir_for_extract, &prefix_for_extract)
    })
    .await
    .map_err(|e| anyhow::anyhow!("解压任务失败: {e}"))?
    // `String` 不是 `StdError`，anyhow 不会自动转换，得显式包一层
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    tracing::info!("整合包 overrides 解压完成：{extracted} 个文件");

    // ── 下载 `files[]` 里的远程内容（mods / resourcepacks / shaderpacks…）──
    // 走 `download::download_file` 而不是自己发请求，是为了复用镜像改写、SHA1 校验
    // 和「镜像失败回退官方源」那套逻辑。
    if !pack_files.is_empty() {
        let ctx = crate::progress::TaskCtx::new(&instance_id, &instance_name, "整合包内容");
        let total = pack_files.len();
        let mut failed = 0usize;
        for (i, pf) in pack_files.iter().enumerate() {
            let dest = instance_dir.join(&pf.rel_path);
            crate::progress::emit_install(
                &ctx,
                "content",
                &format!("正在下载 {}", pf.rel_path),
                i,
                total,
            );
            if let Err(e) =
                crate::download::download_file(&pf.url, &dest.to_string_lossy(), pf.sha1.clone())
                    .await
            {
                failed += 1;
                tracing::warn!("整合包文件下载失败 {}: {e}", pf.rel_path);
            }
        }
        crate::progress::emit_install_done(
            &ctx,
            failed == 0,
            &format!("整合包内容：成功 {} / 失败 {failed}", total - failed),
            total - failed,
            total,
        );
        tracing::info!("整合包内容下载完成：{} 个，失败 {failed} 个", total - failed);
    }
    if no_url_files > 0 {
        tracing::warn!(
            "整合包有 {no_url_files} 个文件没有下载直链（CurseForge 格式需要官方 API），已跳过"
        );
    }

    Ok(MinecraftProfile {
        id: instance_id, name: instance_name, mc_version, loader, loader_version,
        created: Utc::now().timestamp(), last_played: None, total_play_time: 0,
        game_dir: instance_dir.to_string_lossy().to_string(), java_dir: String::new(),
        java_args: None, game_args: None, resolution: Some((854, 480)),
        max_memory_mb: Some(4096), memory_mode: Some("auto".to_string()),
        account_id: None, icon: None, mods: Vec::new(), resource_packs: Vec::new(),
        shaders: Vec::new(), group: None, is_symlink: None,
        source_path: Some(file_path.to_string()),
        // 必须是 false：全项目**唯一**的「安装游戏」按钮由 `installed` 控制
        // （`InstanceDetailView.vue` 的 `v-if="!instance.installed"`）。整合包导入
        // 只铺了 mods/配置，游戏本体还没装 —— 这里写 true 会让那个按钮永久消失，
        // 游戏本体再也装不上。
        installed: false,
    })
}

// ==================== Mod Installation ====================

pub async fn install_mod(instance_id: &str, mod_id: &str, mod_url: &str, file_name: Option<&str>) -> Result<InstalledContent> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    crate::fsutil::validate_id(mod_id, "模组").map_err(|e| anyhow::anyhow!(e))?;
    let data_dir = get_data_dir().await?;
    let instance_dir = Path::new(&data_dir).join("instances").join(instance_id);
    let mods_dir = instance_dir.join("mods");
    fs::create_dir_all(&mods_dir).await.context("Failed to create mods directory")?;
    let file_name = file_name.unwrap_or(&format!("{}.jar", mod_id)).to_string();
    let dest = mods_dir.join(&file_name);
    crate::download::download_file(mod_url, &dest.to_string_lossy(), None).await
        .context("Failed to download mod")?;
    let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    let mut record = InstalledContent::manual(file_name.clone());
    record.project_id = Some(mod_id.to_string());
    record.size = size as i64;
    // 记录写入实例元数据
    let _ = crate::instances::add_content(instance_id, "mod", record.clone()).await;
    Ok(record)
}

pub async fn uninstall_mod(instance_id: &str, mod_id: &str) -> Result<()> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let data_dir = get_data_dir().await?;
    let instance_dir = Path::new(&data_dir).join("instances").join(instance_id);
    let file_path = instance_dir.join("mods").join(format!("{}.jar", mod_id));
    if file_path.exists() {
        fs::remove_file(&file_path).await.context("Failed to remove mod file")?;
    }
    Ok(())
}

pub async fn get_installed_mods(instance_id: &str) -> Result<Vec<InstalledContent>> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    let data_dir = get_data_dir().await?;
    let instance_dir = Path::new(&data_dir).join("instances").join(instance_id);
    let mods_dir = instance_dir.join("mods");
    let mut mods = Vec::new();
    let mut entries = fs::read_dir(&mods_dir).await.context("Failed to read mods directory")?;
    while let Some(entry) = entries.next_entry().await.context("Failed to read next entry")? {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "jar") {
            if let Ok(metadata) = fs::metadata(&path).await {
                let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                let mut record = InstalledContent::manual(file_name.clone());
                record.size = metadata.len() as i64;
                record.installed_at = metadata.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()).unwrap_or(0) as i64;
                mods.push(record);
            }
        }
    }
    Ok(mods)
}
