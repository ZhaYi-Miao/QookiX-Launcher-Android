// 全面逐页审计：JS 错误 / UI 库空壳 / 图片失败 / 原生控件残留 / 横向溢出 / 纵向裁切。
// 用法: node cdp_audit.js "ws://127.0.0.1:9222/devtools/page/XXXX" [实例id]
const wsUrl = process.argv[2];
const IID = process.argv[3] || "f5f26c33-8c06-43f1-b431-0779a4514d3d";

const ROUTES = [
  "/", "/news", "/browse", "/downloads", "/instances", "/create",
  "/multiplayer", "/settings", "/skins",
  `/instance/${IID}`, `/instance/${IID}?tab=saves`, `/instance/${IID}?tab=settings`,
  `/instance/${IID}?tab=mods`, `/instance/${IID}?tab=logs`,
];

// 挂错误收集器（只需一次，路由切换不卸载）
const INSTALL = `(function(){
  if (window.__auditInstalled) return "already";
  window.__auditInstalled = true;
  window.__auditErrs = [];
  window.addEventListener("error", (e) => window.__auditErrs.push(String(e.message).slice(0,160)));
  window.addEventListener("unhandledrejection", (e) => window.__auditErrs.push("rejection: " + String(e.reason).slice(0,160)));
  return "installed";
})()`;

const SCAN = `(function(){
  const ui = getComputedStyle(document.documentElement).getPropertyValue("--ui-scale").trim() || "1";
  const issues = [];
  const seen = new Set();
  const name = (el) => {
    const c = String(el.className || "").trim().split(/\\s+/).filter(Boolean).slice(0,2).join(".");
    return el.tagName.toLowerCase() + (c ? "." + c : "");
  };
  const clippedX = (el) => {
    for (let p = el.parentElement; p && p !== document.documentElement; p = p.parentElement) {
      const ox = getComputedStyle(p).overflowX;
      if (ox === "hidden" || ox === "clip") return true;
    }
    return false;
  };
  const clippedY = (el) => {
    for (let p = el.parentElement; p && p !== document.documentElement; p = p.parentElement) {
      const oy = getComputedStyle(p).overflowY;
      if (oy === "auto" || oy === "scroll") return true; // 有滚动祖先兜底
    }
    return false;
  };

  // ① JS 错误（本页期间累计）
  const errs = (window.__auditErrs || []).slice(0, 5);
  window.__auditErrs = [];

  // ② UI 库空壳：tagName n-* 但没有任何 n- 类 → 未注册组件渲染成空标签
  const shells = [];
  for (const el of document.querySelectorAll("*")) {
    const t = el.tagName.toLowerCase();
    if (!t.startsWith("n-")) continue;
    const cls = String(el.className || "");
    if (!cls.includes("n-")) {
      const k = "shell:" + t;
      if (!seen.has(k)) { seen.add(k); shells.push(t + (cls ? "." + cls.split(/\\s+/)[0] : "")); }
    }
  }

  // ③ 图片加载失败
  const badImgs = [];
  for (const img of document.images) {
    if (img.complete && img.naturalWidth === 0 && img.src && !img.src.startsWith("data:")) {
      const k = "img:" + name(img);
      if (!seen.has(k)) { seen.add(k); badImgs.push((img.src || "").split("/").pop().slice(0, 50)); }
    }
  }

  // ④ 原生表单控件绕过 UI 库
  const rawInputs = [];
  for (const el of document.querySelectorAll("input,textarea,select")) {
    if (!el.closest('[class*="n-"]')) {
      const k = "raw:" + name(el);
      if (!seen.has(k)) { seen.add(k); rawInputs.push(name(el)); }
    }
  }

  // ⑤ 横向问题（同 cdp_scan）
  let docOver = document.documentElement.scrollWidth - innerWidth;
  const hOver = [];
  for (const el of document.querySelectorAll("*")) {
    if (el.id === "app" || el.classList.contains("app-bg")) continue;
    const cs = getComputedStyle(el);
    if (cs.display === "none" || cs.visibility === "hidden") continue;
    const r = el.getBoundingClientRect();
    if (r.width < 3 || r.height < 3) continue;
    const key = name(el);
    if ((r.right > innerWidth + 3 || r.left < -3) && !clippedX(el)) {
      if (!seen.has("o:" + key)) { seen.add("o:" + key); hOver.push("超视口:" + key); }
    }
    if (el.tagName === "BUTTON") continue;
    if (el.scrollWidth > el.clientWidth + 4 && cs.overflowX !== "auto" && cs.overflowX !== "scroll"
        && cs.overflowX !== "hidden" && !clippedX(el)) {
      if (!seen.has("w:" + key)) { seen.add("w:" + key); hOver.push("内容溢出:" + key + " " + el.scrollWidth + ">" + el.clientWidth); }
    }
  }

  // ⑥ 纵向裁切：内容比容器高、自己不滚、无滚动祖先、且容器 overflowY hidden
  //    （故意省略号的 text-ellipsis 单行元素天然满足，跳过）
  const vClip = [];
  for (const el of document.querySelectorAll("*")) {
    if (el.id === "app") continue;
    const cs = getComputedStyle(el);
    if (cs.display === "none" || cs.visibility === "hidden") continue;
    if (cs.overflowY !== "hidden" && cs.overflowY !== "clip") continue;
    const cls = String(el.className || "");
    if (/ellipsis/.test(cls)) continue;
    if (el.scrollHeight > el.clientHeight + 6 && !clippedY(el)) {
      // 只报「 visibly 有文字内容」的容器，装饰性空容器不报
      if (!el.textContent.trim()) continue;
      const k = "v:" + name(el);
      if (!seen.has(k)) { seen.add(k); vClip.push(name(el) + " " + el.scrollHeight + ">" + el.clientHeight); }
    }
  }

  return {路径: location.pathname + location.search, 视口: innerWidth + "x" + innerHeight, 缩放: ui,
    js错误: errs, ui空壳: shells, 坏图: badImgs, 原生控件: rawInputs,
    横向: hOver.slice(0, 6), 纵向裁切: vClip.slice(0, 6)};
})()`;

