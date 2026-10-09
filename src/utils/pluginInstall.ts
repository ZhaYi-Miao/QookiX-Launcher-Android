import { openUrl } from "@tauri-apps/plugin-opener";
import { t as $t } from "../i18n";

/**
 * 「装到服务器」的返回。`installed: false` 不是失败 —— 见下面 `handleExternalPlugin`。
 */
export type PluginInstallResult = {
  installed: boolean;
  externalUrl?: string | null;
  fileName?: string;
  version?: string;
  compatVerified?: boolean;
};

/**
 * 处理「插件没托管在 Hangar」的情况：**自动打开浏览器**让用户自己下载。
 *
 * 为什么不做成报错：Hangar 上有一批插件只提供外部下载页（`externalUrl`，例如
 * EssentialsX 指向 GitHub releases），地址本来就拿到了，把它丢进一句错误信息里
 * 让用户手抄，是白白浪费一个能直接用的链接。手机上「跳浏览器 → 下载 → 回启动器」
 * 是最顺的路。
 *
 * 返回 `true` 表示「已接管，调用方不要再报成功/失败」。
 * 服务器详情页的插件页签与内容中心的安装对话框共用这一份，避免各写一遍分支。
 */
export function handleExternalPlugin(
  r: PluginInstallResult,
  notify: (msg: string) => void,
): boolean {
  if (r.installed) return false;
  const url = (r.externalUrl ?? "").trim();
  if (url) {
    // 打开失败（本机没有浏览器之类）也不额外弹错：提示里已经把话说清了
    void openUrl(url).catch(() => {});
    notify($t("plugins.external-opened"));
  } else {
    notify($t("plugins.no-download"));
  }
  return true;
}
