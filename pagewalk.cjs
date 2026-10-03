// 真机页面遍历：按路由逐页导航 → 截图 + 自动体检，输出 JSON 报告。
//
// 用法: node pagewalk.cjs <ws-url> <outDir>
// 导航走应用自己的 router（#app.__vue_app__ 的 $router），和点底栏是同一条链路，
// 所以看到的 就是用户会看到的。
const fs = require("node:fs");
const path = require("node:path");

const wsUrl = process.argv[2];
const outDir = process.argv[3] || "i:/program/vibe/mc/walk";
fs.mkdirSync(outDir, { recursive: true });

const PAGES = [
  { name: "01-home", to: "/" },
  { name: "02-instances", to: "/instances" },
  { name: "03-browse", to: "/browse" },
  { name: "04-downloads", to: "/downloads" },
  { name: "05-more", to: "/more" },
  { name: "06-multiplayer", to: "/multiplayer" },
  { name: "07-skins", to: "/skins" },
  { name: "08-news", to: "/news" },
  { name: "09-settings", to: "/settings" },
  { name: "10-create", to: "/create" },
];

const ws = new WebSocket(wsUrl);
let id = 0;
const pending = new Map();

function send(method, params) {
  return new Promise((resolve, reject) => {
    const msgId = ++id;
    pending.set(msgId, { resolve, reject });
    ws.send(JSON.stringify({ id: msgId, method, params }));
    setTimeout(() => {
      if (pending.has(msgId)) {
        pending.delete(msgId);
        reject(new Error("timeout: " + method));
      }
    }, 20000);
  });
}

ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const p = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.error) p.reject(new Error(msg.error.message));
    else p.resolve(msg.result);
  }
};

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** 页内体检：横向溢出 / 触控过小的可见可点元素 / 未翻译 key / 空值文案 / 元素重叠页脚 */
const DIAG = `(async () => {
  const out = { path: location.pathname, errs: (window.__errs||[]).slice(-4), issues: [] };
  window.__errs = [];
  const vw = innerWidth;
  // 1) 横向溢出（手机上最常见：某个元素把页面撑宽，右侧被切）
  if (document.documentElement.scrollWidth > vw + 1) {
    const wide = [...document.querySelectorAll('body *')].filter(e => {
      const r = e.getBoundingClientRect();
      return r.width > vw + 1 && r.height > 4 && getComputedStyle(e).overflowX !== 'auto' && getComputedStyle(e).overflowX !== 'scroll';
    }).slice(0, 4).map(e => e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 30) + ' w=' + Math.round(e.getBoundingClientRect().width));
    out.issues.push({ kind: 'h-overflow', detail: document.documentElement.scrollWidth + '>' + vw, els: wide });
  }
  // 2) 触控目标过小（可见且可点，< 32px）
  const small = [...document.querySelectorAll('button, a, [role=button], .van-tabbar-item, input[type=checkbox]')].filter(e => {
    const r = e.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return false;
    if (getComputedStyle(e).display === 'none') return false;
    return r.height < 32 || r.width < 28;
  }).slice(0, 6).map(e => {
    const r = e.getBoundingClientRect();
    return e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 24) + ' ' + Math.round(r.width) + 'x' + Math.round(r.height) + ' "' + (e.textContent || '').trim().slice(0, 10) + '"';
  });
  if (small.length) out.issues.push({ kind: 'small-tap', detail: small.length + ' 个', els: small });
  // 3) 漏翻译：可见文本里出现 i18n key 形状（xxx.yyy）。排除版本号（1.21.1 / 26.3 这类纯数字加点）
  const isKey = (t) => /^[a-z][a-z0-9-]*(\\.[a-z0-9-]+){1,4}$/.test(t);
  const raw = [...document.querySelectorAll('body *')].filter(e => e.children.length === 0).map(e => (e.textContent || '').trim()).filter(t => isKey(t)).slice(0, 5);
  if (raw.length) out.issues.push({ kind: 'raw-i18n', detail: raw.join(' | ') });
  // 4) 空值文案（undefined/null/NaN/[object Object] 直接显示出来）
  const bad = [...document.querySelectorAll('body *')].filter(e => e.children.length === 0).map(e => (e.textContent || '').trim()).filter(t => /^(undefined|null|NaN|\\[object Object\\])$/.test(t)).slice(0, 5);
  if (bad.length) out.issues.push({ kind: 'empty-value', detail: bad.join(' | ') });
  // 5) 内容被底栏盖住（末元素底部超出可视区且没有 padding 预留）
  const page = document.querySelector('.page');
  if (page) {
    const pr = page.getBoundingClientRect();
    const last = page.lastElementChild;
    if (last) {
      const lr = last.getBoundingClientRect();
      if (lr.bottom > pr.bottom + 2) out.issues.push({ kind: 'under-tabbar', detail: '末元素超出 ' + Math.round(lr.bottom - pr.bottom) + 'px' });
    }
  }
  // 6) 贴边：可见内容离屏幕左右边 < 12px（手机上会显得"顶着屏边"）
  const clipped = (e) => {
    for (let p = e.parentElement; p && p !== document.body; p = p.parentElement) {
      const ox = getComputedStyle(p).overflowX;
      if (ox === 'auto' || ox === 'scroll' || ox === 'hidden') return true;
    }
    return false;
  };
  const edgeEls = [...document.querySelectorAll('.page *')].filter(e => {
    const r = e.getBoundingClientRect();
    const cs = getComputedStyle(e);
    if (cs.display === 'none' || cs.visibility === 'hidden') return false;
    if (r.width < 8 || r.height < 8) return false;
    if (r.width >= vw - 2) return false;           // 通栏容器（背景/分组）不算
    const leaf = e.children.length === 0;
    if (!leaf && !(e.textContent || '').trim()) return false;
    if (clipped(e)) return false;   // 被横向滚动容器裁掉的不可见，不算出血
    return r.left < 12 || r.right > vw - 12;
  }).slice(0, 5).map(e => {
    const r = e.getBoundingClientRect();
    return e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 22) + ' l=' + Math.round(r.left) + ' r=' + Math.round(vw - r.right) + ' "' + (e.textContent || '').trim().slice(0, 10) + '"';
  });
  if (edgeEls.length) out.issues.push({ kind: 'edge-hug', detail: edgeEls.length + ' 处', els: edgeEls });
  out.tabs = [...document.querySelectorAll('.van-tabbar-item')].map(e => (e.className.includes('--active') ? '*' : '') + e.textContent.trim()).join(',');
  out.title = (document.querySelector('.top .title') || {}).textContent || '';
  return JSON.stringify(out);
})()`;

