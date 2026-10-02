<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { NButton, useMessage } from "naive-ui";
import { api } from "../api";
import { fmtBytes } from "../utils/format";
import type { PluginInfo, PluginProgressEvent, PluginSetupStatus } from "../types";
import AppSheet from "../ui/AppSheet.vue";

/**
 * 首启准备：渲染器 / 驱动 / 图形组件都是插件分发的，新装的应用里一个都没有
 * （随包只剩 GL4ES 这一条老路，MC 26.x 那种走 SDL3 的版本直接起不来）。
 *
 * 启动后自动开下，窗口只负责显示清单与进度；点「后台继续」可以收起窗口让它在后台装完，
 * 装不上的下次启动还会再来一次，也能在「设置 → 插件」点「补齐推荐」手动补。
 */
const message = useMessage();

const show = ref(false);
const checking = ref(false);
const running = ref(false);
const finished = ref(false);
const status = ref<PluginSetupStatus | null>(null);
const progress = ref<PluginProgressEvent | null>(null);
const doneIds = ref<string[]>([]);
const failed = ref<string[]>([]);
let unlisten: (() => void) | null = null;

/// 这次要装的清单：装完之后 `status.missing` 就空了，但窗口里得留着「刚装了什么」，
/// 否则用户看到的是「没有需要补的 / 0 B」，像什么都没发生。
const planned = ref<PluginInfo[]>([]);
const items = computed<PluginInfo[]>(() =>
  planned.value.length ? planned.value : (status.value?.missing ?? [])
);
const totalText = computed(() =>
  planned.value.length
    ? fmtBytes(planned.value.reduce((sum, p) => sum + (p.size ?? 0), 0))
    : fmtBytes(status.value?.total_size ?? 0)
);

type ItemState = "wait" | "run" | "done" | "fail";

function stateOf(p: PluginInfo): ItemState {
  if (failed.value.some((f) => f.startsWith(p.name))) return "fail";
  if (doneIds.value.includes(p.id)) return "done";
  if (running.value && progress.value?.id === p.id) return "run";
  return "wait";
}

function stateText(p: PluginInfo): string {
  switch (stateOf(p)) {
    case "fail":
      return $t("first-run-setup.failed");
    case "done":
      return $t("first-run-setup.installed");
    case "run":
      return progress.value?.total
        ? `${Math.min(100, Math.round((progress.value.done / progress.value.total) * 100))}%`
        : progress.value?.message || $t("first-run-setup.processing");
    default:
      return $t("first-run-setup.queued");
  }
}

function percentOf(p: PluginInfo): number {
  const e = progress.value;
  if (!e || e.id !== p.id || !e.total) return 0;
  return Math.min(100, Math.round((e.done / e.total) * 100));
}

/// 查一遍还缺什么。缺就直接开下；清单拿不到（首启离线）时也把窗口弹出来，让用户能「重试 / 稍后」。
async function check() {
  if (checking.value) return;
  checking.value = true;
  try {
    const s = await api.getPluginSetupStatus();
    status.value = s;
    if (s.error) {
      show.value = true;
      return;
    }
    if (s.needed && !s.dismissed) {
      planned.value = s.missing;
      show.value = true;
      void start();
    }
  } catch (e) {
    message.warning($t("first-run-setup.check-failed", { p1: e }));
  } finally {
    checking.value = false;
  }
}

async function start() {
  if (running.value) return;
  running.value = true;
  finished.value = false;
  failed.value = [];
  doneIds.value = [];
  try {
    const r = await api.installRecommendedPlugins();
    failed.value = r.failed;
    // 装完重查一遍：失败的会继续留在列表里，方便重试
    const s = await api.getPluginSetupStatus();
    status.value = s;
    finished.value = !s.needed;
    if (r.failed.length) message.warning($t("first-run-setup.partial-failure", { p1: r.failed.length }));
    else if (finished.value) message.success($t("first-run-setup.ready"));
  } catch (e) {
    message.error(String(e));
  } finally {
    running.value = false;
    progress.value = null;
  }
}

async function dismiss() {
  show.value = false;
  try {
    await api.dismissPluginSetup();
  } catch {
    // 只是记个标记，失败也不影响使用
  }
}

