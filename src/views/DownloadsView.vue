<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, ref } from "vue";
import { useMessage } from "../composables/message";
import { useTasksStore, type TaskEntry } from "../stores/tasks";
import { fmtBytes, fmtSpeed } from "../utils/format";
import { taskPercent } from "../utils/task";
import { api } from "../api";
import { Button as VanButton, Progress as VanProgress, Tabs as VanTabs, Tab as VanTab } from "vant";
const tasks = useTasksStore();
const message = useMessage();
const activeTab = ref<"active" | "finished">("active");
const cancelling = ref<number[]>([]);
const list = computed(() => tasks.taskList.filter((t) => (activeTab.value === "active" ? !t.finished : t.finished)));

/**
 * 左右滑动切换「进行中 / 已完成」。
 * 阈值给 56px：太小的位移会把「点一下取消」之类的点击误判成滑动。
 */
let touchX = 0;
let touchY = 0;
function onTouchStart(e: TouchEvent) {
  touchX = e.touches[0].clientX;
  touchY = e.touches[0].clientY;
}
function onTouchEnd(e: TouchEvent) {
  const dx = e.changedTouches[0].clientX - touchX;
  const dy = e.changedTouches[0].clientY - touchY;
  if (Math.abs(dx) < 56 || Math.abs(dx) < Math.abs(dy) * 1.5) return;
  activeTab.value = activeTab.value === "active" ? "finished" : "active";
}

/** 单文件百分比（当前正在下的文件自己有字节数） */
function filePct(done: number, total: number): number {
  if (!total) return 0;
  return Math.min(100, Math.round((done / total) * 100));
}

/**
 * 展开 / 收起。展开后能看到「正在下哪些文件 + 各自进度」和最近完成的文件 ✓/✗
 * （这些数据 tasks store 一直在收，只是这一页之前没渲染 —— 桌面端就是这么做的）。
 */
const expanded = ref<number[]>([]);
function toggle(id: number) {
  expanded.value = expanded.value.includes(id)
    ? expanded.value.filter((i) => i !== id)
    : [...expanded.value, id];
}

async function cancel(t: TaskEntry) {
  if (cancelling.value.includes(t.id)) return;
  cancelling.value = [...cancelling.value, t.id];
  try {
    await api.cancelInstall(t.id);
    message.info($t("downloads.cancel-requested"));
  } catch (e) {
    message.error(String(e));
  } finally {
    cancelling.value = cancelling.value.filter((id) => id !== t.id);
  }
}
</script>

