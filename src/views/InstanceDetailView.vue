<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useMessage } from "../composables/message";
import { Button as VanButton, Progress as VanProgress } from "vant";
import { api } from "../api";
import type { InstanceFileReport } from "../types";
import { useInstancesStore } from "../stores/instances";
import { useAccountsStore } from "../stores/accounts";
import { useTasksStore } from "../stores/tasks";
import { fmtBytes, fmtSpeed, loaderBadge } from "../utils/format";
import AppIcon from "../components/AppIcon.vue";
import ContentTab from "../components/instance/ContentTab.vue";
import SavesTab from "../components/instance/SavesTab.vue";
import SettingsTab from "../components/instance/SettingsTab.vue";
import KeysTab from "../components/instance/KeysTab.vue";
import FileManager from "../components/FileManager.vue";
import LogViewer from "../components/LogViewer.vue";
import CrashAnalyzer from "../components/CrashAnalyzer.vue";
import { IconPlay } from "../components/icons";

const route = useRoute();

const message = useMessage();
const instances = useInstancesStore();
const accounts = useAccountsStore();

const instanceId = String(route.params.id);
const tab = ref("content");
const launching = ref(false);

const tasks = useTasksStore();

/**
 * 这个实例当前有没有「在跑的文件任务」。
 *
 * 创建实例后的后台安装、点「补全游戏文件」、以及内容安装都会把任务挂到
 * `instanceId` 上；只要它还在跑，就不该显示「需要补全游戏文件」那条警告 ——
 * 否则用户刚建完实例点进来，会以为启动器出问题了（其实文件正在下）。
 */
const installTask = computed(
  () => tasks.taskList.find((t) => t.instanceId === instanceId && !t.finished),
);

/** 进度百分比：优先按字节，其次文件数，最后按安装步骤 */
const installPct = computed(() => {
  const t = installTask.value;
  if (!t) return 0;
  if (t.bytesTotal > 0) return Math.min(100, Math.round((t.bytesDone / t.bytesTotal) * 100));
  if (t.fileTotal > 0) return Math.min(100, Math.round((t.fileDone / t.fileTotal) * 100));
  if (t.stepTotal > 0) return Math.min(100, Math.round((t.stepDone / t.stepTotal) * 100));
  return 0;
});

const cancelling = ref(false);
async function cancelInstall() {
  const t = installTask.value;
  if (!t || cancelling.value) return;
  cancelling.value = true;
  try {
    await api.cancelInstall(t.id);
    message.info($t("downloads.cancel-requested"));
  } catch (e) {
    message.error(String(e));
  } finally {
    cancelling.value = false;
  }
}

// 下载结束后重新体检一次：这时才该决定要不要显示「补全」提示
watch(installTask, (now, before) => {
  if (before && !now) void checkFiles();
});

const inst = computed(() => instances.get(instanceId));

/**
 * 游戏文件体检结果（后端 `check_instance_files`，只读、不下载）。
 *
 * null 表示「文件完整」或「还没查出来」——两种情况都不显示提示条。
 * 创建实例时的后台安装一旦失败（网络断流 / 进程被杀 / 磁盘满），实例会停在
 * 「只有 instance.json」的状态：界面看不出原因，也没有补救入口，这里把
 * 「缺什么」和「补全」摆到启动按钮下面。
 */
const fileReport = ref<InstanceFileReport | null>(null);
const repairing = ref(false);

/** 有可修的项才给按钮；全是 `fixable:false`（如加载器没有对应版本）时按钮禁用 */
const canRepair = computed(() => !!fileReport.value?.missing.some((m) => m.fixable));

async function checkFiles() {
  try {
    const r = await api.checkInstanceFiles(instanceId);
    fileReport.value = r.can_launch ? null : r;
  } catch {
    // 实例不存在 / 配置损坏：交给原本的报错路径，这里不弹提示条
    fileReport.value = null;
  }
}

async function repair() {
  if (!canRepair.value || repairing.value) return;
  repairing.value = true;
  message.loading($t("instance-detail.repairing"));
  try {
    await api.repairInstanceFiles(instanceId);
    await instances.load(true);
    await checkFiles();
    // 还有残留（例如加载器版本号为空这种补不回来的）就直接把后端的话给用户看
    if (fileReport.value) message.warning(fileReport.value.advice);
    else message.success($t("instance-detail.repair-done"));
  } catch (e) {
    message.error(String(e));
  } finally {
    repairing.value = false;
  }
}

onMounted(checkFiles);

const TABS = [
  { key: "content", label: $t("nav.browse") },
  { key: "saves", label: $t("instance-detail.group") },
  { key: "keys", label: $t("instance-detail.keys") },
  { key: "settings", label: $t("router.settings") },
  { key: "files", label: $t("instance-detail.files") },
  { key: "logs", label: $t("common.logs") },
  { key: "crash", label: $t("crash-dialog.crash") },
];

