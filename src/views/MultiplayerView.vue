<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { fmtBytes } from "../utils/format";
import { CORE_COLORS, CORE_LABELS } from "../utils/cores";
import { isAprilFools } from "../utils/versions";
import { useRouter } from "vue-router";
import { NInput, NProgress, useMessage } from "naive-ui";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../api";
import { useAccountsStore } from "../stores/accounts";
import { useServersStore } from "../stores/servers";
import type { ServerCore, TerracottaDownloadProgress, TerracottaInfo } from "../types";
import AppSheet from "../ui/AppSheet.vue";
import {
  IconBox,
  IconCheck,
  IconClose,
  IconCopy,
  IconDoorOpen,
  IconDownload,
  IconExternal,
  IconPlay,
  IconPlus,
  IconRefresh,
  IconServer,
  IconStop,
  IconTrash,
  IconUsers,
} from "../components/icons";

const router = useRouter();
const accounts = useAccountsStore();
const servers = useServersStore();
const message = useMessage();

const tab = ref<"servers" | "rooms">("servers");

/** 托管服务器 / 联机房间后端未实现，先关闭入口（详见模板里的注释）。 */
const HOSTED_SERVER_ENABLED = false;

// ---- 版本清单 ----
const versions = ref<{ id: string; type: string; releaseTime: string }[]>([]);
const versionCat = ref<string>("release");

const filteredVersions = computed(() =>
  versions.value.filter((v) => {
    if (versionCat.value === "release") return v.type === "release" || v.type.startsWith("old_");
    if (versionCat.value === "april") return isAprilFools(v);
    return v.type === "snapshot" && !isAprilFools(v);
  }),
);

// ---- 服务器核心选项 ----
const CORES: { value: ServerCore; label: string; desc: string }[] = [
  { value: "vanilla", label: "Vanilla", desc: $t("multiplayer.vanilla") },
  { value: "paper", label: "Paper", desc: $t("multiplayer.paper") },
  { value: "spigot", label: "Spigot", desc: $t("multiplayer.spigot") },
  { value: "purpur", label: "Purpur", desc: $t("multiplayer.purpur") },
  { value: "forge", label: "Forge", desc: $t("multiplayer.forge") },
  { value: "fabric", label: "Fabric", desc: $t("multiplayer.fabric") },
];

// ---- 创建对话框（仅名称 / 核心 / 版本）----
type ServerDialog = {
  name: string;
  core: ServerCore;
  mc_version: string;
};

const dialog = ref<ServerDialog | null>(null);
const saving = ref(false);

// ---- 核心下载进度 ----
const installing = ref<{ name: string; phase: string; done: number; total: number } | null>(null);

const installPct = computed(() => {
  const ins = installing.value;
  if (!ins || ins.total <= 0) return 0;
  return Math.min(100, Math.round((ins.done / ins.total) * 100));
});
const installHasBytes = computed(() => {
  const ins = installing.value;
  return !!ins && ins.total > 1;
});
function openCreate() {
  versionCat.value = "release";
  dialog.value = { name: "", core: "paper", mc_version: "" };
}

const canSave = computed(() => {
  const d = dialog.value;
  return !!d && !!d.core && !!d.mc_version;
});

async function saveDialog() {
  const d = dialog.value;
  if (!d) return;
  if (!d.mc_version) return message.warning($t("create-instance.select-version"));
  saving.value = true;
  try {
    const s = await servers.create(d.name, d.core, d.mc_version);
    dialog.value = null;
    saving.value = false;

    installing.value = { name: s.name, phase: $t("multiplayer.phase"), done: 0, total: 0 };
    const un = await listen<{ serverId: string; phase: string; done: number; total: number }>(
      "server://install-progress",
      (ev) => {
        if (installing.value) {
          installing.value = {
            ...installing.value,
            phase: ev.payload.phase,
            done: ev.payload.done ?? 0,
            total: ev.payload.total ?? 0,
          };
        }
      },
    );
    try {
      await servers.installCore(s.id);
      message.success($t("multiplayer.core-ready", { p1: s.name }));
    } catch (e) {
      message.error(String(e));
    }
    un();
    installing.value = null;
    router.push(`/multiplayer/${s.id}`);
  } catch (e) {
    message.error(String(e));
    saving.value = false;
    installing.value = null;
  }
}

// ---- 删除确认 ----
const confirmState = ref<{ id: string; name: string } | null>(null);
function confirmDelete(id: string, name: string) {
  confirmState.value = { id, name };
}
async function doDelete() {
  const c = confirmState.value;
  if (!c) return;
  try {
    await servers.remove(c.id);
    message.success($t("multiplayer.deleted", { p1: c.name }));
  } catch (e) {
    message.error(String(e));
  }
  confirmState.value = null;
}

// ---- 启动 / 停止 ----
async function toggleRun(id: string, running: boolean) {
  try {
    if (running) {
      await servers.stop(id);
      message.success($t("multiplayer.server-stopped"));
    } else {
      await servers.start(id);
      message.success($t("multiplayer.server-started"));
    }
  } catch (e) {
    message.error(String(e));
  }
}

// ---- 陶瓦联机（联机房间）----
const tc = ref<TerracottaInfo | null>(null);
const tcLoading = ref(false);
const tcBusy = ref(false);
const tcRoomCode = ref("");
// 创建房间的玩家名，默认读取当前账号名
const tcPlayerName = ref("");
const tcRoom = ref<Record<string, unknown> | null>(null);
const tcJoined = ref(false);
const tcDownloading = ref(false);
const tcDownloadProgress = ref<TerracottaDownloadProgress | null>(null);
let tcDlUnlisten: (() => void) | null = null;
let tcPollTimer: number | null = null;

