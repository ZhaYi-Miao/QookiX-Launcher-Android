//! 世界存档备份：把 `saves/<world>` 打包为 zip 快照，支持恢复与删除。
//! 备份存放于 `instances/<id>/backups/<world>/`，文件名形如 `<world>-<unix>.zip`。

use crate::models::BackupInfo;
use crate::settings::get_data_dir;
use std::io::Read;
use std::path::{Path, PathBuf};

/// 备份目录，**相对实例目录**（调用方负责拼上 `instances/<id>`）。
///
/// 以前这里返回的是 `PathBuf::from(instance_id).join("backups")`，而调用方又把它
/// 接到 `instances/<id>` 后面 —— 于是实际路径变成 `instances/<id>/<id>/backups/...`，
/// 备份列表永远为空、创建永远失败、恢复/删除都找不到文件。
fn backups_dir(world: &str) -> PathBuf {
    PathBuf::from("backups").join(world)
}

/// 存档目录，**相对实例目录**（同上）。
fn world_dir(world: &str) -> PathBuf {
    PathBuf::from("saves").join(world)
}

fn timestamp() -> String {
    // unix 秒作为唯一后缀；前端展示时转本地时间
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

/// 可读的备份文件名：<world>-<unix_secs>.zip
fn backup_filename(world: &str) -> String {
    format!("{}-{}.zip", world, timestamp())
}

fn info_from_file(p: &Path) -> Option<BackupInfo> {
    let meta = std::fs::metadata(p).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(BackupInfo {
        filename: p.file_name()?.to_string_lossy().to_string(),
        size: meta.len(),
        modified,
    })
}

async fn instance_dir(instance_id: &str) -> Result<PathBuf, String> {
    crate::fsutil::validate_id(instance_id, "实例")?;
    let data_dir = get_data_dir().await.map_err(|e| e.to_string())?;
    Ok(Path::new(&data_dir).join("instances").join(instance_id))
}

/// 列出某个世界的全部备份（按修改时间倒序）
pub async fn list_backups(instance_id: &str, world: &str) -> Vec<BackupInfo> {
    if crate::fsutil::validate_name(world).is_err() {
        return Vec::new();
    }
    let Ok(inst_dir) = instance_dir(instance_id).await else {
        return Vec::new();
    };
    let dir = inst_dir.join(backups_dir(world));
    let mut out: Vec<BackupInfo> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == "zip").unwrap_or(false))
                .filter_map(|p| info_from_file(&p))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

/// 递归把 `dir` 下的文件写入 zip（zip 内路径相对 `dir`）
fn write_dir_to_zip(zw: &mut zip::ZipWriter<std::fs::File>, dir: &Path, prefix: &str) -> Result<(), String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("读取目录失败: {e}"))?;
    for e in rd.flatten() {
        let p = e.path();
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        if p.is_dir() {
            write_dir_to_zip(zw, &p, &rel)?;
        } else if p.is_file() {
            let mut f = std::fs::File::open(&p).map_err(|e| format!("打开 {} 失败: {e}", p.display()))?;
            let opts = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zw.start_file(rel.clone(), opts)
                .map_err(|e| format!("写入 zip 条目 {rel} 失败: {e}"))?;
            std::io::copy(&mut f, zw).map_err(|e| format!("写入 {rel} 内容失败: {e}"))?;
        }
    }
    Ok(())
}

/// 创建备份：把 `saves/<world>` 打包为 zip。返回备份信息。
pub async fn create_backup(instance_id: &str, world: &str) -> Result<BackupInfo, String> {
    crate::fsutil::validate_name(world)?;
    if !crate::util::is_safe_filename(world) {
        return Err("非法的世界目录名".into());
    }
    let inst_dir = instance_dir(instance_id).await?;
    let src = inst_dir.join(world_dir(world));
    if !src.is_dir() {
        return Err(format!("世界目录不存在: {}", src.display()));
    }
    let dir = inst_dir.join(backups_dir(world));
    let world_owned = world.to_string();
    // 打包整个世界是纯同步 IO（几百 MB 的存档很常见），必须挪出 tokio worker，
    // 否则打包期间同 worker 上的其它 invoke 全部排队。
    tokio::task::spawn_blocking(move || create_backup_sync(&src, &dir, &world_owned))
        .await
        .map_err(|e| format!("备份任务失败: {e}"))?
}

fn create_backup_sync(src: &Path, dir: &Path, world: &str) -> Result<BackupInfo, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
    let zip_path = dir.join(backup_filename(world));
    let file = std::fs::File::create(&zip_path).map_err(|e| format!("创建备份文件失败: {e}"))?;
    let mut zw = zip::ZipWriter::new(file);
    write_dir_to_zip(&mut zw, src, "")?;
    zw.finish().map_err(|e| format!("完成 zip 失败: {e}"))?;
    info_from_file(&zip_path).ok_or_else(|| "读取备份信息失败".into())
}

