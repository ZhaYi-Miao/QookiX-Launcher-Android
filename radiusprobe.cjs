// 量一遍：每个页面里「 sizeable 容器但圆角 < 8px」的元素（= 视觉上的直角块）
const ws = new WebSocket(process.argv[2]);
let id = 0;
const pend = new Map();
const send = (m, p) => new Promise((res, rej) => {
  const i = ++id; pend.set(i, { res, rej });
  ws.send(JSON.stringify({ id: i, method: m, params: p }));
  setTimeout(() => { if (pend.has(i)) { pend.delete(i); rej(new Error("to")); } }, 15000);
});
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pend.has(m.id)) { const p = pend.get(m.id); pend.delete(m.id); m.error ? p.rej(new Error(m.error.message)) : p.res(m.result); }
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const ev = (e) => send("Runtime.evaluate", { expression: e, returnByValue: true, awaitPromise: true });
const PAGES = ["/", "/instances", "/browse", "/downloads", "/more", "/multiplayer", "/skins", "/news", "/settings", "/create"];
const PROBE = `(() => {
  const out = [];
  for (const e of document.querySelectorAll('.page *, .van-tabbar *')) {
    const r = e.getBoundingClientRect();
    if (r.width < 90 || r.height < 24) continue;
    const cs = getComputedStyle(e);
    if (cs.display === 'none' || cs.visibility === 'hidden') continue;
    const rad = parseFloat(cs.borderTopLeftRadius) || 0;
    if (rad >= 8) continue;
    if (e.children.length > 6) continue;
    out.push({ cls: e.tagName.toLowerCase() + '.' + String(e.className).slice(0, 26), rad, w: Math.round(r.width), h: Math.round(r.height) });
  }
  const seen = new Set();
  const uniq = out.filter(o => { const k = o.cls + o.rad; if (seen.has(k)) return false; seen.add(k); return true; });
  return JSON.stringify(uniq.slice(0, 8));
})()`;
ws.onopen = async () => {
  await send("Runtime.enable", {});
  for (const p of PAGES) {
    await ev(`(async()=>{const r=document.querySelector('#app').__vue_app__.config.globalProperties.$router; await r.push('${p}'); return 1})()`);
    await sleep(1700);
    const d = await ev(PROBE);
    let arr = []; try { arr = JSON.parse(d.result.value); } catch {}
    console.log((arr.length ? "!! " : "   ") + p + (arr.length ? "  " + arr.map((x) => `${x.cls} r=${x.rad} ${x.w}x${x.h}`).join(" | ") : ""));
  }
  ws.close();
  setTimeout(() => process.exit(), 200);
};