onMounted(() => {
  void listen<PluginProgressEvent>("plugin://progress", (e) => {
    progress.value = e.payload;
    if (e.payload.phase === "done" && !doneIds.value.includes(e.payload.id)) {
      doneIds.value = [...doneIds.value, e.payload.id];
    }
  }).then((fn) => (unlisten = fn));
  // 等首页和实例列表先渲染完，别和启动时的其它请求抢带宽
  setTimeout(() => void check(), 900);
});
onBeforeUnmount(() => unlisten?.());
</script>

<template>
  <app-sheet
    v-model:show="show"
    :title="$t('first-run-setup.title')"
    class="frs-card"
    :mask-closable="false"
    :closable="false"
  >
    <template v-if="status?.error">
      <p class="frs-line">{{ $t("first-run-setup.manifest-error", { p1: status.error }) }}</p>
      <p class="frs-hint">{{ $t("first-run-setup.network-hint") }}</p>
    </template>

    <template v-else>
      <p class="frs-line">{{ $t("first-run-setup.intro") }}</p>
      <div class="frs-list">
        <div v-for="p in items" :key="p.id" class="frs-row" :class="stateOf(p)">
          <div class="frs-row-main">
            <div class="frs-name">
              {{ p.name }}
              <span class="frs-size">{{ fmtBytes(p.size ?? 0) }}</span>
            </div>
            <div class="frs-summary">{{ p.summary }}</div>
            <div v-if="stateOf(p) === 'run'" class="frs-bar-wrap">
              <div class="frs-bar" :style="{ width: percentOf(p) + '%' }"></div>
            </div>
          </div>
          <div class="frs-state">{{ stateText(p) }}</div>
        </div>
        <div v-if="!items.length" class="frs-hint">{{ $t("first-run-setup.nothing-to-do") }}</div>
      </div>
      <p class="frs-hint">{{ $t("first-run-setup.total-size", { p1: totalText }) }}<template v-if="finished">{{ $t("first-run-setup.all-ready-suffix") }}</template></p>
      <p v-if="finished" class="frs-hint">{{ $t("first-run-setup.ready-hint") }}</p>
      <p v-if="failed.length" class="frs-hint warn">{{ $t("first-run-setup.failed-list", { p1: failed.join("；") }) }}</p>
    </template>

    <div class="frs-actions">
      <!-- 下载在后台跑：收起窗口不影响，进度也能在「设置 → 插件」看到 -->
      <n-button v-if="running" @click="show = false">{{ $t("first-run-setup.run-in-background") }}</n-button>
      <template v-else>
        <n-button v-if="status?.error" @click="check">{{ $t("first-run-setup.retry") }}</n-button>
        <n-button v-else-if="finished || !items.length" type="primary" @click="show = false">{{ $t("common.close") }}</n-button>
        <n-button v-else @click="start">{{ $t("first-run-setup.retry") }}</n-button>
        <n-button v-if="!finished" @click="dismiss">{{ $t("first-run-setup.later") }}</n-button>
      </template>
    </div>
  </app-sheet>
</template>

<style>
/* 弹窗内容会被 teleport 出去，所以这里不用 scoped；类名统一 frs- 前缀避免撞车 */
.frs-line {
  margin: 0 0 10px;
  font-size: 13px;
  line-height: 1.65;
  color: var(--text-2, #c9c9c9);
}
.frs-hint {
  margin: 8px 0 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-3, #8a8a8a);
}
.frs-hint.warn {
  color: #e8a34b;
}
.frs-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 42vh;
  overflow-y: auto;
}
.frs-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 10px;
}
.frs-row.done {
  border-color: var(--accent-45, rgba(232, 154, 75, 0.45));
}
.frs-row.fail {
  border-color: rgba(232, 88, 88, 0.5);
}
.frs-row-main {
  flex: 1;
  min-width: 0;
}
.frs-name {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
}
.frs-size {
  font-size: 11px;
  font-weight: 400;
  color: var(--text-3, #8a8a8a);
}
.frs-summary {
  font-size: 11px;
  color: var(--text-3, #8a8a8a);
  margin-top: 2px;
}
.frs-bar-wrap {
  height: 3px;
  margin-top: 6px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.12);
  overflow: hidden;
}
.frs-bar {
  height: 100%;
  background: var(--accent, #e89a4b);
  transition: width 0.18s linear;
}
.frs-state {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-3, #8a8a8a);
}
.frs-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 14px;
}
</style>