const tcStateName = computed<string>(() => String(tcRoom.value?.state ?? "waiting"));
const tcRoomCodeFinal = computed<string>(() => {
  const s = tcRoom.value;
  if (!s) return "";
  const room = s.room;
  if (room && typeof room === "object") {
    const code = (room as Record<string, unknown>).code;
    if (code) return String(code);
  }
  return String(s.room ?? "");
});
const tcUrl = computed<string>(() => String(tcRoom.value?.url ?? ""));

// 解析玩家列表（profiles 数组），自适应常见字段名
const tcPlayers = computed<Record<string, unknown>[]>(() => {
  const raw = tcRoom.value?.profiles;
  if (!Array.isArray(raw)) return [];
  return raw.filter((p): p is Record<string, unknown> => !!p && typeof p === "object");
});
const playerName = (p: Record<string, unknown>): string =>
  String(p.name ?? p.username ?? p.playerName ?? p.displayName ?? $t("multiplayer.string"));
const playerUuid = (p: Record<string, unknown>): string =>
  String(p.uuid ?? p.id ?? "");
const playerPing = (p: Record<string, unknown>): string => {
  const v = p.ping ?? p.latency;
  return v === undefined || v === null ? "" : String(v);
};
const playerDesc = (p: Record<string, unknown>): string =>
  String(p.description ?? p.desc ?? p.comment ?? "");

const tcStateText = computed(() => {
  const map: Record<string, string> = {
    waiting: $t("multiplayer.waiting"),
    "host-scanning": $t("multiplayer.host-scanning"),
    "host-starting": $t("multiplayer.host-starting"),
    "host-ok": $t("multiplayer.host-ok"),
    "guest-connecting": $t("multiplayer.guest-connecting"),
    "guest-starting": $t("multiplayer.guest-starting"),
    "guest-ok": $t("multiplayer.guest-ok"),
    exception: $t("multiplayer.exception"),
  };
  return map[tcStateName.value] ?? $t("multiplayer.computed");
});

async function refreshTc() {
  tcLoading.value = true;
  try {
    tc.value = await api.terracottaDetect();
    if (tc.value?.running) startTcPoll();
  } catch (e) {
    message.error(String(e));
  } finally {
    tcLoading.value = false;
  }
}

function openTcDownload() {
  const url = tc.value?.download_url;
  if (url) openUrl(url).catch(() => {});
}

const tcDownloadPercent = computed(() => {
  const p = tcDownloadProgress.value;
  if (!p) return 0;
  if (p.done || p.extracting) return 100;
  if (p.percent >= 0) return Math.min(100, p.percent);
  return 0;
});

const tcDownloadText = computed(() => {
  const p = tcDownloadProgress.value;
  if (!p) return $t("multiplayer.tc-download-text");
  if (p.extracting) return $t("multiplayer.xia-zai-wan-cheng-zheng-zai");
  if (p.done) return $t("install-dialog.computed");
  if (p.total > 0) {
    const mb = (v: number) => (v / 1024 / 1024).toFixed(1);
    return `${tcDownloadPercent.value}%（${mb(p.downloaded)} / ${mb(p.total)} MB）`;
  }
  return $t("multiplayer.resolving-address");
});

async function downloadTc() {
  if (tcDownloading.value || tcBusy.value) return;
  tcDownloading.value = true;
  tcDownloadProgress.value = { downloaded: 0, total: 0, percent: 0 };
  try {
    tcDlUnlisten = await listen<TerracottaDownloadProgress>(
      "terracotta://download-progress",
      (ev) => {
        tcDownloadProgress.value = ev.payload;
      },
    );
    await api.terracottaDownload();
    tcDownloadProgress.value = { downloaded: 1, total: 1, percent: 100, done: true };
    message.success($t("multiplayer.terracotta-installed"));
    await refreshTc();
  } catch (e) {
    message.error(String(e));
  } finally {
    if (tcDlUnlisten) {
      tcDlUnlisten();
      tcDlUnlisten = null;
    }
    tcDownloading.value = false;
  }
}

async function launchTc() {
  if (tcBusy.value) return;
  tcBusy.value = true;
  try {
    const l = await api.terracottaLaunch();
    if (tc.value) tc.value = { ...tc.value, running: true, port: l.port };
    startTcPoll();
    message.success($t("multiplayer.terracotta-started"));
  } catch (e) {
    message.error(String(e));
  } finally {
    tcBusy.value = false;
  }
}

async function stopTc() {
  try {
    await api.terracottaStop();
    stopTcPoll();
    if (tc.value) tc.value = { ...tc.value, running: false, port: null };
    tcRoom.value = null;
    message.success($t("multiplayer.terracotta-stopped"));
  } catch (e) {
    message.error(String(e));
  }
}

function openTcUi() {
  const p = tc.value?.port;
  if (p) openUrl(`http://127.0.0.1:${p}/`).catch(() => {});
}

// 正在进行中的状态：轮询时不会被空闲(waiting/空)状态覆盖
const TC_PENDING_STATES = new Set([
  "host-scanning",
  "host-starting",
  "guest-connecting",
  "guest-starting",
]);

