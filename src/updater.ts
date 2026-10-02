/**
 * 启动器自更新（安卓）：查 GitHub Release → 下 APK → 交给系统安装器。
 *
 * 与桌面端一致的地方：启动静默检查、可忽略某版本、下完不自动装。
 * 不同的地方：安卓装包必须走系统确认框，所以最后一步是「安装」按钮。
 */
import { t as $t } from "./i18n";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppUpdateInfo } from "./types";

export const updateInfo = ref<AppUpdateInfo | null>(null);
export const updateChecking = ref(false);
export const updateDownloading = ref(false);
export const updateError = ref<string | null>(null);
/** 已下载好的安装包路径；有值就说明可以装了 */
export const updatePackage = ref<string | null>(null);
export const updateProgress = ref<{ downloaded: number; total: number } | null>(null);

const LS_DISMISSED = "update_dismissed_version";
const LS_AUTO = "update_auto_check";
/** 下载进度事件名（Rust 侧 updater.rs 里同名） */
const PROGRESS_EVENT = "app-update-progress";

export function isAutoCheckEnabled(): boolean {
  return localStorage.getItem(LS_AUTO) !== "0";
}

export function setAutoCheck(enabled: boolean) {
  localStorage.setItem(LS_AUTO, enabled ? "1" : "0");
}

export function dismissedVersion(): string | null {
  return localStorage.getItem(LS_DISMISSED);
}

export function dismissVersion(version: string) {
  localStorage.setItem(LS_DISMISSED, version);
}

export function clearDismissed() {
  localStorage.removeItem(LS_DISMISSED);
  updateInfo.value = null;
}

/** 检查更新。silent=true 时（启动自动检查）失败不打扰用户。 */
export async function checkUpdate(silent = false): Promise<AppUpdateInfo | null> {
  updateChecking.value = true;
  updateError.value = null;
  try {
    const info = await invoke<AppUpdateInfo>("check_for_update");
    updateInfo.value = info;
    updatePackage.value = info.downloadedPath ?? null;
    return info;
  } catch (e) {
    if (!silent) updateError.value = String(e);
    return null;
  } finally {
    updateChecking.value = false;
  }
}

let progressUnlisten: (() => void) | null = null;

export async function downloadUpdate(): Promise<string | null> {
  if (updateDownloading.value) return null;
  updateDownloading.value = true;
  updateError.value = null;
  updateProgress.value = null;
  try {
    if (!progressUnlisten) {
      progressUnlisten = await listen<{ downloaded: number; total: number }>(
        PROGRESS_EVENT,
        (e) => (updateProgress.value = e.payload)
      );
    }
    const path = await invoke<string>("download_update");
    updatePackage.value = path;
    return path;
  } catch (e) {
    updateError.value = String(e);
    return null;
  } finally {
    updateDownloading.value = false;
    updateProgress.value = null;
  }
}

/** 交给系统安装器（会弹确认框，装完由系统重启应用）。 */
export async function installUpdate(): Promise<void> {
  const path = updatePackage.value ?? updateInfo.value?.downloadedPath;
  if (!path) {
    updateError.value = $t("updater.no-apk");
    return;
  }
  try {
    await invoke("install_update", { path });
  } catch (e) {
    updateError.value = String(e);
  }
}
