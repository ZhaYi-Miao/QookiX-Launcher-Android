<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, ref } from "vue";
import { useRoute } from "vue-router";
import { useMessage } from "../composables/message";
import { Button as VanButton } from "vant";
import { useInstancesStore } from "../stores/instances";
import { useAccountsStore } from "../stores/accounts";
import { loaderBadge } from "../utils/format";
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

const inst = computed(() => instances.get(instanceId));

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
  padding: 4px 12px 0;
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
