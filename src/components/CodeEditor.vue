<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import { highlight, langLabel } from "../utils/highlight";
import { api } from "../api";

const props = defineProps<{
  modelValue: string;
  /** 文件名，用于显示语言类型 */
  filename: string;
  readonly?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: string): void;
  (e: "save"): void;
  (e: "contextmenu", p: { x: number; y: number }): void;
}>();

const taRef = ref<HTMLTextAreaElement | null>(null);
const scroller = ref<HTMLDivElement | null>(null);
const scrollTop = ref(0);
const cursorLine = ref(1);
const cursorCol = ref(1);
const focused = ref(false);
const message = useMessage();

const LINE_H = 20;
const INDENT = "  ";
/** 视口高度（用于行号虚拟化），onMounted / resize 时量一次 */
const viewportH = ref(0);

const ext = computed(() => {
  const i = props.filename.lastIndexOf(".");
  return i < 0 ? "" : props.filename.slice(i + 1).toLowerCase();
});

const lines = computed(() => props.modelValue.split("\n"));

/**
 * 高亮结果（**滞后**于 modelValue 最多 120ms）。
 *
 * 以前高亮是直接 computed：每敲一个字都要把整篇跑一遍正则 —— 打开几千行的
 * `config.toml` / `options.txt` 时输入会明显卡顿。高亮是纯装饰，
 * 让它慢一点追上完全没影响，但键入必须立刻响应。
 */
const highlighted = ref("");
let hlTimer: number | undefined;
watch(
  () => props.modelValue,
  (v) => {
    if (hlTimer) clearTimeout(hlTimer);
    hlTimer = window.setTimeout(() => {
      highlighted.value = highlight(v);
    }, 120);
  },
  { immediate: true }
);

/**
 * 行号只渲染可视区内的（±2 行缓冲）。
 * 一个 5000 行的文件原来会生成 5000 个 DOM 节点，光布局就要几百毫秒。
 */
const gutter = computed(() => {
  const total = lines.value.length;
  if (!viewportH.value) return { first: 1, last: total };
  const first = Math.max(1, Math.floor(scrollTop.value / LINE_H) - 1);
  const visible = Math.ceil(viewportH.value / LINE_H) + 3;
  return { first, last: Math.min(total, first + visible) };
});

function onInput(e: Event) {
  const ta = e.target as HTMLTextAreaElement;
  emit("update:modelValue", ta.value);
  syncScroll();
  updateCursor();
}

function onScroll() {
  syncScroll();
}

function syncScroll() {
  const el = scroller.value;
  const ta = taRef.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (ta) {
    // 外层容器滚动时同步 textarea 自身的滚动位置，保证光标可见
    ta.scrollTop = el.scrollTop;
    ta.scrollLeft = el.scrollLeft;
  }
}

function updateCursor() {
  const ta = taRef.value;
  if (!ta) return;
  const upto = ta.value.slice(0, ta.selectionStart);
  const nl = upto.split("\n");
  cursorLine.value = nl.length;
  cursorCol.value = (nl[nl.length - 1]?.length ?? 0) + 1;
}

/** 用 execCommand 插入文本，保留浏览器原生撤销栈 */
function insert(text: string) {
  const ta = taRef.value;
  if (!ta) return;
  ta.focus();
  if (!document.execCommand("insertText", false, text)) {
    const s = ta.selectionStart;
    const e = ta.selectionEnd;
    const v = ta.value.slice(0, s) + text + ta.value.slice(e);
    emit("update:modelValue", v);
    nextTick(() => {
      ta.selectionStart = ta.selectionEnd = s + text.length;
      updateCursor();
    });
  }
}

function onKeydown(e: KeyboardEvent) {
  const ta = taRef.value;
  if (!ta) return;
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    emit("save");
    return;
  }
  if (e.key === "Tab") {
    e.preventDefault();
    insert(INDENT);
    return;
  }
  if (e.key === "Enter" && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
    // 继承当前行的缩进
    const upto = ta.value.slice(0, ta.selectionStart);
    const cur = upto.split("\n").pop() ?? "";
    const m = cur.match(/^[ \t]*/);
    if (m && m[0]) {
      e.preventDefault();
      insert("\n" + m[0]);
    }
  }
}

// ---- 供右键菜单调用的编辑操作 ----

function selection(): string {
  const ta = taRef.value;
  if (!ta) return "";
  return ta.value.slice(ta.selectionStart, ta.selectionEnd);
}