const ws = new WebSocket(wsUrl);
let id = 0;
const pending = new Map();
function send(method, params) {
  return new Promise((resolve, reject) => {
    const msgId = ++id;
    pending.set(msgId, { resolve, reject });
    ws.send(JSON.stringify({ id: msgId, method, params }));
    setTimeout(() => { if (pending.has(msgId)) { pending.delete(msgId); reject(new Error("timeout")); } }, 25000);
  });
}
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const p = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.error) p.reject(new Error(msg.error.message)); else p.resolve(msg.result);
  }
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function evaluate(expression) {
  const res = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (res.exceptionDetails) throw new Error(res.exceptionDetails.exception?.description || "JS 异常");
  return res.result.value;
}
ws.onopen = async () => {
  try {
    await send("Runtime.enable", {});
    console.log("错误收集器: " + (await evaluate(INSTALL)));
    let total = 0;
    for (const route of ROUTES) {
      await evaluate(`(function(){window.history.pushState({}, "", ${JSON.stringify(route)});window.dispatchEvent(new PopStateEvent("popstate"));return 1;})()`);
      await sleep(3200);
      try {
        const r = await evaluate(SCAN);
        const n = r.js错误.length + r.ui空壳.length + r.坏图.length + r.原生控件.length + r.横向.length + r.纵向裁切.length;
        total += n;
        console.log(`${n ? "⚠ " + n : "✓"}  ${r.路径} [${r.视口}] scale=${r.缩放}`);
        const dump = (label, arr) => { if (arr.length) console.log("      " + label + ": " + JSON.stringify(arr)); };
        dump("js错误", r.js错误); dump("ui空壳", r.ui空壳); dump("坏图", r.坏图);
        dump("原生控件", r.原生控件); dump("横向", r.横向); dump("纵向裁切", r.纵向裁切);
      } catch (e) { console.log(`ERR ${route}: ${e.message.slice(0, 120)}`); }
    }
    console.log(`\n合计问题点: ${total}`);
  } catch (e) { console.error("CDP_ERROR:", e.message); process.exitCode = 1; }
  finally { ws.close(); setTimeout(() => process.exit(), 200); }
};
ws.onerror = () => { console.error("WS_ERROR: connection failed"); process.exit(1); };
