<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useMessage } from "../composables/message";
import { useLogZoom } from "../composables/useLogZoom";
import AppSwitch from "../ui/AppSwitch.vue";
import { save } from "@tauri-apps/plugin-dialog";
import { useTasksStore } from "../stores/tasks";
import { api } from "../api";
import { IconClose, IconCopy, IconDownload, IconFolder } from "./icons";

const props = defineProps<{ instanceId: string }>();
const tasks = useTasksStore();
const message = useMessage();
const box = ref<HTMLDivElement | null>(null);
const autoScroll = ref(true);

// 内存里的实时日志（launch://log 事件流）
const liveLogs = computed(() => tasks.logs[props.instanceId] ?? []);
// 磁盘上的日志：游戏崩溃会把整个进程带走，重启后内存里的日志就没了 ——
// 以前日志页此时永远显示「暂无日志」，偏偏在最需要它的时候。
// 所以内存为空时回退到磁盘文件（启动器日志 + 游戏自己的 latest.log）。
const fileLines = ref<{ line: string; stream?: string }[]>([]);
const logs = computed(() => (liveLogs.value.length ? liveLogs.value : fileLines.value));

async function loadFromDisk() {
  if (liveLogs.value.length) return;
  try {
    const text = await api.readInstanceLog(props.instanceId);
    fileLines.value = text.split("\n").map((line) => ({ line }));
  } catch {
    fileLines.value = [];
  }
}
onMounted(loadFromDisk);
watch(() => props.instanceId, () => {
  fileLines.value = [];
  void loadFromDisk();
});
// 一旦有实时日志进来，就用实时的那份
watch(() => liveLogs.value.length, (n) => {
  if (n) fileLines.value = [];
});

const logText = computed(() => logs.value.map((l) => l.line).join("\n"));

watch(() => logs.value.length, async () => {
  if (autoScroll.value) {
    await nextTick();
    if (box.value) box.value.scrollTop = box.value.scrollHeight;
  }
});

function clear() {
  tasks.clearLogs(props.instanceId);
  fileLines.value = [];
}

function onScroll() {
  if (!box.value) return;
  const el = box.value;
  autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
}

async function copyAll() {
  if (!logText.value) {
    message.info($t("log-viewer.no-logs"));
    return;
  }
  try {
    await navigator.clipboard.writeText(logText.value);
    message.success($t("log-viewer.all-copied"));
  } catch {
    // fallback for restricted contexts
    const ta = document.createElement("textarea");
    ta.value = logText.value;
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(ta);
    if (ok) message.success($t("log-viewer.all-copied"));
    else message.error($t("crash-analyzer.copy-failed"));
  }
}

/**
 * 打包分享日志文件（zip）。
 *
 * 为什么上面那个「导出」不够：它只把**当前显示的文本**存成文件，而游戏自己的
 * `latest.log` + 历史 `*.log.gz` 都在游戏目录里 —— 默认位于 `Android/data/<包名>/…`，
 * **Android 11+ 屏蔽了 Android/data**，文件管理器（哪怕有「所有文件访问」）和电脑 MTP
 * 都取不到。这个按钮把整个日志目录打包后走系统分享，才真正能把日志发出去。
 */
async function shareLogFiles() {
  try {
    await api.exportInstanceLogs(props.instanceId);
  } catch (e) {
    message.error(String(e));
  }
}

async function exportLog() {
  if (!logText.value) {
    message.info($t("log-viewer.no-logs"));
    return;
  }
  const ts = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const defaultName = `${props.instanceId}-${ts.getFullYear()}${pad(ts.getMonth() + 1)}${pad(ts.getDate())}-${pad(ts.getHours())}${pad(ts.getMinutes())}${pad(ts.getSeconds())}.log`;
  const path = await save({
    defaultPath: defaultName,
    filters: [{ name: $t("log-viewer.log-file"), extensions: ["log", "txt"] }],
  });
  if (!path) return;
  try {
    await api.saveTextFile(path as string, logText.value);
    // 安卓上 `path` 是 SAF 的 content:// URI，直接显示给用户没有意义
    message.success(path.startsWith("content://") ? $t("log-viewer.exported") : $t("log-viewer.exported-to", { p1: path }));
  } catch (e) {
    message.error(String(e));
  }
}

/* 字号缩放：与服务器日志/控制台共用 `useLogZoom`（含捏合、音量- / 音量+、记忆字号） */
const { fontSize, applyFontSize, zoomBy, touch } = useLogZoom();
</script>

