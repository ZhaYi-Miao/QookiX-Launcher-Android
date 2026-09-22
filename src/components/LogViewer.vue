<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { NCheckbox, useMessage } from "naive-ui";
import { save } from "@tauri-apps/plugin-dialog";
import { useTasksStore } from "../stores/tasks";
import { api } from "../api";
import { IconClose, IconCopy, IconDownload } from "./icons";

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
    message.info("暂无日志内容");
    return;
  }
  try {
    await navigator.clipboard.writeText(logText.value);
    message.success("已复制全部日志");
  } catch {
    // fallback for restricted contexts
    const ta = document.createElement("textarea");
    ta.value = logText.value;
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(ta);
    if (ok) message.success("已复制全部日志");
    else message.error("复制失败");
  }
}

async function exportLog() {
  if (!logText.value) {
    message.info("暂无日志内容");
    return;
  }
  const ts = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const defaultName = `${props.instanceId}-${ts.getFullYear()}${pad(ts.getMonth() + 1)}${pad(ts.getDate())}-${pad(ts.getHours())}${pad(ts.getMinutes())}${pad(ts.getSeconds())}.log`;
  const path = await save({
    defaultPath: defaultName,
    filters: [{ name: "日志文件", extensions: ["log", "txt"] }],
  });
  if (!path) return;
  try {
    await api.saveTextFile(path as string, logText.value);
    // 安卓上 `path` 是 SAF 的 content:// URI，直接显示给用户没有意义
    message.success(path.startsWith("content://") ? "日志已导出" : `已导出到 ${path}`);
  } catch (e) {
    message.error(String(e));
  }
}
</script>

<template>
  <div class="log-panel glass">
    <div class="log-toolbar">
      <span class="log-title">游戏日志输出</span>
      <div class="log-actions">
        <n-checkbox v-model:checked="autoScroll" size="small">自动滚动</n-checkbox>
        <button class="mini" title="复制全部日志" aria-label="复制全部日志" @click="copyAll">
          <IconCopy /> 复制
        </button>
        <button class="mini" title="导出日志文件" aria-label="导出日志文件" @click="exportLog">
          <IconDownload /> 导出
        </button>
        <button class="mini" title="清空日志" aria-label="清空日志" @click="clear">
          <IconClose /> 清空
        </button>
      </div>
    </div>
    <div ref="box" class="log-box mono" @scroll="onScroll">
      <div v-if="!logs.length" class="log-empty">
        {{ tasks.runningInstance === instanceId ? "游戏正在启动…" : "暂无日志。启动游戏后这里会实时显示输出。" }}
      </div>
      <div
        v-for="(l, i) in logs"
        :key="i"
        class="log-line"
        :class="l.stream"
      >{{ l.line }}</div>
    </div>
    <div v-if="tasks.lastExit && tasks.lastExit.instanceId === instanceId && !tasks.gameRunning" class="exit-info">
      游戏已退出（退出码 {{ tasks.lastExit.code ?? "未知" }}）
    </div>
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
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
}
.log-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}
.log-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
.auto {
  font-size: 12px;
  color: var(--text-3);
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  margin-right: 6px;
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
