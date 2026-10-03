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

// 设置页里面的各个 tab（走 /settings 时只会渲染默认那个 tab，其它 tab 的内容
// 以前扫不到 —— 「关于」页的卡片重叠就是这么漏网的）。这里逐个点开扫一遍。
const SETTINGS_TABS = ["常规", "插件", "外观", "下载", "内容服务", "游戏内", "存储", "关于"];

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
  // ③ 元素重叠：文字块 / 交互控件之间相交。
  //
  // 横向扫描抓不到这类问题 —— 典型成因是「UI 库组件高度写死 + 我们允许它换行」，
  // 第二行就会**压住下面已经排好的文字**（实例设置页的 naive-ui 按钮组踩过：
  // 「自定义」折行叠在 2048 MB 上）。所以这里单独扫一遍。
  const intersects = (a, b) => {
    const x = Math.min(a.right, b.right) - Math.max(a.left, b.left);
    const y = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
    return x > 4 && y > 4 ? x * y : 0;
  };
  const hasText = (el) => {
    for (const n of el.childNodes) {
      if (n.nodeType === 3 && n.nodeValue && n.nodeValue.trim()) return true;
    }
    return false;
  };
  const isControl = (el) => /^(BUTTON|INPUT|SELECT|TEXTAREA)$/.test(el.tagName);
  // 固定定位的层（底部导航、启动浮条、弹窗）**本来就是浮在内容上的**，不算重叠。
  const inFixed = (el) => {
    for (let p = el; p && p !== document.documentElement; p = p.parentElement) {
      if (getComputedStyle(p).position === "fixed") return true;
    }
    return false;
  };

  // 元素**实际可见的矩形**：按所有祖先的裁剪框（overflow hidden/clip/auto/scroll）裁一刀。
  // 不裁的话，被 overflow:hidden 裁掉的内部控件（例如 22px 圆点里塞的 naive 取色器）
  // 会拿自己完整矩形的坐标去跟邻居比，产生假阳性；滚出可视区的列表行同理。
  const visibleRect = (el, r) => {
    let top = r.top, left = r.left, right = r.right, bottom = r.bottom;
    for (let p = el.parentElement; p && p !== document.documentElement; p = p.parentElement) {
      const cs = getComputedStyle(p);
      if (!/(auto|scroll|hidden|clip)/.test(cs.overflowX + cs.overflowY)) continue;
      const pr = p.getBoundingClientRect();
      left = Math.max(left, pr.left);
      right = Math.min(right, pr.right);
      top = Math.max(top, pr.top);
      bottom = Math.min(bottom, pr.bottom);
    }
    return {
      left, top, right, bottom,
      width: Math.max(0, right - left),
      height: Math.max(0, bottom - top),
    };
  };

  // naive-ui 的输入框：真实 input 与它自己的 placeholder 层必然重叠（一个透明压着另一个），
  // 这是 UI 库的正常结构，不算问题。
  const sameControl = (a, b) =>
    !!a.closest(".n-input,.n-base-selection") && a.closest(".n-input,.n-base-selection") ===
      b.closest(".n-input,.n-base-selection");

  const pieces = [];
  for (const el of document.querySelectorAll("*")) {
    if (el.id === "app" || el.classList.contains("app-bg")) continue;
    const cs = getComputedStyle(el);
    if (cs.display === "none" || cs.visibility === "hidden" || Number(cs.opacity) < 0.1) continue;
    if (!hasText(el) && !isControl(el)) continue;
    const r = el.getBoundingClientRect();
    // 小于 6px 的多半是隐藏的 input / 装饰，忽略
    if (r.width < 6 || r.height < 6) continue;
    if (inFixed(el)) continue;
    const v = visibleRect(el, r);
    if (v.width < 6 || v.height < 6) continue; // 已经被裁没了 / 滚出可视区
    pieces.push({ el, r, v, key: name(el) });
  }
  for (let i = 0; i < pieces.length; i++) {
    for (let j = i + 1; j < pieces.length; j++) {
      const a = pieces[i], b = pieces[j];
      if (a.el.contains(b.el) || b.el.contains(a.el)) continue;
      if (sameControl(a.el, b.el)) continue;
      const area = intersects(a.v, b.v);
      if (!area) continue;
      const smaller = Math.min(a.v.width * a.v.height, b.v.width * b.v.height);
      if (area < smaller * 0.2) continue; // 轻微擦边不算
      const k = "v:" + a.key + "|" + b.key;
      if (seen.has(k)) continue;
      seen.add(k);
      issues.push({问题:"元素重叠", A:a.key, B:b.key,
        A文本:(a.el.textContent||"").trim().slice(0,14), B文本:(b.el.textContent||"").trim().slice(0,14),
        坐标:[Math.round(Math.max(a.v.left, b.v.left)), Math.round(Math.max(a.v.top, b.v.top))],
        占较小者: Math.round(area / smaller * 100) + "%"});
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

async function goto(path) {
  await evaluate(
    `(function(){window.history.pushState({}, "", ${JSON.stringify(path)});` +
      `window.dispatchEvent(new PopStateEvent("popstate"));return 1;})()`
  );
}

async function report(label, summary) {
  try {
    const r = await evaluate(SCAN);
    summary.push(r);
    const bad = r.问题数 > 0 ? `⚠ ${r.问题数}` : "✓";
    console.log(`${bad}  ${label}  [${r.视口}] scale=${r.界面缩放}`);
    for (const p of r.问题) {
      console.log("      " + JSON.stringify(p));
    }
  } catch (e) {
    console.log(`ERR ${label}: ${e.message}`);
  }
}

ws.onopen = async () => {
  try {
    await send("Runtime.enable", {});
    const summary = [];
    for (const route of ROUTES) {
      await goto(route);
      await sleep(3500);
      await report(route, summary);
    }
    for (const tab of SETTINGS_TABS) {
      await goto("/settings");
      await sleep(1500);
      // 设置页在手机化后是「分组列表 + 页内子页」：一级是 van-cell 行，点击行切到子页。
      // 注意：同一路由重复 goto 不会重挂载组件，所以上一轮的子页状态还在 ——
      // 先按子页里的「取消」回到分组列表，才能找到下一项。
      await evaluate(
        `(function(){` +
          // 返回条的选择器随设置页形态变过两次，这里两种都认：
          // .sv-back = 分组列表页的返回条；.back .van-button = 旧骨架里的「取消」
          `const b=document.querySelector(".sv-back")||document.querySelector(".back .van-button");` +
          `if(b){b.click(); return "reset";} return "list";})()`
      );
      await sleep(900);
      const clicked = await evaluate(
        `(function(){const cells=[...document.querySelectorAll(".van-cell")];` +
          `const el=cells.find(e=>e.textContent.trim()===${JSON.stringify(tab)});` +
          `if(!el) return cells.length ? "no-tab" : "no-cell"; el.click(); return "ok";})()`
      );
      if (clicked !== "ok") {
        console.log(`跳过设置页「${tab}」：${clicked}`);
        continue;
      }
      await sleep(1600);
      await report(`/settings · ${tab}`, summary);
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