/// 递归判断目录下是否至少有一个普通文件。
///
/// 不能只看 `extract_zip` 的返回值：它只统计**文件**条目，条目全是目录的 zip
/// 会返回 0 —— 那会把「备份是好的、只是条目都是目录」误判成空备份。
fn has_any_file(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if has_any_file(&p) {
                return true;
            }
        } else {
            return true;
        }
    }
    false
}

/// 恢复备份：用快照内容替换 `saves/<world>`。
///
/// 关键在**顺序**。原实现是「删掉原存档 → 建空目录 → 解压」，于是解压一旦失败
/// （zip 损坏、磁盘满、中途被杀）存档就已经被清空，而且那个「安全备份」写失败是
/// 被静默忽略的（`if let Ok(f)` + `let _ =`），用户以为有保底其实没有。现在的顺序：
///
///  1. 先解压到**临时目录**，失败或为空直接中止 —— 这时原存档一个字节都没动；
///  2. 再写「恢复前快照」，**写不出来就中止**（不再静默忽略）；
///  3. 最后才替换：原存档 `rename` 到 `.restore-old`，临时目录 `rename` 就位。
///     两步 rename 都在同一父目录内、是原子的；第二步失败会把原存档 rename 回去。
pub async fn restore_backup(instance_id: &str, world: &str, filename: &str) -> Result<(), String> {
    crate::fsutil::validate_name(world)?;
    if !crate::util::is_safe_filename(world) || !crate::util::is_safe_filename(filename) {
        return Err("非法的路径参数".into());
    }
    let inst_dir = instance_dir(instance_id).await?;
    let zip_path = inst_dir.join(backups_dir(world)).join(filename);
    if !zip_path.is_file() {
        return Err("备份文件不存在".into());
    }
    let dest = inst_dir.join(world_dir(world));

    // ── ① 先解压到临时目录 ────────────────────────────────────────────
    let staging = inst_dir.join(format!(".restore-{world}-tmp"));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| format!("清理临时目录失败: {e}"))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let n = match crate::util::extract_zip(&zip_path, &staging, &[]) {
        Ok(n) => n,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("解压备份失败（当前存档未改动）: {e}"));
        }
    };
    if n == 0 && !has_any_file(&staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("备份内容为空，已中止（当前存档未改动）".into());
    }

    // ── ② 恢复前快照：写不出来必须中止 ───────────────────────────────
    let safety = inst_dir.join(backups_dir(world)).join(backup_filename(world));
    if dest.is_dir() {
        if let Some(parent) = safety.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建备份目录失败: {e}"))?;
        }
        let f = std::fs::File::create(&safety)
            .map_err(|e| format!("创建恢复前快照失败，已中止: {e}"))?;
        let mut zw = zip::ZipWriter::new(f);
        write_dir_to_zip(&mut zw, &dest, "")?;
        zw.finish()
            .map_err(|e| format!("写入恢复前快照失败，已中止: {e}"))?;
    }

    // ── ③ 原子替换 ───────────────────────────────────────────────────
    let old = inst_dir.join(format!(".restore-{world}-old"));
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(|e| format!("清理残留目录失败: {e}"))?;
    }
    let had_dest = dest.exists();
    if had_dest {
        std::fs::rename(&dest, &old).map_err(|e| format!("移开当前存档失败: {e}"))?;
    }
    if let Err(e) = std::fs::rename(&staging, &dest) {
        // 回滚：把原存档放回原位，别让用户两头落空
        if had_dest {
            let _ = std::fs::rename(&old, &dest);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("替换存档失败，已回滚到原存档: {e}"));
    }
    if had_dest {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(())
}

/// 删除备份
pub async fn delete_backup(instance_id: &str, world: &str, filename: &str) -> Result<(), String> {
    crate::fsutil::validate_name(world)?;
    if !crate::util::is_safe_filename(world) || !crate::util::is_safe_filename(filename) {
        return Err("非法的路径参数".into());
    }
    let inst_dir = instance_dir(instance_id).await?;
    let p = inst_dir.join(backups_dir(world)).join(filename);
    if !p.is_file() {
        return Err("备份文件不存在".into());
    }
    std::fs::remove_file(&p).map_err(|e| format!("删除失败: {e}"))
}

/// 读取 zip 中的单个条目（保留，便于后续扩展）
#[allow(dead_code)]
fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>, String> {
    let mut e = archive
        .by_name(name)
        .map_err(|e| format!("zip 内缺少 {name}: {e}"))?;
    let mut buf = Vec::new();
    Read::read_to_end(&mut e, &mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}
