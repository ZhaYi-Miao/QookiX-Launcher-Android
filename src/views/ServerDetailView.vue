<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useMessage } from "../composables/message";
import {
  Button as VanButton,
  Tabs as VanTabs,
  Tab as VanTab,
  Field as VanField,
} from "vant";
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
const form = ref({ maxMem: 1024, minMem: 512, jvmArgs: "", stopCommand: "stop" });

/** 核心是否已下载（决定按钮是「下载核心」还是「启动」） */
const coreReady = ref(false);
/** 联机地址（WiFi IP:端口），null = 未连 WiFi */
const address = ref<string | null>(null);
/** 控制台输入 */
const consoleInput = ref("");
const consoleOut = ref("");
/** 日志滚动容器（用于自动追尾） */
const logPane = ref<HTMLElement | null>(null);

const server = computed(() => servers.byId(serverId));
const running = computed(() => (server.value ? servers.isRunning(server.value.id) : false));

/** 轮询运行状态：服务端在 :server 进程里，主进程只能靠 IPC 探活才知道 */
let timer: ReturnType<typeof setInterval> | null = null;
function startPolling() {
  if (timer) return;
  timer = setInterval(async () => {
    const s = server.value;
    if (!s) return;
    try {
      const now = await api.isHostedServerRunning(s.id);
      if (now !== servers.isRunning(s.id)) servers.setRunning(s.id, now);
      if (now) {
        const addr = await api.hostedServerAddress(s.id);
        if (addr !== address.value) address.value = addr;
      }
      // 日志页签打开且正在运行 → 自动追尾刷新。
      // 服务端在另一个进程里跑，不轮询的话日志就停在打开那一刻，
      // 用户看到的是「死」的日志，还以为服务器挂了。
      if (tab.value === "logs" && now) await loadLogs();
    } catch {
      /* 忽略：下一轮再试 */
    }
  }, 2500);
}

