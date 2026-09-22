// 逐页自适应扫描：遍历所有路由，找出「横向溢出 / 超出视口」的元素。
//
// 用法: node cdp_scan.js "ws://127.0.0.1:9222/devtools/page/XXXX" [实例id]
//
// 判定两类问题（这是 B 阶段「逐页去写死尺寸」的验收依据）：
//   1) 内容溢出：元素的内容比自身宽，且它自己不滚动（overflow 不是 auto/scroll/hidden）
//      → 内容被裁掉或把外层撑破。
//   2) 超出视口：元素的左右边界跑到视口外 → 横向滚动条/够不到。
//
// 说明：`#app` 上挂了 zoom（界面缩放），`getBoundingClientRect()` 返回的是**缩放后**
// 的视觉像素，而 innerWidth 是布局视口。界面缩放非 100% 时会出现假阳性，
// 所以报告里带上 --ui-scale，判断前先看它。
const wsUrl = process.argv[2];
const IID = process.argv[3] || "d692fff9-161a-4555-86a0-ae86de53c18f";

const ROUTES = [
  "/",
  "/news",
  "/browse",
  "/downloads",
  "/instances",
  "/create",
  "/multiplayer",
  "/settings",
  "/skins",
  `/instance/${IID}`,
  `/instance/${IID}?tab=saves`,
  `/instance/${IID}?tab=settings`,
  `/instance/${IID}?tab=mods`,
  `/instance/${IID}?tab=logs`,
];

const SCAN = `(function(){
  const ui = getComputedStyle(document.documentElement).getPropertyValue("--ui-scale").trim() || "1";
  const issues = [];
  const seen = new Set();
  const name = (el) => {
    const c = String(el.className || "").trim().split(/\\s+/).filter(Boolean).slice(0,2).join(".");
    return el.tagName.toLowerCase() + (c ? "." + c : "");
  };

  // ① 整页横向滚动 —— 这是用户真正会遇到的「页面能左右拖」。
  const docOver = document.documentElement.scrollWidth - innerWidth;
  if (docOver > 1) {
    issues.push({问题:"整页横向滚动", 多出:docOver, 文档内容宽:document.documentElement.scrollWidth});
  }

  // ② 有没有横向的裁剪祖先（有的话，超出它的内容是被有意裁掉的，不算问题）
  const clippedHorizontally = (el) => {
    for (let p = el.parentElement; p && p !== document.documentElement; p = p.parentElement) {
      const ox = getComputedStyle(p).overflowX;
      if (ox === "hidden" || ox === "clip") return true;
    }
    return false;
  };

  for (const el of document.querySelectorAll("*")) {
    if (el.id === "app" || el.classList.contains("app-bg")) continue;
    const cs = getComputedStyle(el);
    if (cs.display === "none" || cs.visibility === "hidden") continue;
    const r = el.getBoundingClientRect();
    if (r.width < 3 || r.height < 3) continue;
    const key = name(el);

    // 2a) 弹层/固定元素横向超出视口，且没有被裁剪 → 够不到
    if ((r.right > innerWidth + 3 || r.left < -3) && !clippedHorizontally(el)) {
      if (!seen.has("o:" + key)) { seen.add("o:" + key);
        issues.push({问题:"超出视口且未裁剪", 元素:key, 左:Math.round(r.left), 右:Math.round(r.right), 视口宽:innerWidth}); }
    }

    // 2b) 内容比容器宽、自己不滚动、也没有裁剪祖先 → 内容被撑破或被裁
    //
    // 跳过 button：1-11 给图标按钮加了 ::after 把命中区撑到 40/44px，
    // 伪元素不属于 querySelectorAll，但它会计入 scrollWidth →
    // 所有图标按钮都会假阳性。这是已知且有意为之的，不再报。
    if (el.tagName === "BUTTON") continue;
    const ox = cs.overflowX;
    if (el.scrollWidth > el.clientWidth + 4 && ox !== "auto" && ox !== "scroll" && ox !== "hidden"
        && !clippedHorizontally(el)) {
      if (!seen.has("w:" + key)) { seen.add("w:" + key);
        issues.push({问题:"内容溢出", 元素:key, 内容宽:el.scrollWidth, 容器宽:el.clientWidth}); }
    }
  }
  return {路径: location.pathname + location.search, 视口: innerWidth + "x" + innerHeight,
          界面缩放: ui, 问题数: issues.length, 问题: issues.slice(0, 12)};
})()`;

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

async function evaluate(expression) {
  const res = await send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (res.exceptionDetails) {
    throw new Error(res.exceptionDetails.exception?.description || "JS 异常");
  }
  return res.result.value;
}

ws.onopen = async () => {
  try {
    await send("Runtime.enable", {});
    const summary = [];
    for (const route of ROUTES) {
      await evaluate(
        `(function(){window.history.pushState({}, "", ${JSON.stringify(route)});` +
          `window.dispatchEvent(new PopStateEvent("popstate"));return 1;})()`
      );
      await sleep(3500);
      try {
        const r = await evaluate(SCAN);
        summary.push(r);
        const bad = r.问题数 > 0 ? `⚠ ${r.问题数}` : "✓";
        console.log(`${bad}  ${r.路径}  [${r.视口}] scale=${r.界面缩放}`);
        for (const p of r.问题) {
          console.log("      " + JSON.stringify(p));
        }
      } catch (e) {
        console.log(`ERR ${route}: ${e.message}`);
      }
    }
    const total = summary.reduce((a, b) => a + b.问题数, 0);
    console.log(`\n合计问题点: ${total}`);
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
