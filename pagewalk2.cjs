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
  return JSON.stringify(out);
})()`;

ws.onopen = async () => {
  try {
    await send("Runtime.enable", {});
    await send("Page.enable", {});
    await evaluate("document.querySelectorAll('.van-overlay').forEach(o=>o.click()); 'ok'");
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
      const d = JSON.parse(diag.result.value);
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
    ws.close();
    setTimeout(() => process.exit(), 200);
  }
};
ws.onerror = () => { console.error("WS_ERROR"); process.exit(1); };
