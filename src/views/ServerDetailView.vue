<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useMessage } from "../composables/message";
import { useLogZoom } from "../composables/useLogZoom";
import {
  Button as VanButton,
  Tabs as VanTabs,
  Tab as VanTab,
  Field as VanField,
} from "vant";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { useServersStore } from "../stores/servers";
import ServerFileManager from "../components/ServerFileManager.vue";
import AppInput from "../ui/AppInput.vue";
import AppSwitch from "../ui/AppSwitch.vue";

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

/**
 * 日志字号缩放：与实例日志页**共用** `useLogZoom`。
 *
 * 这一页的服务器日志和控制台都是手写的 `<pre>`（不是 `LogViewer`），所以缩放
 * 得单独接一次—— 两处共用同一个字号，在日志与控制台之间切来切去大小一致。
 */
const { fontSize, applyFontSize, zoomBy, touch } = useLogZoom();

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
      startStage.value = $t("server-detail.starting");
      await watchStage();
      await servers.start(s.id);
      message.success($t("multiplayer.server-started"));
    }
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
    startStage.value = "";
  }
}

/**
 * 启动阶段文案（后端 `server://stage` 事件），直接显示在「启动」按钮上。
 *
 * 启动一台服务器可能要几十秒 —— 旧版 Paper 首次要先生成补丁、新版要下几十 MB 原版包 ——
 * 只给一颗转圈的按钮，用户分不清是在干活还是卡死了。后端会把阶段（甚至当前日志行，
 * 如 `Downloading mojang_26.3.jar`）报过来，这里原样显示。
 */
const startStage = ref("");
let unlistenStage: (() => void) | null = null;

