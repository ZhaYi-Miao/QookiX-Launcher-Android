import { createApp } from "vue";
import { createPinia } from "pinia";
import { openUrl } from "@tauri-apps/plugin-opener";
import App from "./App.vue";
import router from "./router";
import "./styles.css";

// ── 输入法统一设置（安卓 IME 的自动大写 / 拼写替换）────────────────────
// 安卓输入法默认 `autocapitalize=sentences` 并且会做拼写替换 ——
// 输 `config.toml` 会变成 `Config.toml`、`options.txt` 会变成 `Options.txt`。
// `spellcheck=false` **管不住** IME 的自动大写，必须显式声明这三个属性。
//
// 全仓 55/56 个输入框都没声明（只有 CodeEditor 一处）。不去逐个改模板 ——
// 那样每次新增输入框都会漏 —— 而是在根上挂一次 `focusin` 委托统一设置。
// 已有显式声明的（比如 CodeEditor 的 autocapitalize="off"）不覆盖。
window.addEventListener(
  "focusin",
  (e) => {
    const el = e.target as HTMLElement | null;
    if (!el) return;
    const tag = el.tagName;
    if (tag !== "INPUT" && tag !== "TEXTAREA") return;
    if (!el.hasAttribute("autocapitalize")) el.setAttribute("autocapitalize", "off");
    if (!el.hasAttribute("autocorrect")) el.setAttribute("autocorrect", "off");
    if (!el.hasAttribute("autocomplete")) el.setAttribute("autocomplete", "off");
    if (el instanceof HTMLTextAreaElement || (el as HTMLInputElement).type === "text") {
      el.spellcheck = false;
    }
  },
  true
);

// ── 文档级滚动钳制 ────────────────────────────────────────────────────
// 应用外壳（.app）是精确满屏的，文档级滚动永远是误操作 —— 但 overflow:hidden
// 只挡用户手势，**挡不住 JS 的程序化滚动**（focus()/scrollIntoView() 都会滚它）。
// 一旦被滚十几像素，表现就是「标题栏顶出屏幕、左栏和标题栏之间出现一条缝」。
// 这里捕获阶段监听所有滚动，把文档级（html/body）的滚动位置钳回 0；
// 页面内部的滚动容器不受影响。
window.addEventListener(
  "scroll",
  () => {
    const de = document.documentElement;
    if (de.scrollTop !== 0) de.scrollTop = 0;
    if (document.body.scrollTop !== 0) document.body.scrollTop = 0;
  },
  true
);

// 屏蔽浏览器默认右键菜单 —— 但输入框/可编辑区域必须放行：
// 安卓上长按输入框弹出的「选择 / 粘贴」菜单就是由 contextmenu 触发的，
// 全局 preventDefault 会把它一起干掉（表现为编辑器里无法长按选择/粘贴）。
window.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement | null;
  if (el && (el.isContentEditable || /^(input|textarea)$/i.test(el.tagName))) return;
  e.preventDefault();
});

// 普通 <a href> 外链必须在系统浏览器打开。
// `tauri-plugin-opener` 只接管 target="_blank"（以及 Ctrl/Shift 点击）的链接，
// 其余 <a href="http…"> 会让 WebView 在**同一个页面**里导航 —— 手机上没有地址栏、
// 返回键又刚接好，界面会被顶掉且回不来。这里统一兜住。
window.addEventListener(
  "click",
  (e) => {
    const el = e.target as HTMLElement | null;
    const a = el?.closest?.("a");
    if (!a) return;
    const href = a.getAttribute("href") ?? "";
    if (!/^https?:/i.test(href)) return;
    e.preventDefault();
    openUrl(href).catch(() => {
      /* 打不开就算了，不要因为外链弹错误 */
    });
  },
  true // 捕获阶段：抢在 Vue 的 click 处理之前
);

// ── 安卓返回键 / 返回手势 ──────────────────────────────────────────────
// 原生侧（MainActivity.setupBackNavigation）会派发**可取消**的 `qk-back`：
// 这里负责「先在应用内退一层」，只有退无可退时才让原生走 WebView 历史 / finish()。
// 不接这个事件的话，任何页面按返回都会直接关掉整个启动器。
const BACK_PARENT: Record<string, string> = {
  instance: "/instances",
  create: "/instances",
  "server-detail": "/multiplayer",
};

window.addEventListener("qk-back", (e) => {
  const route = router.currentRoute.value;
  if (route.path === "/") return; // 首页没有上一层，交给原生处理
  e.preventDefault();
  router.push(BACK_PARENT[String(route.name)] ?? "/");
});

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.mount("#app");
