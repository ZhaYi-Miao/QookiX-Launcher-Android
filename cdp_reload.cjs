// 清掉 WebView 的 HTTP 缓存并硬重载。
// 背景：APK 里已经是新资源（index.html 指向新的 hash），但 WebView 命中旧缓存
// 仍在跑上一版前端 —— 改完前端只看截图会误判成「改动没生效」。
// 用法：node cdp_reload.cjs <ws-url>
const ws = process.argv[2];
if (!ws) {
  console.error("用法: node cdp_reload.cjs <ws-url>");
  process.exit(1);
}

const socket = new WebSocket(ws);
let id = 0;
const pending = new Map();

function send(method, params = {}) {
  const mid = ++id;
  socket.send(JSON.stringify({ id: mid, method, params }));
  return new Promise((resolve) => pending.set(mid, resolve));
}

socket.addEventListener("message", (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg.result ?? msg.error);
    pending.delete(msg.id);
  }
});

socket.addEventListener("open", async () => {
  await send("Network.enable");
  const cleared = await send("Network.clearBrowserCache");
  console.log("已清缓存:", JSON.stringify(cleared));
  await send("Page.enable");
  await send("Page.reload", { ignoreCache: true });
  console.log("已硬重载");
  setTimeout(() => {
    socket.close();
    process.exit(0);
  }, 1500);
});

socket.addEventListener("error", (e) => {
  console.error("WS_ERROR", e.message ?? e);
  process.exit(2);
});
