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
    <van-tabs v-model:active="activeTab" class="tabs">
      <van-tab name="active" :title="$t('downloads.in-progress')" />
      <van-tab name="finished" :title="$t('downloads.finished')" />
    </van-tabs>
    <p v-if="!list.length" class="empty">{{ $t("log-viewer.no-logs") }}</p>
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
</style>
