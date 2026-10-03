<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useMessage } from "../composables/message";
import { Button as VanButton, Tabs as VanTabs, Tab as VanTab } from "vant";
import { api } from "../api";
import { useServersStore } from "../stores/servers";
import ServerFileManager from "../components/ServerFileManager.vue";
import AppInput from "../ui/AppInput.vue";

const route = useRoute();
const message = useMessage();
const servers = useServersStore();
const serverId = String(route.params.id);
const tab = ref("config");
const busy = ref("");
const logs = ref<string[]>([]);
const form = ref({ maxMem: 2048, minMem: 512, jvmArgs: "", stopCommand: "stop" });

const server = computed(() => servers.byId(serverId));
const running = computed(() => (server.value ? servers.isRunning(server.value.id) : false));

async function toggleRun() {
  const s = server.value;
  if (!s) return;
  busy.value = "run";
  try {
    if (running.value) {
      await servers.stop(s.id);
      message.success($t("multiplayer.server-stopped"));
    } else {
      await servers.start(s.id);
      message.success($t("multiplayer.server-started"));
    }
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

/** 参数保存：手机上没有「失焦自动保存」的心理预期，改成显式按钮 */
async function save() {
  const s = server.value;
  if (!s) return;
  busy.value = "save";
  try {
    await servers.update({
      id: s.id,
      max_memory_mb: Number(form.value.maxMem),
      min_memory_mb: Number(form.value.minMem),
      jvm_args: form.value.jvmArgs,
      stop_command: form.value.stopCommand,
    });
    message.success($t("common.save"));
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

async function loadLogs() {
  try {
    logs.value = await api.readHostedServerLog(serverId);
  } catch (e) {
    message.error(String(e));
  }
}

onMounted(async () => {
  if (!servers.servers.length) await servers.load();
  const s = server.value;
  if (s) {
    form.value.maxMem = s.max_memory_mb;
    form.value.minMem = s.min_memory_mb;
    form.value.jvmArgs = s.jvm_args ?? "";
    form.value.stopCommand = s.stop_command ?? "stop";
  }
});
</script>

<template>
  <div class="sd">
    <div class="head glass">
      <div class="hm">
        <div class="hn">{{ server?.name ?? "" }}</div>
        <div class="hs">{{ server?.core }} · {{ server?.mc_version }} · :{{ server?.port }}</div>
      </div>
      <span class="dot" :class="{ on: running }"></span>
    </div>
    <van-button class="run" block :type="running ? 'default' : 'primary'" :loading="busy === 'run'" @click="toggleRun">
      {{ running ? $t("multiplayer.stop") : $t("instance-card.launch") }}
    </van-button>

    <van-tabs v-model:active="tab" class="tabs" @change="tab === 'logs' && loadLogs()">
      <van-tab name="config" :title="$t('router.settings')" />
      <van-tab name="files" :title="$t('instance-detail.files')" />
      <van-tab name="logs" :title="$t('common.logs')" />
    </van-tabs>
    <div v-if="tab === 'config'" class="pane">
      <div class="field">
        <label>{{ $t("server-detail.max-memory") }}</label>
        <app-input v-model:value="form.maxMem" type="text" />
      </div>
      <div class="field">
        <label>{{ $t("server-detail.min-memory") }}</label>
        <app-input v-model:value="form.minMem" type="text" />
      </div>
      <div class="field">
        <label>JVM</label>
        <app-input v-model:value="form.jvmArgs" :placeholder="$t('instance-settings.jvm-args-hint')" type="textarea" :rows="3" />
      </div>
      <div class="field">
        <label>stop</label>
        <app-input v-model:value="form.stopCommand" placeholder="stop" />
      </div>
      <van-button block type="primary" :loading="busy === 'save'" @click="save">{{ $t("common.save") }}</van-button>
    </div>
    <div v-else-if="tab === 'files'" class="pane"><ServerFileManager :server-id="serverId" /></div>
    <div v-else class="pane logs">
      <pre v-for="(l, i) in logs" :key="i" class="ln">{{ l }}</pre>
      <p v-if="!logs.length" class="empty">{{ $t("log-viewer.no-logs") }}</p>
    </div>
  </div>
</template>

<style scoped>
.sd {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 4px 16px 12px;
}
.head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  flex-shrink: 0;
}
.hm {
  flex: 1;
  min-width: 0;
}
.hn {
  font-size: 16px;
  font-weight: 700;
}
.hs {
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-3);
}
.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--text-3);
}
.dot.on {
  background: #7ad08a;
}
.run {
  min-height: 48px;
  flex-shrink: 0;
}
.tabs {
  flex-shrink: 0;
}
.pane {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 4px 12px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.field label {
  display: block;
  margin-bottom: 6px;
  font-size: 13px;
  color: var(--text-2);
}
.logs {
  gap: 2px;
}
.ln {
  margin: 0;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11px;
  color: var(--text-2);
  white-space: pre-wrap;
  word-break: break-all;
}
.empty {
  text-align: center;
  color: var(--text-3);
  padding: 40px 0;
}
</style>