async function pollTcState() {
  try {
    const s = (await api.terracottaStatus()) as Record<string, unknown> | null;
    const next = String(s?.state ?? "waiting");
    // 本地正处于进行中状态，而陶瓦返回空闲/空，说明还没进入对应状态，
    // 保留本地状态，避免界面掉回「创建/加入」面板
    if (TC_PENDING_STATES.has(tcStateName.value) && next === "waiting") {
      return;
    }
    tcRoom.value = s;
  } catch {
    // 轮询失败忽略，等待下一次
  }
}

function startTcPoll() {
  stopTcPoll();
  pollTcState();
  tcPollTimer = window.setInterval(pollTcState, 1500);
}

function stopTcPoll() {
  if (tcPollTimer !== null) {
    window.clearInterval(tcPollTimer);
    tcPollTimer = null;
  }
}

async function createRoom() {
  if (tcBusy.value) return;
  tcBusy.value = true;
  // 点击后立即进入扫描引导，避免等待轮询期间掉回创建/加入面板
  tcRoom.value = { state: "host-scanning" };
  try {
    const name = tcPlayerName.value.trim();
    await api.terracottaCreateRoom(name || undefined);
    await pollTcState();
  } catch (e) {
    tcRoom.value = null;
    message.error(String(e));
  } finally {
    tcBusy.value = false;
  }
}

async function joinRoom() {
  if (tcBusy.value) return;
  const code = tcRoomCode.value.trim();
  if (!code) return message.warning($t("multiplayer.room-code-required"));
  tcBusy.value = true;
  // 点击后立即进入连接引导，避免等待轮询期间掉回创建/加入面板
  tcRoom.value = { state: "guest-connecting" };
  try {
    const name = tcPlayerName.value.trim();
    await api.terracottaJoinRoom(code, name || undefined);
    await pollTcState();
  } catch (e) {
    tcRoom.value = null;
    message.error(String(e));
  } finally {
    tcBusy.value = false;
  }
}

async function leaveRoom() {
  try {
    await api.terracottaLeave();
    tcRoom.value = null;
    message.success($t("multiplayer.left-room"));
  } catch (e) {
    message.error(String(e));
  }
}

async function copyRoomCode() {
  if (!tcRoomCodeFinal.value) return;
  try {
    await navigator.clipboard.writeText(tcRoomCodeFinal.value);
    tcJoined.value = true;
    setTimeout(() => (tcJoined.value = false), 1600);
  } catch {
    // 忽略剪贴板失败
  }
}

// 默认读取当前账号名，切换账号时若玩家名未被手动修改则同步更新
watch(
  () => accounts.current?.username ?? "",
  (name) => {
    if (name && (!tcPlayerName.value || tcPlayerName.value === "")) tcPlayerName.value = name;
  },
  { immediate: true },
);

onMounted(async () => {
  servers.load();
  refreshTc();
  try {
    const m = await api.getVersionManifest();
    versions.value = m.versions;
  } catch (e) {
    message.error(String(e));
  }
});

// 标题栏的"创建服务器"按钮仅在"服务器"标签页显示
watch(tab, (v) => servers.setCanCreate(v === "servers"), { immediate: true });
onUnmounted(() => servers.setCanCreate(false));

// 响应标题栏的"创建服务器"按钮
watch(
  () => servers.createRequest,
  (n) => {
    if (n > 0) openCreate();
  },
);

onUnmounted(() => stopTcPoll());
</script>

