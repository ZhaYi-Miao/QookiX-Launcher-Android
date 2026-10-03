/**
 * 顶部加载条（自研，替代 naive 的 `useLoadingBar`）。
 *
 * 为什么自研：naive 的加载条必须挂在 provider 上、并经由 Bridge 组件把 API 偷出来给
 * 非组件代码（api.ts）用；而它要的东西很少 —— 视口顶部一条 2px 的进度条 + 完成/失败两态。
 * 这里直接用 DOM 做，模块级即可持有，api.ts 的调用一行不用改（trackStart/trackEnd/trackError 同名）。
 *
 * 视觉上遵循本项目约定：颜色取 `--accent`（跟随主题），失败转红并短暂停留后收起。
 * 挂载点在 `#app` 内（`position: fixed` 在 zoom 子树里会自动跟随界面缩放）。
 */

const BAR_ID = "qk-loading-bar";
let pending = 0;
let hasError = false;
let timer: number | null = null;

function el(): HTMLDivElement {
  let node = document.getElementById(BAR_ID) as HTMLDivElement | null;
  if (!node) {
    node = document.createElement("div");
    node.id = BAR_ID;
    node.innerHTML = '<i class="qk-lb-fill"></i>';
    const app = document.getElementById("app") ?? document.body;
    app.appendChild(node);
  }
  return node;
}

function fill(): HTMLElement {
  return el().querySelector(".qk-lb-fill") as HTMLElement;
}

function clearTimer() {
  if (timer !== null) {
    window.clearTimeout(timer);
    timer = null;
  }
}

export function trackStart() {
  if (pending === 0) {
    hasError = false;
    clearTimer();
    const node = el();
    const bar = fill();
    bar.style.transition = "none";
    bar.style.width = "0%";
    bar.style.background = "var(--accent)";
    node.style.opacity = "1";
    // 强制重排后再启动过渡，否则宽度从 0 开始的那一帧会被合并掉
    void bar.offsetWidth;
    bar.style.transition = "width 8s cubic-bezier(0.1, 0.6, 0.2, 1)";
    bar.style.width = "82%";
  }
  pending++;
}

export function trackEnd() {
  pending = Math.max(0, pending - 1);
  if (pending > 0) return;
  const node = el();
  const bar = fill();
  if (hasError) {
    bar.style.transition = "width 0.15s ease";
    bar.style.background = "#e5534b";
    bar.style.width = "100%";
    timer = window.setTimeout(() => {
      node.style.opacity = "0";
      bar.style.width = "0%";
    }, 600);
  } else {
    bar.style.transition = "width 0.18s ease";
    bar.style.width = "100%";
    timer = window.setTimeout(() => {
      node.style.opacity = "0";
      bar.style.width = "0%";
    }, 220);
  }
  hasError = false;
}

export function trackError() {
  hasError = true;
}

/** 兼容旧调用点（原先把 naive 的 API 存到这里）；现在加载条是自管的，什么都不用做。 */
export function setLoadingBarApi(_api: unknown) {
  void _api;
}
