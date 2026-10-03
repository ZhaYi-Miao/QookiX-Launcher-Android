// 真机深一层遍历：点进子页/子 tab 再截图 + 体检。
// 用法: node pagewalk2.cjs <ws-url> <outDir> <mode>
//   mode=settings  逐个点开设置分组
//   mode=instance  逐个点开实例详情的 tab
const fs = require("node:fs");
const path = require("node:path");

const wsUrl = process.argv[2];
const outDir = process.argv[3] || "i:/program/vibe/mc/walk";
const mode = process.argv[4] || "settings";
fs.mkdirSync(outDir, { recursive: true });

const STEPS =
  mode === "settings"
    ? ["常规", "插件", "外观", "下载", "内容服务", "游戏内", "存储", "关于"]
    : ["内容", "存档", "设置", "按键"];

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
const evaluate = (expression) => send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });

/** 与 pagewalk 相同的体检项 */
const DIAG = `(() => {
  const out = { issues: [] };
  const vw = innerWidth;
  if (document.documentElement.scrollWidth > vw + 1) {
    const wide = [...document.querySelectorAll('.page *')].filter(e => {
      const r = e.getBoundingClientRect();
      return r.width > vw + 1 && r.height > 4 && getComputedStyle(e).overflowX === 'visible';
    }).slice(0, 4).map(e => e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 26) + ' w=' + Math.round(e.getBoundingClientRect().width));
    out.issues.push({ kind: 'h-overflow', detail: document.documentElement.scrollWidth + '>' + vw, els: wide });
  }
  // 文字被容器裁掉（非省略号却溢出）：手机上最常见的「显示不全」
  const clipped = [...document.querySelectorAll('.page *')].filter(e => {
    if (e.children.length) return false;
    const cs = getComputedStyle(e);
    if (cs.textOverflow === 'ellipsis' || cs.overflow === 'visible' || cs.whiteSpace === 'normal') return false;
    return e.scrollWidth > e.clientWidth + 2;
  }).slice(0, 5).map(e => e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 24) + ' "' + (e.textContent || '').trim().slice(0, 18) + '"');
  if (clipped.length) out.issues.push({ kind: 'text-clipped', detail: clipped.length + ' 处', els: clipped });
  // 触控过小
  const small = [...document.querySelectorAll('.page button, .page a, .page [role=button]')].filter(e => {
    const r = e.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && r.height < 30;
  }).slice(0, 5).map(e => { const r = e.getBoundingClientRect(); return e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 22) + ' ' + Math.round(r.width) + 'x' + Math.round(r.height) + ' "' + (e.textContent || '').trim().slice(0, 8) + '"'; });
  if (small.length) out.issues.push({ kind: 'small-tap', detail: small.length + ' 个', els: small });
  // 6) 贴边：可见内容离左右边 < 12px（底栏/通栏容器不算）
  //    被祖先横向滚动/裁剪的要素不算 —— 它的 rect 也在视口外，但用户看不到（不是出血）
  const inScroller = (e) => {
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
    if (r.width >= vw - 2) return false;
    const leaf = e.children.length === 0;
    if (!leaf && !(e.textContent || '').trim()) return false;
    if (inScroller(e)) return false;
    return r.left < 12 || r.right > vw - 12;
  }).slice(0, 5).map(e => {
    const r = e.getBoundingClientRect();
    return e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 22) + ' l=' + Math.round(r.left) + ' r=' + Math.round(vw - r.right) + ' "' + (e.textContent || '').trim().slice(0, 10) + '"';
  });
  if (edgeEls.length) out.issues.push({ kind: 'edge-hug', detail: edgeEls.length + ' 处', els: edgeEls });
  return JSON.stringify(out);
})()`;

ws.onopen = async () => {
  let bad = 0;
  try {
    await send("Runtime.enable", {});
    await send("Page.enable", {});
    await evaluate("document.querySelectorAll('.van-overlay').forEach(o=>o.click()); 'ok'");
    // 设置分组只在 /settings 下才有。先「跳走再回来」强制重建组件 ——
    // 上一轮若停在某个子标签页，组件不会卸载，push('/settings') 仍是子页（找不到行）。
    if (mode === "settings") {
      await evaluate("(async()=>{const r=document.querySelector('#app').__vue_app__.config.globalProperties.$router; await r.push('/more'); await r.push('/settings'); return 'ok'})()");
      await sleep(1800);
    }
    for (let i = 0; i < STEPS.length; i++) {
      const label = STEPS[i];
      // 点「分组行」（设置首页）或「tab」（实例详情）；找不到就跳过
      const clicked = await evaluate(`(() => {
        const t = ${JSON.stringify(label)};
        const el = [...document.querySelectorAll('.page .van-cell, .page button, .page [role=tab], .page .tab')]
          .find(e => (e.textContent || '').trim().startsWith(t) || e.getAttribute('aria-label') === t);
        if (!el) return 'not-found';
        el.click();
        return 'ok';
      })()`);
      if (clicked.result.value !== "ok") {
        console.log("SKIP " + label + " → " + clicked.result.value);
        continue;
      }
      await sleep(2000);
      const diag = await evaluate(DIAG);
      const shot = await send("Page.captureScreenshot", { format: "png" });
      const file = path.join(outDir, mode + "-" + String(i + 1).padStart(2, "0") + "-" + label + ".png");
      fs.writeFileSync(file, Buffer.from(shot.data, "base64"));
      let d = { issues: [] };
      try { d = JSON.parse(diag.result.value); } catch { console.log("   " + label + " 体检失败（页面可能在跳转）"); }
      if (d.issues.length) bad++;
      console.log((d.issues.length ? "!! " : "   ") + label + (d.issues.length ? "  " + d.issues.map((i) => i.kind + "(" + (i.els || []).join(" ; ") + ")").join("  |  ") : ""));
      // 回到列表（设置有返回键；实例详情用 tab 直接切，不用返回）
      if (mode === "settings") {
        await evaluate("document.querySelector('.sv-back')?.click(); 'ok'");
        await sleep(1200);
      }
    }
  } catch (e) {
    console.error("WALK_ERROR:", e.message);
    process.exitCode = 1;
  } finally {
    // 有问题就返回非 0（npm run ui:audit 据此判失败）
    if (bad) process.exitCode = 1;
    ws.close();
    setTimeout(() => process.exit(), 200);
  }
};
ws.onerror = () => { console.error("WS_ERROR"); process.exit(1); };