<template>
  <div class="mp-view">
    <div class="mode-tabs glass">
      <button :class="{ active: tab === 'servers' }" @click="tab = 'servers'">
        <IconServer />{{ $t("home.server") }}</button>
      <!-- 「联机房间」整条链路依赖后端，而 `*_hosted_server*` / 房间系列命令在
           api.ts 里整批标注为**未实现**（Rust 侧零实现）。与其让用户点进一个
           点什么都报错的页面，先把入口隐藏；功能补齐后把 HOSTED_SERVER_ENABLED
           打开即可恢复（下面的面板代码保留原样）。 -->
      <button
        v-if="HOSTED_SERVER_ENABLED"
        :class="{ active: tab === 'rooms' }"
        @click="tab = 'rooms'"
      >
        <IconUsers />{{ $t("multiplayer.rooms") }}</button>
    </div>

    <!-- 服务器 -->
    <template v-if="tab === 'servers'">
      <div v-if="servers.count" class="grid">
        <div
          v-for="s in servers.servers"
          :key="s.id"
          class="server-card glass clickable"
          @click="router.push(`/multiplayer/${s.id}`)"
        >
          <div class="card-head">
            <span
              class="core-badge"
              :style="{ color: CORE_COLORS[s.core], borderColor: CORE_COLORS[s.core] }"
            >
              {{ CORE_LABELS[s.core] }}
            </span>
            <h3 class="card-name">{{ s.name }}</h3>
          </div>
          <div class="card-meta">
            <div class="meta-row"><span>{{ $t("multiplayer.version") }}</span><b class="mono">{{ s.mc_version }}</b></div>
            <div class="meta-row"><span>{{ $t("multiplayer.port") }}</span><b>{{ s.port }}</b></div>
          </div>
          <div class="card-motd">{{ s.motd || "A Minecraft Server" }}</div>
          <div class="card-foot" @click.stop>
            <span class="status" :class="{ on: servers.isRunning(s.id) }">
              {{ servers.isRunning(s.id) ? $t('multiplayer.running') : $t('multiplayer.not-running') }}
            </span>
            <div class="ops">
              <button
                class="op"
                :class="servers.isRunning(s.id) ? 'stop' : 'start'"
                @click="toggleRun(s.id, servers.isRunning(s.id))"
              >
                <IconStop v-if="servers.isRunning(s.id)" />
                <IconPlay v-else />
                {{ servers.isRunning(s.id) ? $t('multiplayer.stop') : $t('instance-saves.launch') }}
              </button>
              <button class="op danger" :title="$t('common.delete')" :aria-label="$t('common.delete')" @click="confirmDelete(s.id, s.name)">
                <IconTrash />
              </button>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="empty glass">
        <div class="empty-icon"><IconServer /></div>
        <p>{{ $t("multiplayer.no-servers") }}</p>
        <button class="btn primary" @click="openCreate">{{ $t("multiplayer.create-first") }}</button>
      </div>
    </template>

    <!-- 联机房间（陶瓦联机） -->
    <template v-else>
      <!-- 检测中 -->
      <div v-if="tcLoading || !tc" class="empty glass">
        <div class="empty-icon"><IconRefresh /></div>
        <p>{{ $t("multiplayer.detecting") }}</p>
      </div>

      <!-- 未安装 -->
      <div v-else-if="!tc.found" class="tc-download">
        <div class="tc-hero glass">
          <div v-if="tc.icon" class="tc-hero-icon tc-icon-img">
            <img :src="tc.icon" :alt="$t('multiplayer.terracotta')" />
          </div>
          <div class="tc-hero-text">
            <h2>{{ $t("multiplayer.terracotta") }}</h2>
            <p>{{ $t("multiplayer.terracotta-desc") }}</p>
          </div>
        </div>

        <div class="glass tc-dl-card">
          <div class="tc-dl-title"><IconDownload />{{ $t("multiplayer.need-terracotta") }}</div>
          <p class="tc-dl-desc">{{ $t("multiplayer.terracotta-missing") }}</p>

          <div v-if="tcDownloading" class="tc-dl-progress">
            <div class="tc-dl-bar">
              <div class="tc-dl-fill" :style="{ width: tcDownloadPercent + '%' }"></div>
            </div>
            <span class="tc-dl-text">{{ tcDownloadText }}</span>
          </div>
          <div v-else class="tc-dl-actions">
            <button class="btn primary" :disabled="tcLoading" @click="downloadTc">
              <IconDownload />{{ $t("multiplayer.download-terracotta") }}</button>
            <button class="btn ghost" :disabled="tcLoading" @click="refreshTc">
              <IconRefresh />{{ $t("multiplayer.re-detect") }}</button>
          </div>

          <p class="tc-hint">{{ $t("multiplayer.install-path-hint") }}<code>%LOCALAPPDATA%\Programs\Terracotta\Terracotta.exe</code>
          </p>
        </div>
      </div>

      <!-- 已安装未运行 -->
      <div v-else-if="!tc.running" class="tc-download">
        <div class="tc-hero glass">
          <div v-if="tc.icon" class="tc-hero-icon tc-icon-img">
            <img :src="tc.icon" :alt="$t('multiplayer.terracotta')" />
          </div>
          <div class="tc-hero-text">
            <h2>{{ $t("multiplayer.terracotta") }}</h2>
            <p>{{ $t("multiplayer.terracotta-ready") }}</p>
          </div>
        </div>

        <div class="tc-actions">
          <button class="tc-action glass" :disabled="tcBusy" @click="launchTc">
            <div class="tc-action-icon"><IconPlay /></div>
            <div class="tc-action-name">{{ $t("multiplayer.launch-terracotta") }}</div>
            <div class="tc-action-desc">{{ $t("multiplayer.launch-terracotta-hint") }}</div>
          </button>
          <button class="tc-action glass" @click="openTcDownload">
            <div class="tc-action-icon"><IconDownload /></div>
            <div class="tc-action-name">{{ $t("instance-content.check-updates") }}</div>
            <div class="tc-action-desc">{{ $t("multiplayer.terracotta-releases") }}</div>
          </button>
        </div>
      </div>

      <!-- 运行中 -->
      <div v-else class="tc-download">
        <div class="tc-hero glass">
          <div v-if="tc.icon" class="tc-hero-icon tc-icon-img">
            <img :src="tc.icon" :alt="$t('multiplayer.terracotta')" />
          </div>
          <div class="tc-hero-text">
            <h2>{{ $t("multiplayer.terracotta") }}</h2>
            <p class="tc-state">{{ tcStateText }}</p>
          </div>
          <div class="tc-hero-ops">
            <button class="op" @click="openTcUi"><IconExternal />{{ $t("multiplayer.open-ui") }}</button>
            <button class="op stop" @click="stopTc"><IconStop />{{ $t("multiplayer.stop") }}</button>
          </div>
        </div>

        <!-- 主机：房间已创建 -->
        <div v-if="tcStateName === 'host-ok'" class="glass tc-code-card">
          <div class="tc-code-label"><IconUsers />{{ $t("multiplayer.room-created") }}</div>
          <div class="tc-code-value mono">{{ tcRoomCodeFinal }}</div>
          <p class="tc-code-hint">{{ $t("multiplayer.share-code-hint") }}</p>
          <div class="tc-code-actions">
            <button class="btn primary" @click="copyRoomCode">
              <IconCopy /> {{ tcJoined ? $t('crash-analyzer.copied') : $t('multiplayer.copy-room-code') }}
            </button>
            <button class="btn ghost" @click="leaveRoom"><IconClose />{{ $t("multiplayer.leave-room") }}</button>
          </div>
        </div>

        <!-- 主机：玩家列表 -->
        <div v-if="tcStateName === 'host-ok' && tcPlayers.length" class="glass tc-players-card">
          <div class="tc-players-head">
            <span class="tc-players-title"><IconUsers />{{ $t("multiplayer.room-players") }}</span>
            <span class="tc-players-count">{{ tcPlayers.length }}</span>
          </div>
          <div class="tc-players-list">
            <div v-for="(p, i) in tcPlayers" :key="i" class="tc-player-row">
              <span class="tc-player-avatar">{{ (playerName(p) || "?").slice(0, 1) }}</span>
              <div class="tc-player-info">
                <div class="tc-player-name">{{ playerName(p) }}</div>
                <div v-if="playerUuid(p)" class="tc-player-uuid">{{ playerUuid(p) }}</div>
              </div>
              <div v-if="playerDesc(p)" class="tc-player-desc">{{ playerDesc(p) }}</div>
              <span v-if="playerPing(p)" class="tc-player-ping">{{ playerPing(p) }}ms</span>
            </div>
          </div>
        </div>

        <!-- 访客：已加入 -->
        <div v-if="tcStateName === 'guest-ok'" class="glass tc-code-card">
          <div class="tc-code-label"><IconCheck />{{ $t("multiplayer.guest-ok") }}</div>
          <p class="tc-code-hint">{{ $t("multiplayer.direct-connect-hint") }}<b class="mono">{{ tcUrl || "127.0.0.1" }}</b>
          </p>
          <div class="tc-code-actions">
            <button class="btn ghost" @click="leaveRoom"><IconClose />{{ $t("multiplayer.leave-room") }}</button>
          </div>
        </div>

        <!-- 访客：玩家列表 -->
        <div
          v-if="tcStateName === 'guest-ok' && tcPlayers.length"
          class="glass tc-players-card"
        >
          <div class="tc-players-head">
            <span class="tc-players-title"><IconUsers />{{ $t("multiplayer.room-players") }}</span>
            <span class="tc-players-count">{{ tcPlayers.length }}</span>
          </div>
          <div class="tc-players-list">
            <div v-for="(p, i) in tcPlayers" :key="i" class="tc-player-row">
              <span class="tc-player-avatar">{{ (playerName(p) || "?").slice(0, 1) }}</span>
              <div class="tc-player-info">
                <div class="tc-player-name">{{ playerName(p) }}</div>
                <div v-if="playerUuid(p)" class="tc-player-uuid">{{ playerUuid(p) }}</div>
              </div>
              <div v-if="playerDesc(p)" class="tc-player-desc">{{ playerDesc(p) }}</div>
              <span v-if="playerPing(p)" class="tc-player-ping">{{ playerPing(p) }}ms</span>
            </div>
          </div>
        </div>

        <!-- 访客：连接中 -->
        <div
          v-if="tcStateName === 'guest-connecting' || tcStateName === 'guest-starting'"
          class="glass tc-scan-card"
        >
          <div class="tc-scan-head">
            <span class="tc-scan-spinner"></span>
            <div>
              <div class="tc-scan-title">{{ tcStateText }}</div>
              <p class="tc-scan-sub">{{ $t("multiplayer.connecting") }}</p>
            </div>
          </div>
        </div>

        <!-- 扫描中：引导对局域网开放 -->
        <div
          v-if="tcStateName === 'host-scanning' || tcStateName === 'host-starting'"
          class="glass tc-scan-card"
        >
          <div class="tc-scan-head">
            <span class="tc-scan-spinner"></span>
            <div>
              <div class="tc-scan-title">{{ $t("multiplayer.host-scanning") }}</div>
              <p class="tc-scan-sub">{{ $t("multiplayer.waiting-lan") }}</p>
            </div>
          </div>
          <div class="tc-scan-steps">
            <div class="tc-scan-step">
              <span class="tc-step-num">1</span>
              <span>{{ $t("multiplayer.launch-mc") }}</span>
            </div>
            <div class="tc-scan-step">
              <span class="tc-step-num">2</span>
              <span>{{ $t("multiplayer.press") }}<b>ESC</b>{{ $t("multiplayer.open-pause-menu") }}</span>
            </div>
            <div class="tc-scan-step">
              <span class="tc-step-num">3</span>
              <span>{{ $t("multiplayer.click") }}<b>{{ $t("multiplayer.open-to-lan") }}</b>{{ $t("multiplayer.keep-port") }}</span>
            </div>
          </div>
          <p class="tc-scan-tip">{{ $t("multiplayer.lan-auto-hint") }}</p>
        </div>

        <!-- 空闲：创建 / 加入 -->
        <div v-if="tcStateName === 'waiting'" class="glass tc-room-controls">
          <div class="tc-name-input">
            <label class="tc-field-label">{{ $t("multiplayer.player-name") }}</label>
            <n-input
              v-model:value="tcPlayerName"
              :placeholder="$t('multiplayer.player-name-hint')"
              maxlength="16"
              clearable
            />
          </div>
          <div class="tc-control-grid">
            <div class="tc-col">
              <div class="tc-sub-title"><IconBox />{{ $t("multiplayer.create-room") }}</div>
              <p class="tc-sub-desc">{{ $t("multiplayer.create-room-hint") }}</p>
              <button class="btn primary" :disabled="tcBusy" @click="createRoom">
                <IconPlus />{{ $t("multiplayer.create-room") }}</button>
            </div>
            <div class="tc-divider"></div>
            <div class="tc-col">
              <div class="tc-sub-title"><IconDoorOpen />{{ $t("multiplayer.join-room") }}</div>
              <p class="tc-sub-desc">{{ $t("multiplayer.join-room-hint") }}</p>
              <div class="tc-join-input">
                <n-input
                  v-model:value="tcRoomCode"
                  :placeholder="$t('multiplayer.room-code-example')"
                  @keyup.enter="joinRoom"
                />
                <button class="btn primary" :disabled="tcBusy" @click="joinRoom">
                  <IconPlay />{{ $t("multiplayer.join") }}</button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 第三方版权标注（AGPL 例外条款要求） -->
      <div class="tc-license">{{ $t("multiplayer.powered-by") }}<a href="https://github.com/burningtnt/Terracotta" target="_blank" rel="noopener">
          Terracotta
        </a>
        |
        <a href="https://github.com/burningtnt/Terracotta" target="_blank" rel="noopener">{{ $t("multiplayer.terracotta") }}</a>{{ $t("multiplayer.powered-by-brand") }}</div>
    </template>

    <!-- 创建服务器（仅名称 / 核心 / 版本）-->
    <app-sheet
      :show="dialog !== null"
      :title="$t('title-bar.new-server')"
      @update:show="(v: boolean) => { if (!v) dialog = null; }"
    >
      <div v-if="dialog" class="dialog-body">
        <div class="field">
          <label>{{ $t("multiplayer.server-name") }}</label>
          <n-input v-model:value="dialog.name" :placeholder="$t('multiplayer.server-name-hint')" maxlength="40" />
        </div>

        <div class="field">
          <label>{{ $t("multiplayer.server-core") }}</label>
          <div class="core-grid">
            <button
              v-for="c in CORES"
              :key="c.value"
              class="core-btn"
              :class="{ active: dialog.core === c.value }"
              @click="dialog.core = c.value"
            >
              <span class="core-name">{{ c.label }}</span>
              <span class="core-desc">{{ c.desc }}</span>
            </button>
          </div>
        </div>

        <div class="field">
          <label>{{ $t("browse.game-version") }}</label>
          <div class="ver-cats">
            <button
              v-for="c in [
                { key: 'release', label: $t('install-dialog.release') },
                { key: 'snapshot', label: $t('create-instance.snapshots') },
                { key: 'april', label: $t('create-instance.april-fools') },
              ]"
              :key="c.key"
              :class="{ active: versionCat === c.key }"
              @click="versionCat = c.key"
            >
              {{ c.label }}
            </button>
          </div>
          <div class="ver-list">
            <button
              v-for="v in filteredVersions"
              :key="v.id"
              class="ver-item"
              :class="{ active: dialog.mc_version === v.id }"
              @click="dialog.mc_version = v.id"
            >
              <span class="ver-id mono">{{ v.id }}</span>
              <span class="ver-type">{{ v.type === "release" || v.type.startsWith("old_") ? $t('create-instance.release') : $t('create-instance.snapshot') }}</span>
            </button>
            <div v-if="!filteredVersions.length" class="ver-empty">{{ $t("create-instance.no-versions-in-category") }}</div>
          </div>
        </div>

        <div class="dialog-foot">
          <button class="btn ghost" @click="dialog = null">{{ $t("common.cancel") }}</button>
          <button class="btn primary" :disabled="!canSave || saving" @click="saveDialog">
            <IconPlus /> {{ saving ? $t('create-instance.creating') : $t('multiplayer.create') }}
          </button>
        </div>
      </div>
    </app-sheet>

    <!-- 删除确认 -->
    <app-sheet
      :show="confirmState !== null"
      :title="$t('multiplayer.delete-server')"
      @update:show="(v: boolean) => { if (!v) confirmState = null; }"
    >
      <div v-if="confirmState" class="confirm-body">
        <p class="confirm-text">{{ $t("multiplayer.delete-server-confirm-a") }}<b>{{ confirmState.name }}</b>{{ $t("multiplayer.delete-server-confirm-b") }}</p>
        <div class="dialog-foot">
          <button class="btn ghost" @click="confirmState = null">{{ $t("common.cancel") }}</button>
          <button class="btn danger" @click="doDelete">{{ $t("common.delete") }}</button>
        </div>
      </div>
    </app-sheet>

    <!-- 核心下载进度 -->
    <app-sheet
      :show="installing !== null"
      :title="$t('multiplayer.preparing-server')"
      :mask-closable="false"
      :closable="false"
    >
      <div v-if="installing" class="install-body">
        <div class="install-spin"><IconServer /></div>
        <p class="install-name">{{ installing.name }}</p>
        <p class="install-phase">{{ installing.phase }}</p>
        <div v-if="installHasBytes" class="install-progress">
          <n-progress
            type="line"
            :percentage="installPct"
            :height="8"
            :border-radius="4"
            :show-indicator="false"
            color="#96b5e1"
          />
          <span class="install-bytes">
            {{ fmtBytes(installing.done) }} / {{ fmtBytes(installing.total) }} · {{ installPct }}%
          </span>
        </div>
      </div>
    </app-sheet>
  </div>
