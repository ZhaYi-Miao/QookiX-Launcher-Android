import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // 直接写进真正被打包的那个 Android 模块。
    // 以前指向仓库根部的 `app/`（一个已经废弃、无法独立构建的旧副本），
    // 结果 `./gradlew assembleDebug` 打出来的 APK 里只有占位页 index.html、
    // 没有任何 JS/CSS —— 手机上看到的是「正在加载…」而不是启动器界面。
    outDir: "src-tauri/gen/android/app/src/main/assets",
    // 每次构建清空，避免历史 chunk 越积越多（之前累积了 12 份同名 chunk）。
    emptyOutDir: true,
  },
}));
