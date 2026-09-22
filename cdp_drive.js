// CDP 驱动脚本：通过 WebView 调试协议在 Tauri Android 的 WebView 中执行 JS
// 用法: node cdp_drive.js "JS 表达式"
const wsUrl = process.argv[2];
const js = process.argv[3];
const timeout = Number(process.argv[4] || 8000);

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
    }, timeout);
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
    await send("Runtime.enable", {});
    const res = await send("Runtime.evaluate", {
      expression: js,
      returnByValue: true,
      awaitPromise: true,
    });
    console.log(JSON.stringify(res.result, null, 2));
  } catch (e) {
    console.error("CDP_ERROR:", e.message);
    process.exitCode = 1;
  } finally {
    ws.close();
    setTimeout(() => process.exit(), 200);
  }
};

ws.onerror = () => {
  console.error("WS_ERROR: connection failed");
  process.exit(1);
};