</template>

<style scoped>
.mp-view {
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.mode-tabs {
  display: inline-flex;
  gap: 4px;
  padding: 5px;
  align-self: flex-start;
}
.mode-tabs button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: transparent;
  color: var(--text-2);
  padding: 8px 18px;
  border-radius: 9px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
}
.mode-tabs button.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: none;
  border-radius: 10px;
  padding: 9px 18px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.15s;
}
.btn.primary {
  background: linear-gradient(135deg, var(--accent), var(--accent-deep));
  color: #1a1208;
}
.btn.primary:hover:not(:disabled) {
  filter: brightness(1.08);
}
.btn.ghost {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-1);
  border: 1px solid var(--border);
}
.btn.ghost:hover {
  background: rgba(255, 255, 255, 0.1);
}
.btn.danger {
  background: rgba(229, 83, 75, 0.16);
  color: #f0907f;
  border: 1px solid rgba(229, 83, 75, 0.4);
}
.btn.danger:hover {
  background: rgba(229, 83, 75, 0.26);
}
.btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(300px, 100%), 1fr));
  gap: 16px;
}
.server-card {
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  cursor: pointer;
}
.card-head {
  display: flex;
  align-items: center;
  gap: 10px;
}
.core-badge {
  font-size: 11px;
  font-weight: 700;
  padding: 3px 9px;
  border-radius: 999px;
  border: 1px solid;
  background: rgba(255, 255, 255, 0.04);
  letter-spacing: 0.02em;
  flex-shrink: 0;
}
.card-name {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card-meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border);
}
.meta-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  font-size: 12px;
}
.meta-row span {
  color: var(--text-3);
}
.meta-row b {
  color: var(--text-1);
  font-weight: 600;
}
.card-motd {
  font-size: 12px;
  color: var(--text-2);
  font-style: italic;
  padding: 0 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 2px;
}
.status {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-3);
  padding: 3px 8px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.05);
}
.status.on {
  color: #57c98a;
  background: rgba(87, 201, 138, 0.14);
}
.ops {
  display: flex;
  align-items: center;
  gap: 6px;
}
.op {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-2);
  border-radius: 8px;
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.13s;
}
.op:hover {
  background: rgba(255, 255, 255, 0.09);
  color: var(--text-1);
}
.op.start {
  color: var(--accent);
  border-color: var(--accent-35);
}
.op.start:hover {
  background: var(--accent-soft);
}
.op.stop {
  color: #e0a85a;
  border-color: rgba(224, 168, 90, 0.4);
}
.op.stop:hover {
  background: rgba(224, 168, 90, 0.12);
}
.op.danger:hover {
  color: #e5534b;
  border-color: rgba(229, 83, 75, 0.5);
}
.empty {
  padding: 56px 30px;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  color: var(--text-3);
}
.empty-icon {
  font-size: 36px;
  opacity: 0.6;
}
.empty p {
  margin: 0;
  font-size: 14px;
}
.empty .sub {
  font-size: 12px;
  color: var(--text-3);
}
.rooms-empty {
  margin-top: 40px;
}
.dialog-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.field label {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}
.core-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 8px;
}
.core-btn {
  display: flex;
  flex-direction: column;
  gap: 2px;
  align-items: flex-start;
  padding: 9px 12px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-2);
  cursor: pointer;
  font-family: inherit;
  text-align: left;
  transition: all 0.13s;
}
.core-btn:hover {
  background: rgba(255, 255, 255, 0.08);
}
.core-btn.active {
  background: var(--accent-soft);
  border-color: var(--accent-45);
  color: var(--accent);
}
.core-name {
  font-size: 13px;
  font-weight: 700;
}
.core-desc {
  font-size: 11px;
  color: var(--text-3);
}
.core-btn.active .core-desc {
  color: var(--accent-60);
}
.ver-cats {
  display: flex;
  gap: 4px;
}
.ver-cats button {
  border: none;
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-2);
  padding: 7px 16px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
}
.ver-cats button.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.ver-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 240px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 4px;
}
.ver-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-2);
  padding: 7px 10px;
  border-radius: 8px;
  cursor: pointer;
  font-family: inherit;
  text-align: left;
}
.ver-item:hover {
  background: rgba(255, 255, 255, 0.05);
}
.ver-item.active {
  border-color: var(--accent-05);
  background: var(--accent-soft);
  color: var(--accent);
}
.ver-id {
  font-size: 12px;
  font-weight: 600;
}
.ver-type {
  font-size: 10px;
  color: var(--text-3);
}
.ver-empty {
  text-align: center;
  color: var(--text-3);
  font-size: 13px;
  padding: 16px 0;
}
.dialog-foot {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
.confirm-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.confirm-text {
  margin: 0;
  font-size: 14px;
  color: var(--text-2);
  line-height: 1.6;
}
.confirm-text b {
  color: var(--text-1);
}
.install-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 18px 0 8px;
}
.install-spin {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 24px;
  animation: spin 1.1s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
.install-name {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-1);
}
.install-phase {
  margin: 0;
  font-size: 13px;
  color: var(--text-3);
}
.install-progress {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  margin-top: 2px;
}
.install-bytes {
  font-size: 12px;
  color: var(--text-3);
  font-variant-numeric: tabular-nums;
}