async function watchStage() {
  if (unlistenStage) return;
  unlistenStage = await listen<{ id: string; stage: string }>("server://stage", (e) => {
    if (e.payload?.id === serverId) startStage.value = e.payload.stage;
  });
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

/**
 * 同意 / 撤回 Minecraft EULA。
 *
 * 后端一直支持 `update_hosted_server({eula})`，`start_hosted_server` 也会按它写 eula.txt，
 * 但界面上**从来没有这个开关** —— 原来的提示却让用户「去设置里勾选」，用户当然找不到。
 * 这里直接给开关；保存走 `servers.update`，本地列表会同步替换，勾完提示与启动按钮立刻跟着变。
 */
async function toggleEula(v: boolean) {
  const s = server.value;
  if (!s) return;
  try {
    await servers.update({ id: s.id, eula: v });
  } catch (e) {
    message.error(String(e));
  }
}

// ── 对外开房（陶瓦联机 + 自建服）─────────────────────────────────────────
//
// Terracotta 发现「本机有台 MC 服务器」的方式**不是扫端口，而是被动收组播包**
// （官方 scanning.rs：bind 4445 + join_multicast 224.0.2.60，解析 `[MOTD]…[/AD]<端口>[/AD]`）。
// 那个包只有**游戏客户端开局域网世界**时才会发，独立服务端不发 —— 所以只点「开启房间」
// 会一直停在「正在寻找本机开放局域网的游戏…」。这里由我们自己发这个包把端口报出去，
// Terracotta 发现后就会自动建房、生成房间码、并把该端口加进转发白名单。
//
// 端口不必是 25565：Terracotta 转发的是「扫描到的那个端口」。
const shareOn = ref(false);
const shareBusy = ref(false);
const shareRoom = ref("");
const shareErr = ref("");
let shareTimer: ReturnType<typeof setInterval> | null = null;

/** 从 `/state` 的 JSON 里掏房间码（字段名与隧道侧 extractRoomCode 一致） */
function pickRoomCode(raw: string): string {
  for (const key of ['"room"', '"code"', '"room_code"', '"roomCode"']) {
    const i = raw.indexOf(key);
    if (i < 0) continue;
    const colon = raw.indexOf(":", i + key.length);
    if (colon < 0) continue;
    const rest = raw.slice(colon + 1).trimStart();
    if (!rest.startsWith('"')) continue;
    const end = rest.indexOf('"', 1);
    if (end <= 1) continue;
    const v = rest.slice(1, end);
    if (v.trim()) return v.trim();
  }
  return "";
}

function stopSharePolling() {
  if (shareTimer) clearInterval(shareTimer);
  shareTimer = null;
}

/**
 * 调隧道接口，失败自动重试几次。
 *
 * 冷启动应用时隧道要 `loadLibrary` + 初始化 EasyTier（手机上还跑着服务器 JVM 时更慢），
 * 头几秒必然连不上。原来直接把这句错误弹给用户（"隧道还在启动中，请稍后重试"），
 * 用户只能看着干等 —— 这里自己重试 3 次（共约 6 秒），多数情况用户点一下就成了。
 */
async function tunnelCall(path: string, tries = 3): Promise<string> {
  let last = "";
  for (let i = 0; i < tries; i++) {
    try {
      return await api.terracottaTunnelRequest(path);
    } catch (e) {
      last = String(e);
      if (i < tries - 1) await new Promise((r) => setTimeout(r, 2000));
    }
  }
  throw last;
}

/**
 * 开房前确保拿到「VPN」系统授权。
 *
 * 陶瓦联机的数据面必须走 TUN：房主要把 mesh 上的入站流量转成「连本机 25565」，
 * 客人才连得进来。没有 TUN 的症状很隐蔽 —— **房间码能开出来，但客人永远连不上**
 * （电脑端加入时报「连接发生错误」，而房主侧日志里只有自己）。
 *
 * 而且 Terracotta 的 VpnService 请求只有 30 秒答复窗口，系统授权对话框要用户点一下，
 * 所以必须提前问、不能塞在那个窗口里。
 */
async function ensureVpnConsent(): Promise<boolean> {
  try {
    if (await api.terracottaVpnGranted()) return true;
    await api.terracottaRequestVpn();
    // 等用户点「允许」（授权一次后长期有效）
    for (let i = 0; i < 30; i++) {
      await new Promise((r) => setTimeout(r, 1000));
      if (await api.terracottaVpnGranted()) return true;
    }
    message.warning($t("server-detail.share-vpn-denied"));
    return false;
  } catch {
    return await api.terracottaVpnGranted();
  }
}

async function toggleShare(v: boolean) {
  const s = server.value;
  if (!s) return;
  shareBusy.value = true;
  shareErr.value = "";
  try {
    if (!v) {
      stopSharePolling();
      // 顺序有讲究：先停广播，再让 Terracotta 回到等待 —— `/waiting` 里也会顺手停广播，
      // 但先停一次能保证「立刻」不再对外宣称有台服务器。
      await api.terracottaTunnelRequest("/no-advertise").catch(() => "");
      await api.terracottaTunnelRequest("/waiting").catch(() => "");
      shareOn.value = false;
      shareRoom.value = "";
      return;
    }
    if (!running.value) {
      shareOn.value = false;
      message.warning($t("server-detail.share-need-running"));
      return;
    }
    // 没 VPN 授权就别开房：否则房间码出得来、朋友却怎么都连不上
    if (!(await ensureVpnConsent())) {
      shareOn.value = false;
      return;
    }
    const name = s.name || $t("server-detail.share");
    // 先广播、再让 Terracotta 去扫；顺序反了它这次扫描就白扫了。
    await tunnelCall(`/advertise?port=${s.port}&name=${encodeURIComponent(name)}`);
    await tunnelCall(`/host?player=${encodeURIComponent(name)}`);
    shareOn.value = true;
    shareRoom.value = "";
    // 房间码要等 Terracotta 扫到我们的广播才出现（几秒），这里轮询拿
    let waited = 0;
    shareTimer = setInterval(async () => {
      waited += 1500;
      try {
        const raw = await api.terracottaTunnelRequest("/state");
        const code = pickRoomCode(raw);
        if (code) {
          shareRoom.value = code;
          stopSharePolling();
          message.success($t("server-detail.share-ok"));
        } else if (waited >= 45000) {
          stopSharePolling();
          shareErr.value = $t("server-detail.share-no-room");
        }
      } catch (e) {
        if (waited >= 45000) {
          stopSharePolling();
          shareErr.value = String(e);
        }
      }
    }, 1500);
  } catch (e) {
    shareOn.value = false;
    shareErr.value = String(e);
    message.error(String(e));
  } finally {
    shareBusy.value = false;
  }
}

async function copyShareRoom() {
  try {
    await navigator.clipboard.writeText(shareRoom.value);
    message.success($t("terracotta.copied"));
  } catch {
    message.error($t("terracotta.copy-failed"));
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
  unlistenStage?.();
  unlistenStage = null;
  stopSharePolling();
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
      {{
        running
          ? $t("multiplayer.stop")
          : busy === "run" && startStage
            ? startStage
            : $t("server-detail.start")
      }}
    </van-button>
    <p v-if="!coreReady && !running" class="addr-hint">{{ $t("server-detail.memory-hint") }}</p>

    <van-tabs v-model:active="tab" class="tabs" @change="tab !== 'config' && loadLogs()">
      <van-tab name="config" :title="$t('router.settings')" />
      <van-tab name="files" :title="$t('instance-detail.files')" />
      <van-tab name="logs" :title="$t('common.logs')" />
      <van-tab name="console" :title="$t('server-detail.console')" />
    </van-tabs>
    <div v-if="tab === 'config'" class="pane">
      <!-- EULA 开关：这里点，不用去别处找。勾上后端会把服务端目录里的 eula.txt 改成 true，
           启动前还会再兜一次（见 servers.rs::start_hosted_server）。 -->
      <div class="choice">
        <div class="choice-info">
          <span class="choice-label">{{ $t("server-detail.eula") }}</span>
          <p class="choice-hint">
            {{ server?.eula ? $t("server-detail.eula-done") : $t("server-detail.eula-required") }}
          </p>
        </div>
        <app-switch :value="!!server?.eula" @update:value="toggleEula" />
      </div>
      <!-- 对外开房：把本机这台服务器经陶瓦联机暴露给朋友（端口可以自定义） -->
      <div class="choice">
        <div class="choice-info">
          <span class="choice-label">{{ $t("server-detail.share") }}</span>
          <p class="choice-hint">
            <template v-if="shareRoom">{{ $t("server-detail.share-room", { code: shareRoom }) }}</template>
            <template v-else-if="shareErr">{{ shareErr }}</template>
            <template v-else-if="shareOn">{{ $t("server-detail.share-waiting") }}</template>
            <template v-else>{{ $t("server-detail.share-hint") }}</template>
          </p>
        </div>
        <app-switch
          :value="shareOn"
          :disabled="shareBusy"
          @update:value="toggleShare"
        />
      </div>
      <div v-if="shareRoom" class="tc-actions">
        <van-button size="small" @click="copyShareRoom">{{ $t("terracotta.copy") }}</van-button>
      </div>
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
      <van-button class="save" block type="primary" :loading="busy === 'save'" @click="save">{{ $t("common.save") }}</van-button>
    </div>
    <div v-else-if="tab === 'files'" class="pane"><ServerFileManager :server-id="serverId" /></div>
    <div v-else-if="tab === 'logs'" class="pane logs">
      <!-- 字号那一组：A− / 当前字号 / A+，与实例日志页同一套（useLogZoom） -->
      <div class="zoombar">
        <button class="zbtn" :title="$t('log-viewer.zoom-out')" @click="zoomBy(-1)">A−</button>
        <button class="zbtn zval" :title="$t('log-viewer.zoom-reset')" @click="applyFontSize(12)">{{ fontSize }}</button>
        <button class="zbtn" :title="$t('log-viewer.zoom-in')" @click="zoomBy(1)">A+</button>
      </div>
      <div
        ref="logPane"
        class="log-scroll"
        :style="{ fontSize: fontSize + 'px' }"
        @touchstart.passive="touch.onTouchStart"
        @touchmove.passive="touch.onTouchMove"
        @touchend="touch.onTouchEnd"
        @touchcancel="touch.onTouchEnd"
      >
        <pre v-for="(l, i) in logs" :key="i" class="ln">{{ l }}</pre>
        <p v-if="!logs.length" class="empty">{{ $t("log-viewer.no-logs") }}</p>
      </div>
    </div>
    <div v-else class="pane console">
      <pre v-if="consoleOut" class="cout" :style="{ fontSize: fontSize + 'px' }">{{ consoleOut }}</pre>
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
  /* 字号由 useLogZoom 绑上来，不在这里写死 */
  overflow-y: auto;
  margin: 0;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.22);
  font-family: "Cascadia Code", Consolas, monospace;
  /* 字号由 useLogZoom 的 fontSize 绑上来（这行只是兜底默认值） */
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
/* tab 条圆角胶囊（与下载页「下载中 / 已下载」同一套观感）：Vant 默认直角，手机上太硬 */
.tabs :deep(.van-tabs__wrap) {
  border-radius: 12px;
  overflow: hidden;
}
.tabs :deep(.van-tab) {
  min-height: 42px;
  align-items: center;
}
/* EULA 开关行：与设置页的 .choice-row 同一套排法 */
.choice {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  flex-shrink: 0;
}
.choice-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.choice-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-1);
}
.choice-hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-3);
}
/* 保存按钮：`.pane` 是列方向 flex，按钮默认 flex-shrink:1 会被上面的字段挤成一条细长条 */
.save {
  flex-shrink: 0;
  min-height: 44px;
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
/* 日志页：`.pane` 自己不再滚，滚动交给里面的 `.log-scroll`，
   这样顶上的字号按钮条才能固定不动（原来整页一起滚，按钮会跟着跑）。 */
.pane.logs {
  overflow: hidden;
  gap: 8px;
}
.zoombar {
  display: flex;
  gap: 6px;
  flex: 0 0 auto;
}
.zbtn {
  min-width: 40px;
  min-height: 32px;
  padding: 0 8px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.zbtn.zval {
  min-width: 34px;
  color: var(--text-3);
  font-size: 12px;
}
.log-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  user-select: text;
  -webkit-user-select: text;
  /* 字号由 useLogZoom 的 fontSize 绑上来；这里给个兜底值，
     免得 JS 没跑起来时pre 继承成16px 把布局撑乱 */
  font-size: 12px;
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
  /* 原来写死 11px，会盖掉 useLogZoom 绑上来的字号 -> 只留兜底值，
     真实字号由 .log-scroll 的 inline style 控制（改这里等于没改） */
  font-size: 1em;
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