async function copySelection() {
  const text = selection();
  if (!text) return;
  // 顺序：原生 → Web API → execCommand。
  // 实测 WebView 连 `clipboard-write` 都没给（`writeText` 抛 NotAllowedError），
  // 以前只能押在 `execCommand("copy")` 的兜底上（它依赖用户手势，不牢靠）。
  try {
    await api.writeClipboard(text);
    return;
  } catch {
    /* 原生不可用（桌面端）→ 试 Web API */
  }
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    /* 同样不可用 */
  }
  if (!document.execCommand("copy")) {
    message.warning("复制失败，可长按编辑区用系统菜单复制");
  }
}

function cutSelection() {
  const ta = taRef.value;
  if (!ta) return;
  void copySelection();
  if (!document.execCommand("delete")) {
    const s = ta.selectionStart;
    const e = ta.selectionEnd;
    const v = ta.value.slice(0, s) + ta.value.slice(e);
    emit("update:modelValue", v);
    nextTick(() => {
      ta.selectionStart = ta.selectionEnd = s;
      updateCursor();
    });
  }
}

async function pasteClipboard() {
  // 两条 Web 路线在安卓 WebView 上都是死的：
  //   ① `navigator.clipboard.readText()` 需要 `clipboard-read` 权限，wry 没授予 → 抛错；
  //   ② `document.execCommand("paste")` 在 Chromium 里按设计**永远返回 false**。
  // 于是以前点「粘贴」既不报错也不插入内容。第一条路改成走原生 ClipboardManager，
  // Web API 只留给桌面端兜底。
  let text = "";
  try {
    text = await api.readClipboard();
  } catch {
    /* 原生不可用（桌面端）→ 试 Web API */
  }
  if (!text) {
    try {
      text = await navigator.clipboard.readText();
    } catch {
      /* 同样不可用 */
    }
  }
  if (!text) {
    // 以前这里是静默失败，用户完全不知道发生了什么
    message.warning("读不到剪贴板内容，可长按编辑区用系统菜单粘贴");
    return;
  }
  insert(text);
}

/**
 * 最近一次指针是不是触摸屏。
 *
 * `contextmenu` 事件本身不带指针类型，只能自己记。
 * 为什么要记：触屏长按 textarea 是**打开系统「选择 / 粘贴」工具条的唯一入口**，
 * 对它 `preventDefault` 等于把粘贴的最后一条路也封死
 * （`main.ts` 里给 input/textarea 留的白名单会被组件自己的 `.prevent` 反压掉）。
 */
let lastPointerWasTouch = false;
window.addEventListener(
  "pointerdown",
  (e) => {
    lastPointerWasTouch = e.pointerType === "touch";
  },
  true
);

function onContextMenu(e: MouseEvent) {
  const touch =
    lastPointerWasTouch ||
    (e as MouseEvent & { sourceCapabilities?: { firesTouchEvents?: boolean } })
      .sourceCapabilities?.firesTouchEvents === true;
  // 触屏：放行默认行为，让系统菜单出来
  if (touch) return;
  // 鼠标 / 触控板：用应用自己的右键菜单
  e.preventDefault();
  emit("contextmenu", { x: e.clientX, y: e.clientY });
}

function selectAll() {
  const ta = taRef.value;
  if (!ta) return;
  ta.focus();
  ta.select();
  updateCursor();
}

defineExpose({
  focus: () => taRef.value?.focus(),
  copySelection,
  cutSelection,
  pasteClipboard,
  selectAll,
  /** 是否存在选区，用于启用/禁用复制与剪切 */
  hasSelection: () => selection().length > 0,
  /** 滚动到指定行（1 开始） */
  gotoLine: (n: number) => {
    const el = scroller.value;
    if (!el) return;
    el.scrollTop = Math.max(0, (n - 1) * LINE_H - 40);
    syncScroll();
  },
});

watch(
  () => props.filename,
  () => {
    scrollTop.value = 0;
    if (scroller.value) {
      scroller.value.scrollTop = 0;
      scroller.value.scrollLeft = 0;
    }
    requestAnimationFrame(() => taRef.value?.focus());
  }
);

function measureViewport() {
  viewportH.value = scroller.value?.clientHeight ?? 0;
}

onMounted(() => {
  updateCursor();
  measureViewport();
  window.addEventListener("resize", measureViewport);
});

onBeforeUnmount(() => {
  if (hlTimer) clearTimeout(hlTimer);
  window.removeEventListener("resize", measureViewport);
});
</script>