async function installCore() {
  const s = server.value;
  if (!s) return;
  busy.value = "install";
  try {
    await api.installHostedServerCore(s.id);
    coreReady.value = true;
    message.success($t("server-detail.core-ready"));
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

async function toggleRun() {
  const s = server.value;
  if (!s) return;
  busy.value = "run";
  try {
    if (running.value) {
      const note = await api.stopHostedServer(s.id);
      servers.setRunning(s.id, false);
      message.success(note || $t("multiplayer.server-stopped"));
      await loadLogs();
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

async function copyAddress() {
  if (!address.value) return;
  try {
    await navigator.clipboard.writeText(address.value);
    message.success($t("server-detail.copied"));
  } catch {
    message.error(address.value);
  }
}

async function sendCommand() {
  const s = server.value;
  const cmd = consoleInput.value.trim();
  if (!s || !cmd) return;
  try {
    const out = await api.serverConsoleCommand(s.id, cmd);
    consoleOut.value = out || $t("server-detail.console-ok");
    consoleInput.value = "";
    await loadLogs();
  } catch (e) {
    message.error(String(e));
  }
}

/** 参数保存：手机上没有「失焦自动保存」的心理预期，改成显式按钮 */
async function save() {
  const s = server.value;
  if (!s) return;
  busy.value = "save";
  try {
    // 「最小内存 > 最大内存」是 JVM 直接拒绝启动的经典错误（新建服务器的默认值
    // 就正好是反的：min 1024 / max 512）。这里在保存前纠正，别让用户撞上。
    let maxMem = Number(form.value.maxMem) || 1024;
    let minMem = Number(form.value.minMem) || 512;
    if (minMem > maxMem) {
      minMem = maxMem;
      form.value.minMem = minMem;
      message.warning($t("server-detail.min-clamped"));
    }
    await servers.update({
      id: s.id,
      max_memory_mb: maxMem,
      min_memory_mb: minMem,
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
    // 自动追尾：新日志在数组末尾，不滚到底部就等于没刷新
    await nextTick();
    const pane = logPane.value;
    if (pane) pane.scrollTop = pane.scrollHeight;
  } catch (e) {
    message.error(String(e));
  }
}

onMounted(async () => {
  // 详情页可能被直接深链打开（刷新 / 从通知进入），列表为空时要自己拉一次，
  // 否则 byId 拿不到配置，标题栏会空着、按钮也会误判成「没装核心」。
  await servers.load(true);
  const s = server.value;
  if (s) {
    form.value.maxMem = s.max_memory_mb;
    form.value.minMem = s.min_memory_mb;
    form.value.jvmArgs = s.jvm_args ?? "";
    form.value.stopCommand = s.stop_command ?? "stop";
    try {
      coreReady.value = await api.hostedServerCoreInstalled(s.id);
      address.value = await api.hostedServerAddress(s.id);
    } catch {
      /* 忽略 */
    }
  }
  startPolling();
});

onUnmounted(() => {
  if (timer) {
    clearInterval(timer);
    timer = null;
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

    <!-- 联机地址：朋友在电脑上填这个就能连进来 -->
    <div class="addr glass">
      <div class="addr-l">
        <div class="addr-t">{{ $t("server-detail.address") }}</div>
        <div class="addr-v">{{ address ?? $t("server-detail.no-wifi") }}</div>
      </div>
      <van-button v-if="address" size="small" class="addr-b" @click="copyAddress">
        {{ $t("server-detail.copy-address") }}
      </van-button>
    </div>
    <p class="addr-hint">{{ $t("server-detail.address-hint") }}</p>

    <!-- 核心没装时先给「下载核心」，装了才允许启动 -->
    <van-button
      v-if="!coreReady && !running"
      class="run"
      block
      type="primary"
      :loading="busy === 'install'"
      @click="installCore"
    >
      {{ busy === "install" ? $t("server-detail.installing") : $t("server-detail.install") }}
    </van-button>
    <van-button
      v-else
      class="run"
      block
      :type="running ? 'default' : 'primary'"
      :loading="busy === 'run'"
      @click="toggleRun"
    >
      {{ running ? $t("multiplayer.stop") : $t("server-detail.start") }}
    </van-button>
    <p v-if="!coreReady && !running" class="addr-hint">{{ $t("server-detail.memory-hint") }}</p>

    <van-tabs v-model:active="tab" class="tabs" @change="tab !== 'config' && loadLogs()">
      <van-tab name="config" :title="$t('router.settings')" />
      <van-tab name="files" :title="$t('instance-detail.files')" />
      <van-tab name="logs" :title="$t('common.logs')" />
      <van-tab name="console" :title="$t('server-detail.console')" />
    </van-tabs>
    <div v-if="tab === 'config'" class="pane">
      <!-- EULA 没勾选时给出明确提示（勾了就不占地方） -->
      <p v-if="server && !server.eula" class="warn">{{ $t("server-detail.eula-required") }}</p>
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
    <div v-else-if="tab === 'logs'" ref="logPane" class="pane logs">
      <pre v-for="(l, i) in logs" :key="i" class="ln">{{ l }}</pre>
      <p v-if="!logs.length" class="empty">{{ $t("log-viewer.no-logs") }}</p>
    </div>
    <div v-else class="pane console">
      <pre v-if="consoleOut" class="cout">{{ consoleOut }}</pre>
      <p v-else class="empty">{{ $t("server-detail.console-hint") }}</p>
      <div class="cbar">
        <van-field v-model="consoleInput" class="cinput" :placeholder="$t('server-detail.console-placeholder')" />
        <van-button size="small" class="cgo" :disabled="!consoleInput.trim()" @click="sendCommand">
          {{ $t("server-detail.send") }}
        </van-button>
      </div>
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
/* 联机地址卡：与其它卡一致留白，按钮 34px 触控达标 */
.addr {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  flex-shrink: 0;
}
.addr-l {
  flex: 1;
  min-width: 0;
}
.addr-t {
  font-size: 12px;
  color: var(--text-3);
}
.addr-v {
  margin-top: 3px;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-1);
  word-break: break-all;
}
.addr-b {
  min-height: 34px;
  flex-shrink: 0;
}
.warn {
  margin: 0;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(255, 152, 0, 0.12);
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-2);
  flex-shrink: 0;
}
.addr-hint {
  margin: -4px 2px 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-3);
  flex-shrink: 0;
}
/* 控制台：输出区可滚，输入固定在底部（不被键盘顶飞） */
.console {
  gap: 10px;
}
.cout {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  margin: 0;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.22);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-1);
  white-space: pre-wrap;
  word-break: break-all;
}
.cbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.cinput {
  flex: 1;
  min-width: 0;
}
.cgo {
  min-height: 36px;
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
