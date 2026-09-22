import { open as rawOpen } from "@tauri-apps/plugin-dialog";
import { api } from "../api";

type OpenOptions = Parameters<typeof rawOpen>[0];

/**
 * 安卓的 SAF 返回 `content://` URI，后端只能读普通文件路径，直接传下去必然失败。
 * 这里统一在 `open()` 之后把内容落地成真实路径（桌面端原样返回），
 * 所有需要「选文件」的地方都应该走这两个入口，不要各自处理 URI。
 *
 * 注意：这里**没有**「选目录」入口（原先的 `pickDirectory()` 已删）。安卓的
 * dialog 插件对 `directory: true` 在 mobile 分支直接返回 `Err(FolderPickerNotImplemented)`，
 * 根本没有「选到目录并拿到真实路径」的等价 API —— 需要选目录的功能在移动端只能隐藏。
 */
async function resolvePicked(path: string): Promise<string> {
  try {
    return await api.resolvePickedPath(path);
  } catch {
    // 原生桥不可用（如桌面端）时保持原值，不影响原有行为
    return path;
  }
}

/** 选择单个文件，返回可被后端直接读取的真实路径。 */
export async function pickFile(options?: OpenOptions): Promise<string | null> {
  const picked = await rawOpen(options as never);
  if (!picked) return null;
  const path = Array.isArray(picked) ? picked[0] : picked;
  if (typeof path !== "string") return null;
  return resolvePicked(path);
}

/** 选择多个文件，逐个落地为真实路径。 */
export async function pickFiles(options?: OpenOptions): Promise<string[]> {
  const picked = await rawOpen(options as never);
  if (!picked) return [];
  const list = Array.isArray(picked) ? picked : [picked];
  const out: string[] = [];
  for (const item of list) {
    if (typeof item === "string") out.push(await resolvePicked(item));
  }
  return out;
}