<template>
  <div class="ed" :class="{ focus: focused }">
    <!-- 行号：只渲染可视区（±缓冲）。上面用 spacer 撑出被跳过的高度，
         保证行号与正文始终对齐。5000 行的文件原来会生成 5000 个节点。 -->
    <div class="ed-gutter" aria-hidden="true">
      <div class="ed-gutter-inner" :style="{ transform: `translateY(${-scrollTop}px)` }">
        <div class="ed-gutter-spacer" :style="{ height: `${(gutter.first - 1) * LINE_H}px` }"></div>
        <div
          v-for="n in gutter.last - gutter.first + 1"
          :key="gutter.first + n - 1"
          class="ed-ln"
          :class="{ cur: gutter.first + n - 1 === cursorLine }"
        >
          {{ gutter.first + n - 1 }}
        </div>
      </div>
    </div>

    <div ref="scroller" class="ed-scroll" @scroll="onScroll">
      <div class="ed-inner">
        <pre class="ed-hl" aria-hidden="true"><code v-html="highlighted"></code></pre>
        <textarea
          ref="taRef"
          class="ed-input"
          :value="modelValue"
          :readonly="readonly"
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
          wrap="off"
          @input="onInput"
          @scroll="onScroll"
          @keydown="onKeydown"
          @click="updateCursor"
          @keyup="updateCursor"
          @select="updateCursor"
          @focus="
            focused = true;
            updateCursor();
          "
          @blur="focused = false"
          @contextmenu="onContextMenu"
        ></textarea>
      </div>
    </div>

    <div class="ed-status">
      <span class="ed-lang">{{ langLabel(ext) }}</span>
      <span>行 {{ cursorLine }}，列 {{ cursorCol }}</span>
      <span>{{ lines.length }} 行</span>
      <span class="ed-tip">Ctrl+S 保存 · Tab 缩进</span>
    </div>
  </div>
</template>

<style scoped>
.ed {
  position: relative;
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.22);
  border-radius: 0 0 13px 13px;
  user-select: text;
  -webkit-user-select: text;
}

.ed-gutter {
  width: 56px;
  flex-shrink: 0;
  overflow: hidden;
  padding: 10px 0 22px;
  background: rgba(255, 255, 255, 0.025);
  border-right: 1px solid var(--border);
  text-align: right;
}
.ed-gutter-inner {
  will-change: transform;
}
/* 被虚拟化跳过的那些行，用一块等高的空白占位，保证行号与正文对齐 */
.ed-gutter-spacer {
  width: 100%;
}
.ed-ln {
  height: 20px;
  line-height: 20px;
  padding-right: 10px;
  font-family: "Cascadia Code", Consolas, "Courier New", monospace;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: var(--text-3);
  opacity: 0.55;
}
.ed-ln.cur {
  color: var(--accent);
  opacity: 1;
}

.ed-scroll {
  position: relative;
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding-bottom: 22px;
}

.ed-inner {
  position: relative;
  width: max-content;
  min-width: 100%;
  min-height: 100%;
}

.ed-hl,
.ed-input {
  margin: 0;
  padding: 10px 14px 0;
  font-family: "Cascadia Code", Consolas, "Courier New", monospace;
  font-size: 13px;
  line-height: 20px;
  letter-spacing: 0;
  tab-size: 2;
  white-space: pre;
  word-break: normal;
  overflow-wrap: normal;
  border: none;
}

.ed-hl {
  display: block;
  color: var(--text-1);
  pointer-events: none;
}
.ed-hl code {
  font: inherit;
}

.ed-input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  resize: none;
  outline: none;
  overflow: hidden;
  background: transparent;
  color: transparent;
  -webkit-text-fill-color: transparent;
  caret-color: var(--accent);
}
.ed-input::selection {
  background: rgba(232, 154, 75, 0.32);
}

.ed-status {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 22px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 12px;
  font-size: 11px;
  color: var(--text-3);
  background: var(--panel);
  border-top: 1px solid var(--border);
}
.ed-lang {
  color: var(--accent);
  font-weight: 600;
}
.ed-tip {
  margin-left: auto;
  opacity: 0.7;
}

/* v-html 生成的内容需要 :deep 才能命中 */
.ed :deep(.tk-key) {
  color: #9cdcfe;
}
.ed :deep(.tk-str) {
  color: #ce9178;
}
.ed :deep(.tk-com) {
  color: #6a9955;
  font-style: italic;
}
.ed :deep(.tk-sec) {
  color: #e8a33d;
  font-weight: 600;
}
.ed :deep(.tk-kw) {
  color: #569cd6;
}
.ed :deep(.tk-num) {
  color: #b5cea8;
}
</style>