async function launch() {
  if (!inst.value) return;
  if (!accounts.accounts.length) {
    message.warning($t("home.add-account-hint"));
    accounts.showManager = true;
    return;
  }
  launching.value = true;
  try {
    const res = await instances.launch(instanceId);
    if (res) message.success($t("home.launched", { p1: inst.value.name }));
  } catch (e) {
    message.error(String(e));
    // 启动失败常见原因就是文件没下全，顺手复查一次，把提示条亮出来
    void checkFiles();
  } finally {
    launching.value = false;
  }
}
</script>

<template>
  <div class="dv">
    <div class="head glass">
      <div class="ico"><AppIcon :name="inst?.icon" /></div>
      <div class="info">
        <div class="nm">{{ inst?.name ?? "" }}</div>
        <div class="mt">
          <span class="badge">{{ loaderBadge(inst?.loader ?? "") }}</span>
          <span>{{ inst?.mc_version }}</span>
        </div>
      </div>
      <van-button type="primary" class="go" :loading="launching" @click="launch"><IconPlay /></van-button>
    </div>

    <!-- 游戏文件正在下载：显示进度 + 取消。
         这种时候**不能**显示下面那条「需要补全游戏文件」——用户刚建完实例点进来
         看到「缺 4 项，点补全」，会以为启动器坏了（其实后台正在下）。 -->
    <div v-if="installTask" class="guard guard-run">
      <div class="guard-txt">
        <div class="guard-title">{{ installTask.message || installTask.stage }}</div>
        <van-progress :percentage="installPct" :show-pivot="false" />
        <div class="guard-advice run-meta">
          <span v-if="installTask.bytesTotal">{{ fmtBytes(installTask.bytesDone) }} / {{ fmtBytes(installTask.bytesTotal) }}</span>
          <span v-else-if="installTask.fileTotal">{{ installTask.fileDone }} / {{ installTask.fileTotal }}</span>
          <span v-if="installTask.speed > 0">{{ fmtSpeed(installTask.speed) }}</span>
        </div>
      </div>
      <van-button size="small" class="guard-btn" :loading="cancelling" @click="cancelInstall">
        {{ $t("common.cancel") }}
      </van-button>
    </div>

    <div v-else-if="fileReport" class="guard">
      <div class="guard-txt">
        <div class="guard-title">
          {{ $t("instance-detail.files-incomplete", { p1: fileReport.ok, p2: fileReport.total }) }}
        </div>
        <div class="guard-advice">{{ fileReport.advice }}</div>
      </div>
      <van-button
        size="small"
        type="primary"
        class="guard-btn"
        :loading="repairing"
        :disabled="!canRepair"
        @click="repair"
      >{{ $t("instance-detail.repair-files") }}</van-button>
    </div>

    <div class="tabs">
      <button v-for="t in TABS" :key="t.key" class="tb" :class="{ on: tab === t.key }" @click="tab = t.key">{{ t.label }}</button>
    </div>

    <div class="body">
      <ContentTab v-if="tab === 'content'" :instance-id="instanceId" kind="mod" />
      <SavesTab v-else-if="tab === 'saves'" :instance-id="instanceId" />
      <KeysTab v-else-if="tab === 'keys'" :instance-id="instanceId" />
      <SettingsTab v-else-if="tab === 'settings'" :instance-id="instanceId" />
      <FileManager v-else-if="tab === 'files'" :instance-id="instanceId" />
      <LogViewer v-else-if="tab === 'logs'" :instance-id="instanceId" />
      <CrashAnalyzer v-else-if="tab === 'crash'" :instance-id="instanceId" />
    </div>
  </div>
</template>

<style scoped>
.dv {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 4px 16px 0;
  gap: 10px;
}
.head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  flex-shrink: 0;
}
.ico {
  width: 52px;
  height: 52px;
  border-radius: 14px;
  overflow: hidden;
  background: linear-gradient(135deg, var(--accent-25), var(--accent-08));
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--accent);
  flex-shrink: 0;
}
.info {
  flex: 1;
  min-width: 0;
}
.nm {
  font-size: 16px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mt {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-3);
}
.badge {
  background: var(--accent-16);
  color: var(--accent);
  border-radius: 6px;
  padding: 1px 7px;
  font-weight: 600;
}
.go {
  flex-shrink: 0;
  min-height: 44px;
  min-width: 52px;
}
.guard {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid var(--accent-25);
  background: var(--accent-08);
  flex-shrink: 0;
}
.guard-txt {
  flex: 1;
  min-width: 0;
}
.guard-title {
  font-size: 13px;
  font-weight: 600;
}
.guard-advice {
  margin-top: 3px;
  font-size: 12px;
  line-height: 1.4;
  color: var(--text-3);
}
.guard-btn {
  flex-shrink: 0;
}
.tabs {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  scrollbar-width: none;
  flex-shrink: 0;
  padding-bottom: 2px;
}
.tb {
  flex-shrink: 0;
  min-height: 36px;
  padding: 6px 14px;
  border-radius: 18px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
  white-space: nowrap;
}
.tb.on {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-08);
}
.body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.body > * {
  height: 100%;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
}
</style>