/* ---- 陶瓦联机 ---- */
.tc-download {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.tc-hero {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 20px 24px;
}
.tc-hero-icon {
  width: 52px;
  height: 52px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 14px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 26px;
  overflow: hidden;
}
.tc-icon-img img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  border-radius: 10px;
}
.tc-hero-text {
  flex: 1;
  min-width: 0;
}
.tc-hero-text h2 {
  margin: 0 0 4px;
  font-size: 17px;
}
.tc-hero-text p {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
  line-height: 1.5;
}
.tc-state {
  font-weight: 600;
  color: var(--accent) !important;
}
.tc-hero-ops {
  display: flex;
  gap: 8px;
}
.tc-dl-card {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px;
}
.tc-dl-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 700;
  color: var(--text-1);
}
.tc-dl-desc {
  margin: 0;
  font-size: 13px;
  color: var(--text-2);
  line-height: 1.6;
}
.tc-dl-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
.tc-dl-progress {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.tc-dl-bar {
  height: 12px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.07);
  border: 1px solid var(--border);
  overflow: hidden;
}
.tc-dl-fill {
  height: 100%;
  border-radius: 6px;
  background: linear-gradient(90deg, var(--accent), var(--accent-2, var(--accent)));
  transition: width 0.2s ease;
}
.tc-dl-text {
  font-size: 12px;
  color: var(--text-2);
}
.tc-hint {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
}
.tc-hint code {
  padding: 2px 6px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid var(--border);
  font-size: 11px;
}
.tc-license {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  position: fixed;
  left: 50%;
  transform: translateX(-50%);
  bottom: 12px;
  z-index: 40;
  padding: 5px 14px;
  border-radius: 999px;
  background: var(--panel);
  border: 1px solid var(--border);
  backdrop-filter: blur(var(--glass-blur, 8px));
  font-size: 11px;
  color: var(--text-3);
  text-align: center;
  white-space: nowrap;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
}
.tc-license a {
  color: var(--text-2);
  text-decoration: none;
}
.tc-license a:hover {
  color: var(--accent);
  text-decoration: underline;
}
.tc-actions {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(260px, 100%), 1fr));
  gap: 14px;
  margin-top: 4px;
}
.tc-action {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 20px;
  text-align: left;
  color: var(--text-1);
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--panel);
  cursor: pointer;
  font-family: inherit;
  transition: all 0.15s;
}
.tc-action:hover:not(:disabled) {
  background: var(--panel-hover);
  border-color: var(--accent-45);
  transform: translateY(-1px);
}
.tc-action:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.tc-action-icon {
  font-size: 22px;
  color: var(--accent);
}
.tc-action-name {
  font-size: 15px;
  font-weight: 700;
}
.tc-action-desc {
  font-size: 12px;
  color: var(--text-3);
}
.tc-code-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 20px 24px;
  text-align: center;
}
.tc-code-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-1);
}
.tc-code-value {
  font-size: 30px;
  font-weight: 700;
  letter-spacing: 0.12em;
  color: var(--accent);
  background: var(--accent-soft);
  padding: 12px 22px;
  border-radius: 14px;
}
.tc-code-hint {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
}
.tc-code-hint .mono {
  color: var(--text-1);
}
.tc-players-card {
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.tc-players-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-1);
}
.tc-players-count {
  min-width: 20px;
  height: 20px;
  padding: 0 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 10px;
  font-size: 12px;
  font-weight: 600;
  color: var(--accent);
  background: var(--accent-soft);
}
.tc-players-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.tc-player-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--panel);
}
.tc-player-avatar {
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  font-weight: 600;
  color: var(--accent);
  background: var(--accent-soft);
}
.tc-player-info {
  flex: 1;
  min-width: 0;
}
.tc-player-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tc-player-uuid {
  font-size: 11px;
  color: var(--text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tc-player-desc {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-2);
}
.tc-player-ping {
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 500;
  color: var(--accent);
  background: var(--accent-soft);
  padding: 2px 8px;
  border-radius: 10px;
}
.tc-scan-card {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 24px;
}
.tc-scan-head {
  display: flex;
  align-items: center;
  gap: 14px;
}
.tc-scan-spinner {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
  border: 3px solid var(--accent-soft);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: tc-spin 0.8s linear infinite;
}
@keyframes tc-spin {
  to {
    transform: rotate(360deg);
  }
}
.tc-scan-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-1);
}
.tc-scan-sub {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--text-3);
}
.tc-scan-steps {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--panel);
}
.tc-scan-step {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  color: var(--text-2);
}
.tc-step-num {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-size: 12px;
  font-weight: 600;
  color: var(--accent);
  background: var(--accent-soft);
}
.tc-scan-step b {
  color: var(--text-1);
}
.tc-scan-tip {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
}
.tc-code-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  justify-content: center;
}
.tc-room-controls {
  padding: 20px 24px;
}
.tc-room-controls > .tc-name-input {
  margin-bottom: 18px;
  max-width: 320px;
}
.tc-control-grid {
  display: grid;
  grid-template-columns: 1fr 1px 1fr;
  gap: 20px;
  align-items: stretch;
}
.tc-col {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
}
.tc-sub-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 700;
  color: var(--text-1);
}
.tc-sub-desc {
  margin: 0 0 4px;
  font-size: 12px;
  color: var(--text-3);
}
.tc-join-input {
  display: flex;
  gap: 8px;
  width: 100%;
}
.tc-join-input :deep(.n-input) {
  flex: 1;
}
.tc-name-input {
  width: 100%;
}
.tc-field-label {
  display: block;
  margin-bottom: 4px;
  font-size: 12px;
  color: var(--text-3);
}
.tc-divider {
  width: 1px;
  background: var(--border);
}

/* 顶部模式标签固定，下面的内容区自己滚 */
.mp-view {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.mp-view > :last-child {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
</style>
