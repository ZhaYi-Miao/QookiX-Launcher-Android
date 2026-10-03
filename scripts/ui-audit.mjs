// UI 体检：真机逐页遍历（npm run ui:audit）
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
const ADB = process.env.ADB ?? "adb";
const a = process.argv.slice(2);
const dev = a.includes("--device") ? a[a.indexOf("--device") + 1] : null;
const OUT = join(process.cwd(), "..", "walk");
mkdirSync(OUT, { recursive: true });
const sh = (x) => execFileSync(ADB, x, { encoding: "utf8" }).trim();
const list = sh(["devices"]).split(/\r?\n/).slice(1).map((l) => l.trim().split(/\s+/)).filter((p) => p[1] === "device");
if (!list.length) {
  console.error("没有在线的安卓设备（adb devices 为空）");
  process.exit(1);
}
const serial = dev ?? list[0][0];
console.log("设备: " + serial);
const port = 9700 + Math.floor(Math.random() * 200);
const pid = sh(["-s", serial, "shell", "pidof", "com.zhayi.qookix"]);
if (!pid) { console.error("应用没在运行"); process.exit(1); }
sh(["-s", serial, "forward", "tcp:" + port, "localabstract:webview_devtools_remote_" + pid]);
const r = await fetch("http://127.0.0.1:" + port + "/json/list");
const pg = (await r.json()).find((t) => t.type === "page");
if (!pg) { console.error("拿不到 WebView 调试页"); process.exit(1); }
const ws = pg.webSocketDebuggerUrl;
const walk = (f, ...rest) => {
  const p = spawnSync("node", [f, ws, OUT, ...rest], { stdio: "inherit" }).status ?? 1;
  return p;
};
console.log("--- 主页面 ---");
const bad = walk("pagewalk.cjs");
console.log("--- 设置子页 ---");
const bad2 = walk("pagewalk2.cjs", "settings");
process.exit(bad || bad2 ? 1 : 0);
