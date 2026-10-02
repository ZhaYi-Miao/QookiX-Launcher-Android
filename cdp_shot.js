// CDP 截屏：Page.captureScreenshot 从 WebView 渲染器抓帧（绕过安卓 surface 合成，
// 模拟器上 screencap 经常冻结在旧帧）。用法: node cdp_shot.js <wsUrl> <out.png>
import { writeFileSync } from "node:fs";

const wsUrl = process.argv[2];
const out = process.argv[3] ?? "cdp_shot.png";
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
        reject(new Error(`timeout: ${method}`));
      }
    }, 8000);
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

ws.onopen = async () => {
  try {
    await send("Page.enable", {});
    const res = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(out, Buffer.from(res.data, "base64"));
    console.log(`saved: ${out}`);
  } catch (e) {
    console.error("failed:", e.message);
    process.exitCode = 1;
  } finally {
    ws.close();
  }
};