<template>
  <div class="dl">
    <!-- 手机：tab 容器原来是直角（Vant 默认无圆角），改成圆角胶囊；
         另外接上左右滑动切换（手机上滑一下切 tab 比点更顺手）。
         滑动自己实现而不开 Vant 的 swipeable —— 列表在 tabs 之外，
         swipeable 只对 pane 内容生效，这里 pane 是空的。 -->
    <div
      class="tab-swipe"
      @touchstart="onTouchStart"
      @touchend="onTouchEnd"
    >
    <van-tabs v-model:active="activeTab" class="tabs">
      <van-tab name="active" :title="$t('downloads.in-progress')" />
      <van-tab name="finished" :title="$t('downloads.finished')" />
    </van-tabs>
    </div>
    <p v-if="!list.length" class="empty">{{ $t("downloads.no-tasks") }}</p>
    <div v-for="t in list" :key="t.id" class="task glass">
      <!-- 整行可点：展开 / 收起逐文件详情（跟桌面端一致） -->
      <div class="row" @click="toggle(t.id)">
        <div class="name">
          {{ t.source ?? t.message }}
          <span v-if="t.instanceName" class="inst">{{ t.instanceName }}</span>
        </div>
        <span class="caret" :class="{ open: expanded.includes(t.id) }">▾</span>
        <van-button v-if="!t.finished" size="small" @click.stop="cancel(t)">
          {{ cancelling.includes(t.id) ? $t("downloads.cancel-requested") : $t("common.cancel") }}
        </van-button>
      </div>
      <div class="stage">{{ t.message || t.stage }}</div>
      <van-progress :percentage="taskPercent(t)" :show-pivot="false" />
      <div class="meta">
        <span v-if="t.bytesTotal">{{ fmtBytes(t.bytesDone) }} / {{ fmtBytes(t.bytesTotal) }}</span>
        <span v-else-if="t.fileTotal">{{ $t("downloads.files-progress", { p1: t.fileDone, p2: t.fileTotal }) }}</span>
        <span v-if="t.speed > 0">{{ fmtSpeed(t.speed) }}</span>
      </div>

      <!-- 展开：正在下的文件（各自带进度条）+ 最近完成的文件 ✓/✗ -->
      <div v-if="expanded.includes(t.id)" class="detail">
        <div class="d-row">
          <span>{{ $t("downloads.downloading") }}</span>
          <span>{{ $t("downloads.files-count", { p1: t.activeFiles.length }) }}</span>
        </div>
        <div v-if="t.speed > 0" class="d-row">
          <span>{{ $t("downloads.avg-speed") }}</span>
          <span>{{ fmtSpeed(t.speed) }}</span>
        </div>
        <div v-for="(f, i) in t.activeFiles" :key="'a' + i" class="f-cur">
          <div class="f-row">
            <span class="f-ico">→</span>
            <span class="f-name">{{ f.name }}</span>
            <span v-if="f.bytesTotal" class="f-pct">{{ filePct(f.bytesDone, f.bytesTotal) }}%</span>
          </div>
          <div v-if="f.bytesTotal" class="f-bar">
            <div class="f-fill" :style="{ width: filePct(f.bytesDone, f.bytesTotal) + '%' }" />
          </div>
        </div>
        <div
          v-for="(f, i) in t.files.slice(-30).reverse()"
          :key="i"
          class="f-row"
          :class="f.ok ? 'ok' : 'fail'"
        >
          <span class="f-ico">{{ f.ok ? "✓" : "✗" }}</span>
          <span class="f-name">{{ f.name }}</span>
        </div>
        <p v-if="!t.activeFiles.length && !t.files.length" class="f-empty">
          {{ $t("downloads.no-file-detail") }}
        </p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dl {
  padding: 4px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.empty {
  text-align: center;
  color: var(--text-3);
  padding: 48px 0;
}
.task {
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.name {
  flex: 1;
  min-width: 0;
  font-weight: 600;
  font-size: 14px;
}
.stage {
  font-size: 12px;
  color: var(--text-3);
}
.meta {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--text-3);
}
.inst {
  font-weight: 400;
  font-size: 12px;
  color: var(--text-3);
}
/* tab 条：Vant 默认直角，手机上太硬 —— 改成圆角胶囊 */
.tab-swipe {
  margin-bottom: 4px;
}
.tabs :deep(.van-tabs__wrap) {
  border-radius: 12px;
  overflow: hidden;
}
.tabs :deep(.van-tab) {
  min-height: 42px;
  align-items: center;
}
/* 展开箭头：收起时朝下、展开时翻转朝上 */
.caret {
  flex: none;
  font-size: 12px;
  color: var(--text-3);
  transition: transform 0.2s;
}
.caret.open {
  transform: rotate(180deg);
}
/* 展开区：正在下的文件（带各自进度条）+ 最近完成的文件 ✓/✗ */
.detail {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 8px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}
.d-row {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--text-3);
}
.f-cur {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.f-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--text-3);
}
.f-ico {
  flex: none;
  width: 12px;
}
.f-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.f-pct {
  flex: none;
}
.f-row.ok {
  color: var(--text-2);
}
.f-row.fail {
  color: #ff6b6b;
}
.f-bar {
  height: 3px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}
.f-fill {
  height: 100%;
  border-radius: 2px;
  background: #ffb020;
}
.f-empty {
  margin: 0;
  font-size: 11px;
  color: var(--text-3);
}

/* ── 手机横屏（矮窗口）：下载任务并排两张（页签行与空态横跨整行） ── */
@media (orientation: landscape) and (max-height: 560px) {
  .dl {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-items: start;
    gap: 10px;
  }
  .dl > .tab-swipe,
  .dl > .empty {
    grid-column: 1 / -1;
  }
}
</style>
