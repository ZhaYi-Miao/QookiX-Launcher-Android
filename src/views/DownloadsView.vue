<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, ref } from "vue";
import { useMessage } from "../composables/message";
import { useTasksStore, type TaskEntry } from "../stores/tasks";
import { fmtBytes, fmtSpeed } from "../utils/format";
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

function pct(t: TaskEntry): number {
  if (t.bytesTotal > 0) return Math.min(100, Math.round((t.bytesDone / t.bytesTotal) * 100));
  if (t.stepTotal > 0) return Math.min(100, Math.round((t.stepDone / t.stepTotal) * 100));
  return 0;
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
      <div class="row">
        <div class="name">
          {{ t.source ?? t.message }}
          <span v-if="t.instanceName" class="inst">{{ t.instanceName }}</span>
        </div>
        <van-button v-if="!t.finished" size="small" @click="cancel(t)">
          {{ cancelling.includes(t.id) ? $t("downloads.cancel-requested") : $t("common.cancel") }}
        </van-button>
      </div>
      <div class="stage">{{ t.message || t.stage }}</div>
      <van-progress :percentage="pct(t)" :show-pivot="false" />
      <div class="meta">
        <span v-if="t.bytesTotal">{{ fmtBytes(t.bytesDone) }} / {{ fmtBytes(t.bytesTotal) }}</span>
        <span v-if="t.speed > 0">{{ fmtSpeed(t.speed) }}</span>
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
</style>
