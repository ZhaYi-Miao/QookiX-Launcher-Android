/**
 * 构建前清掉上一次的前端产物。
 *
 * ## 为什么需要这个脚本
 *
 * `vite.config.ts` 把 `outDir` 指到 `src-tauri/gen/android/app/src/main/assets` —— 这个目录在
 * **vite 项目根之外**，于是 `emptyOutDir: true` 只会打印一条警告、**拒绝清空** ✗
 * （2026-10-05 实测：那里累积了 6 份历史 `index-*.js` ✗，注释里"每次构建清空"是假的 ✓）。
 *
 * 后果不只是占体积：APK 里会同时存在**新的 `index.html`** 与**旧 chunk** ✗，
 * 而 WebView 会按 URL 缓存 `index.html`（跨版本同名 ✗）→ 拿到旧 HTML → 引用旧 chunk
 * → **用户装完新版看到的还是旧界面** ✗（真机实测 ✓）。
 *
 * 所以每次构建前把 `assets/` 子目录整个删掉（保留旁边的 `app-icon.png` ✓ 那是手工放的文件 ✓），
 * 让包里只剩这一次的产物。
 */
import { rmSync, existsSync } from "node:fs";

const OUT = "src-tauri/gen/android/app/src/main/assets/assets";

if (existsSync(OUT)) {
  rmSync(OUT, { recursive: true, force: true });
  console.log(`[clean-web-assets] 已清空 ${OUT}`);
} else {
  console.log(`[clean-web-assets] ${OUT} 不存在，跳过`);
}
