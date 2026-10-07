/**
 * 日志字号缩放 —— 实例日志页与服务器日志/控制台**共用**这一套。
 *
 * 为什么要抽出来：这两处是**两份独立实现**（`LogViewer.vue` 自己一套，
 * `ServerDetailView.vue` 又手写了一对 `<pre>`），缩放这种交互很容易只改一处、
 * 然后用户在另一个页面发现「怎么这里不能缩放」。所以捏合手势、音量键、
 * 上下限、localStorage 记忆全部收在这里，加新的日志界面时直接 `useLogZoom()`。
 *
 * 三种入口，同一套逻辑：
 *   ① 双指捏合（`touch` 三个处理函数直接绑到滚动容器上）
 *   ② 音量- / 音量+（原生 `MainActivity.onKeyDown` 派发 `qk-log-zoom` 事件 ——
 *      Android 音量键**不会**变成 WebView 的 keydown，前端监听 keydown 收不到）
 *   ③ 调用方自己放 A− / A+ 按钮调`zoomBy(±1)`
 *
 * 上下限 9–26px：小于 9px 在高 DPI 屏上看不清堆栈名，大于 26px 一行放不下几个字。
 */
import { onBeforeUnmount, onMounted, ref } from "vue";
import { api } from "../api";

const FONT_MIN = 9;
const FONT_MAX = 26;
const FONT_STEP = 2;
/** 与实例日志页共用同一个键：用户在实例那边调好的字号，服务器这边也认 */
const FONT_KEY = "qookix.log.fontSize";

function loadFontSize(): number {
  try {
    const raw = Number(localStorage.getItem(FONT_KEY));
    if (raw >= FONT_MIN && raw <= FONT_MAX) return raw;
  } catch {
    // localStorage 不可用（隐私模式等）就用默认值
  }
  return 12;
}

/**
 * 模块级共享状态：字号和 localStorage 键共用一份，这样两个日志界面之间
 * 切来切去字号是一致的（也避免了「谁最后卸载谁把字号写回去」的竞态）。
 */
const fontSize = ref<number>(loadFontSize());

/**
 * 有多少个界面正在用音量键缩放。
 *
 * 音量键捕获是**全局开关**（原生那边只有一个 `logZoomCapture` 标志），所以必须
 * 引用计数：两个日志界面若同时存在，只在第一个挂载时开、最后一个卸载时才关。
 * 否则先卸载的那个会把另一个的音量键也一起关掉。
 */
let users = 0;

export function useLogZoom() {
  function applyFontSize(next: number) {
    const v = Math.min(FONT_MAX, Math.max(FONT_MIN, Math.round(next)));
    fontSize.value = v;
    try {
      localStorage.setItem(FONT_KEY, String(v));
    } catch {
      // 存不住也无所谓，本次会话内照样生效
    }
  }

  function zoomBy(steps: number) {
    applyFontSize(fontSize.value + steps * FONT_STEP);
  }

  /* ── 双指捏合 ──────────────────────────────────────────────────────
   * 只在**双指**时接管：日志都是长列表，单指滚动远比缩放常用。
   * 1.15 倍死区：手指轻微抖动时不改字号，否则字会自己乱变。 */
  const pinch = { active: false, dist: 0, startFont: 0 };

  function touchDist(a: Touch, b: Touch): number {
    const dx = a.clientX - b.clientX;
    const dy = a.clientY - b.clientY;
    return Math.hypot(dx, dy);
  }

  function onTouchStart(e: TouchEvent) {
    if (e.touches.length !== 2) {
      pinch.active = false;
      return;
    }
    pinch.active = true;
    pinch.dist = touchDist(e.touches[0], e.touches[1]);
    pinch.startFont = fontSize.value;
  }

  function onTouchMove(e: TouchEvent) {
    if (!pinch.active || e.touches.length !== 2) return;
    const d = touchDist(e.touches[0], e.touches[1]);
    if (pinch.dist < 1) return;
    const ratio = d / pinch.dist;
    if (Math.abs(ratio - 1) < 0.15) return;
    applyFontSize(pinch.startFont * ratio);
    pinch.dist = d;
    pinch.startFont = fontSize.value;
    // 阻止 WebView 把双指当页面缩放
    e.preventDefault();
  }

  function onTouchEnd() {
    pinch.active = false;
  }

  /** 音量- / 音量+ ：原生拦截后派发（音量键不会进 WebView 的 keydown） */
  function onVolumeZoom(e: Event) {
    const delta = (e as CustomEvent<{ delta: number }>).detail?.delta;
    if (delta > 0) zoomBy(1);
    else if (delta < 0) zoomBy(-1);
  }

  onMounted(() => {
    window.addEventListener("qk-log-zoom", onVolumeZoom);
    if (++users === 1) {
      // 只有日志页在前台时才让原生接管音量键，否则游戏里按音量+-会没反应
      void api.setLogZoomCapture(true).catch(() => {});
    }
  });

  onBeforeUnmount(() => {
    window.removeEventListener("qk-log-zoom", onVolumeZoom);
    // 离开日志页必须关，否则游戏里音量键失效
    if (--users <= 0) {
      users = 0;
      void api.setLogZoomCapture(false).catch(() => {});
    }
  });

  return {
    fontSize,
    applyFontSize,
    zoomBy,
    /** 直接展开到模板上：@touchstart.passive="onTouchStart" 等 */
    touch: { onTouchStart, onTouchMove, onTouchEnd },
  };
}