<template>
  <div class="log-panel glass">
    <div class="log-toolbar">
      <span class="log-title">{{ $t("log-viewer.game-output") }}</span>
      <div class="log-actions">
        <label class="log-auto">
          <span>{{ $t("log-viewer.auto-scroll") }}</span>
          <app-switch :value="autoScroll" @update:value="(v: boolean) => (autoScroll = v)" />
        </label>
        <!-- 字号：按钮最直观，捏合和音量键是同一套逻辑的另外两个入口 -->
        <div class="log-zoom">
          <button class="mini icon-only" :title="$t('log-viewer.zoom-out')" :aria-label="$t('log-viewer.zoom-out')" @click="zoomBy(-1)">
            <span class="zoom-glyph">A</span><span class="zoom-sign">−</span>
          </button>
          <button class="mini icon-only" :title="$t('log-viewer.zoom-reset')" :aria-label="$t('log-viewer.zoom-reset')" @click="applyFontSize(12)">
            <span class="zoom-glyph">{{ fontSize }}</span>
          </button>
          <button class="mini icon-only" :title="$t('log-viewer.zoom-in')" :aria-label="$t('log-viewer.zoom-in')" @click="zoomBy(1)">
            <span class="zoom-glyph">A</span><span class="zoom-sign">+</span>
          </button>
        </div>
        <button class="mini" :title="$t('log-viewer.copy-all')" :aria-label="$t('log-viewer.copy-all')" @click="copyAll">
          <IconCopy />{{ $t("common.copy") }}</button>
        <button class="mini" :title="$t('log-viewer.export-file')" :aria-label="$t('log-viewer.export-file')" @click="exportLog">
          <IconDownload />{{ $t("log-viewer.export") }}</button>
        <button class="mini" :title="$t('log-viewer.share-files')" :aria-label="$t('log-viewer.share-files')" @click="shareLogFiles">
          <IconFolder />{{ $t("log-viewer.share-files-short") }}</button>
        <button class="mini" :title="$t('log-viewer.clear-logs')" :aria-label="$t('log-viewer.clear-logs')" @click="clear">
          <IconClose />{{ $t("log-viewer.clear") }}</button>
      </div>
    </div>
    <div
      ref="box"
      class="log-box mono"
      :style="{ fontSize: fontSize + 'px' }"
      @scroll="onScroll"
      @touchstart.passive="touch.onTouchStart"
      @touchmove.passive="touch.onTouchMove"
      @touchend="touch.onTouchEnd"
      @touchcancel="touch.onTouchEnd"
    >
      <div v-if="!logs.length" class="log-empty">
        {{ tasks.runningInstance === instanceId ? $t('log-viewer.game-starting') : $t('log-viewer.no-logs-hint') }}
      </div>
      <div
        v-for="(l, i) in logs"
        :key="i"
        class="log-line"
        :class="l.stream"
      >{{ l.line }}</div>
    </div>
    <div v-if="tasks.lastExit && tasks.lastExit.instanceId === instanceId && !tasks.gameRunning" class="exit-info">{{ $t("log-viewer.game-exited", { p1: tasks.lastExit.code ?? $t("crash-analyzer.unknown") }) }}</div>
  </div>
</template>

<style scoped>
.log-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
.log-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  /* 手机：窄屏放不下就换行，别把按钮压成小图标 */
  flex-wrap: wrap;
  gap: 8px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
}
/* 自动滚动开关：标签 + 开关一行，触控高度给足 */
.log-auto {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  font-size: 13px;
  color: var(--text-3);
}
.log-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}
.log-actions {
  display: flex;
  align-items: center;
  /* **窄屏必须换行**：原来这里没写 wrap，「自动滚动 + 4 个按钮」硬挤在一行，
     结果「打包日志」被压成竖排、「复制」被拆字—— 观感跟坏掉一样。 */
  flex-wrap: wrap;
  gap: 8px;
}
/* 字号那一组：三个小方块挨在一起，当成一个控件 */
.log-zoom {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0 2px;
}
.mini {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-2);
  border-radius: 7px;
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.12s;
  /* 同上：不写这句，窄屏下按钮里的文字会自己折行（「打包日志」竖成一条） */
  white-space: nowrap;
  flex: 0 0 auto;
}
/* 纯图标的方形按钮（字号组）：不撑宽度，避免跟旁边抢地方 */
.mini.icon-only {
  padding: 4px 7px;
  justify-content: center;
  position: relative;
}
.zoom-glyph {
  font-size: 12px;
  line-height: 1;
}
.zoom-sign {
  position: absolute;
  right: 1px;
  bottom: 0;
  font-size: 9px;
  line-height: 1;
  color: var(--accent);
}
.mini:hover {
  color: var(--text-1);
  background: rgba(255, 255, 255, 0.06);
}
.log-box {
  flex: 1;
  overflow-y: auto;
  padding: 12px 14px;
  font-size: 12px;
  line-height: 1.55;
  background: rgba(0, 0, 0, 0.25);
  /* allow selecting / copying log text */
  user-select: text;
  -webkit-user-select: text;
  cursor: text;
}
.log-line.err {
  color: #f0907f;
}
.log-line.out {
  color: #c9ccd6;
}
.log-empty {
  color: var(--text-3);
  padding: 20px 0;
  text-align: center;
}
.exit-info {
  padding: 8px 14px;
  font-size: 12px;
  color: var(--text-3);
  border-top: 1px solid var(--border);
}
</style>