ws.onopen = async () => {
  const report = [];
  try {
    await send("Runtime.enable", {});
    await send("Page.enable", {});
    // 全程收集运行时错误
    await send("Runtime.evaluate", {
      expression: `window.__errs=[];addEventListener('error',e=>__errs.push('err:'+e.message));addEventListener('unhandledrejection',e=>__errs.push('rej:'+e.reason));
        // 关掉可能开着的弹层（账号面板等），否则遮罩会把每个页面都拍灰
        document.querySelectorAll('.van-overlay').forEach(o=>o.click());
        'ok'`,
      returnByValue: true,
    });
    await sleep(700);

    for (const p of PAGES) {
      const nav = await send("Runtime.evaluate", {
        expression: `(async()=>{const app=document.querySelector('#app'); const r=app&&app.__vue_app__&&app.__vue_app__.config.globalProperties.$router; if(!r) return 'no-router'; await r.push('${p.to}'); return 'ok'})()`,
        returnByValue: true,
        awaitPromise: true,
      });
      await sleep(2200);
      const diag = await send("Runtime.evaluate", { expression: DIAG, returnByValue: true, awaitPromise: true });
      const shot = await send("Page.captureScreenshot", { format: "png" });
      const file = path.join(outDir, p.name + ".png");
      fs.writeFileSync(file, Buffer.from(shot.data, "base64"));
      let parsed = {};
      try { parsed = JSON.parse(diag.result.value); } catch (e) { parsed = { parseError: String(diag.result && diag.result.value) }; }
      report.push({ page: p.name, nav: nav.result && nav.result.value, ...parsed });
      console.log("WALKED " + p.name + (nav.result && nav.result.value !== "ok" ? " NAV:" + nav.result.value : ""));
    }
    fs.writeFileSync(path.join(outDir, "report.json"), JSON.stringify(report, null, 2));
    console.log("\n=== 体检结果 ===");
    let bad = 0;
    for (const r of report) {
      const iss = (r.issues || []).map((i) => i.kind + "(" + i.detail + ")");
      if (iss.length || (r.errs || []).length) bad++;
      console.log((iss.length ? "!! " : "   ") + r.page + "  " + r.title + "  " + (r.tabs || "") + (iss.length ? "\n      " + iss.join("\n      ") : ""));
      if (r.errs && r.errs.length) console.log("      运行错误: " + r.errs.join(" ; "));
    }
    // 有问题就返回非 0，便于 npm run ui:audit / CI 直接看出失败
    if (bad) process.exitCode = 1;
  } catch (e) {
    console.error("WALK_ERROR:", e.message);
    process.exitCode = 1;
  } finally {
    ws.close();
    setTimeout(() => process.exit(), 200);
  }
};

ws.onerror = () => { console.error("WS_ERROR"); process.exit(1); };
