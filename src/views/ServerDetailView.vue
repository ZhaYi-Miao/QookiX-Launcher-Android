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
import AppSelect from "../ui/AppSelect.vue";
import AppSlider from "../ui/AppSlider.vue";
import AppSwitch from "../ui/AppSwitch.vue";
import type { ServerProperties } from "../types";

const route = useRoute();
const message = useMessage();
const servers = useServersStore();
const serverId = String(route.params.id);
/** 当前页签。支持 `?tab=files` 深链 —— 多人页的服务器卡片上有「文件 / 设置」图标按钮，
 *  直接落到对应页签（否则那两个图标只是装饰，点了还是进默认页）。 */
const KNOWN_TABS = ["config", "files", "logs", "console"];
const tab = ref(
  KNOWN_TABS.includes(String(route.query.tab)) ? String(route.query.tab) : "config"
);
const busy = ref("");
const logs = ref<string[]>([]);
const form = ref({
  maxMem: 1024,
  minMem: 512,
  motd: "",
  jvmArgs: "",
  stopCommand: "stop",
  // 空闲休眠：0 = 关。默认值只是初始化，实际以 server.json 为准（onMounted 里回填）
  sleepTimeout: 30,
  sleepHint: "",
  wakeHint: "",
});

/**
 * 空闲休眠状态（轮询得到）。
 *
 * 与 store 里的 `sleepingIds` 分开存：store 那份是给列表页用的（只关心在不在休眠），
 * 这份还要带上「唤醒失败的原因」和进入休眠的时间，详情页要显示出来。
 */
const sleepInfo = ref<{ sleeping: boolean; since: number; error: string | null }>({
  sleeping: false,
  since: 0,
  error: null,
});
const sleeping = computed(() => sleepInfo.value.sleeping);

/** 休眠 / 唤醒的提示语：用户自定义优先，留空就用界面自带文案 */
const sleepText = computed(
  () => server.value?.sleep_hint?.trim() || $t("server-detail.sleep-default")
);
const wakeText = computed(
  () => server.value?.wake_hint?.trim() || $t("server-detail.wake-default")
);

/**
 * `server.properties` 里可编辑的那几项。
 *
 * 这些项 Minecraft **只在启动时读**，改完要重启服务端才生效 —— 卡片上写了这句，
 * 否则用户改完发现没变化会以为没保存。
 */
const world = ref<ServerProperties>({
  onlineMode: true,
  whiteList: false,
  viewDistance: 10,
  simulationDistance: 10,
  maxPlayers: 20,
  difficulty: "easy",
  gameMode: "survival",
});

const sleepOptions = computed(() => [
  { label: $t("server-detail.sleep-off"), value: 0 },
  { label: $t("server-detail.sleep-15"), value: 15 },
  { label: $t("server-detail.sleep-30"), value: 30 },
  { label: $t("server-detail.sleep-60"), value: 60 },
]);
const difficultyOptions = computed(() => [
  { label: $t("server-detail.difficulty-peaceful"), value: "peaceful" },
  { label: $t("server-detail.difficulty-easy"), value: "easy" },
  { label: $t("server-detail.difficulty-normal"), value: "normal" },
  { label: $t("server-detail.difficulty-hard"), value: "hard" },
]);
const gameModeOptions = computed(() => [
  { label: $t("server-detail.gamemode-survival"), value: "survival" },
  { label: $t("server-detail.gamemode-creative"), value: "creative" },
  { label: $t("server-detail.gamemode-adventure"), value: "adventure" },
  { label: $t("server-detail.gamemode-spectator"), value: "spectator" },
]);

/** 核心是否已下载（决定按钮是「下载核心」还是「启动」） */
const coreReady = ref(false);
/** 联机地址（WiFi IP:端口），null = 未连 WiFi */
const address = ref<string | null>(null);
/**
 * 设备当前可用内存（MB），null = 读不到。
 *
 * 服务端启动时堆上限会**按设备可用内存钳制**（后端 `clamp_server_heap_mb`），
 * 所以必须把「会被限制到多少」告诉用户 —— 否则他填 4G、实际只跑 1.5G，
 * 出了「服务器自己没了 / 应用被杀」的问题都不知道该调哪个旋钮。
 */
const deviceMemMb = ref<number | null>(null);

/** 只有填的内存超过设备能给的额度时才提示，平时不打扰 */
const memClampHint = computed(() => {
  const avail = deviceMemMb.value;
  if (!avail) return "";
  // 与后端 clamp_server_heap_mb 的 60% / 512MB 下限保持一致
  const cap = Math.max(Math.floor(avail * 0.6), 512);
  const want = Number(form.value.maxMem) || 0;
  if (want <= cap) return "";
  const fmt = (mb: number) => (mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`);
  return $t("server-detail.mem-clamped-hint", { avail: fmt(avail), cap: fmt(cap) });
});

/**
 * 电池优化是否已放行。默认 `true`（读不到就不打扰用户）。
 *
 * false 时才给入口：ROM 的省电策略会在息屏一段时间后掐掉后台的前台服务，
 * 服务器「玩一会儿自己没了」多半就是它 —— `foregroundServiceType=specialUse`
 * 只挡得住 Android 15 的 6h/24h 硬上限，挡不住 ROM 自己的省电策略。
 */
const batteryOk = ref(true);

function refreshBattery() {
  api
    .batteryUnrestricted()
    .then((v) => {
      batteryOk.value = v;
    })
    .catch(() => {
      /* 读不到就当已放行 */
    });
}

/** 弹系统「忽略电池优化」页；回来后靠 window focus 事件刷新状态 */
function askBattery() {
  void api.requestBatteryUnrestricted().catch(() => {});
}

/**
 * 设备热状态（0 正常 … 2 中等 … 4 危急 … 6 关机）。
 *
 * 手机开服跑久了会被系统降频，服务器跟着变卡。说出来，用户才知道
 * 「不是服务器坏了，是手机热了」。只提示、不拦：热节流是手机的自我保护，
 * 凉下来会自己恢复，各家 ROM 的阈值也不一样 —— 按系统给的数说话最可靠。
 * 阈值取 `PowerManager.THERMAL_STATUS_MODERATE` / `_CRITICAL`（跟 Anvil 同一套）。
 */
const THERMAL_WARN = 2;
const THERMAL_CRITICAL = 4;
const thermal = ref(0);

const thermalHint = computed(() => {
  if (thermal.value >= THERMAL_CRITICAL) return $t("server-detail.thermal-critical");
  if (thermal.value >= THERMAL_WARN) return $t("server-detail.thermal-warn");
  return "";
});

function refreshThermal() {
  api
    .deviceThermalStatus()
    .then((v) => {
      thermal.value = v ?? 0;
    })
    .catch(() => {
      /* 读不到就当正常，不打扰用户 */
    });
}

/** 从系统设置返回 / 回到前台时，把设备状态（电池放行 + 发热）一起刷一遍 */
function refreshDeviceState() {
  refreshBattery();
  refreshThermal();
}

/**
 * 打开服务器文件目录。
 *
 * 服务器文件在应用私有区（`/data/data/<包名>/files/servers/<id>`），系统文件管理器默认
 * 进不去 —— 所以后端走 FileProvider 把它包成 `content://` 目录 URI 交给系统。**能不能被
 * 打开取决于本机装了哪个管理器**（返回 0 就是没有能认目录的应用），这时直接切到内置的
 * 「服务器文件」页：那个页面本来就是同一个目录，用户不会「点了没反应」。
 */
const openingDir = ref(false);
async function openDir() {
  openingDir.value = true;
  try {
    const r = await api.openHostedServerDirectory(serverId);
    if (r === 1) return;
    message.info($t("server-detail.open-dir-fallback"));
    tab.value = "files";
  } catch (e) {
    message.error(String(e));
  } finally {
    openingDir.value = false;
  }
}

/* ── 内存滑块 ────────────────────────────────────────────────────────
 * 手机上「往文本框里填一个数字」不是好的内存输入方式：单位、步长、上下限全都看不见，
 * 用户填 4096 也不知道是 4G 还是 4096M。换成滑块（0.5GB 步进）+ 实时 GB 值，
 * 钳制提示挂在下面 —— 和竞品 Anvil 的「性能」页同一个思路。
 */
const MEM_MIN = 512;
const MEM_MAX = 6144;
const MEM_STEP = 512;

const memSlider = computed(() =>
  Math.min(MEM_MAX, Math.max(MEM_MIN, Number(form.value.maxMem) || MEM_MIN))
);
function onMemSlider(v: number) {
  form.value.maxMem = v;
}
const memGb = computed(() => `${(memSlider.value / 1024).toFixed(1)} GB`);
const memMinGb = computed(() => `${(MEM_MIN / 1024).toFixed(1)} GB`);
const memMaxGb = computed(() => `${(MEM_MAX / 1024).toFixed(1)} GB`);

/**
 * JVM 参数留空时**预填**一组针对手机的 G1 调优参数（竞品 Anvil 也预填）。
 *
 * 为什么不是完整的 Aikar flags：完整版里有 `-XX:+AlwaysPreTouch` 和
 * `G1NewSizePercent/MaxNewSizePercent` 的大值组合，那是给 12GB+ 的服务器准备的 ——
 * `AlwaysPreTouch` 会把整个堆**预先摸一遍**（等于启动即提交全部内存），在内存紧张、
 * 还要给启动器/隧道留余量的手机上正好是反效果。
 */
const AIKAR_PHONE_FLAGS =
  "-XX:+DisableExplicitGC -XX:+ParallelRefProcEnabled -XX:+PerfDisableSharedMem " +
  "-XX:+UnlockExperimentalVMOptions -XX:MaxGCPauseMillis=200 -XX:G1NewSizePercent=30 " +
  "-XX:G1MaxNewSizePercent=40 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 " +
  "-XX:InitiatingHeapOccupancyPercent=15 -XX:G1ReservePercent=20";

/* ── 控制台快捷命令 ──────────────────────────────────────────────────
 * 高频命令打成 chips，免得每次都在手机键盘上敲 "/kick "。带参数的（/op、/kick）
 * 只**填进输入框**并带一个尾随空格，让用户接着补参数；无参的直接可发。
 */
const QUICK_CMDS: { cmd: string; danger?: boolean }[] = [
  { cmd: "/list" },
  { cmd: "/help" },
  { cmd: "/op " },
  { cmd: "/kick " },
  { cmd: "/stop", danger: true },
];

function useQuickCmd(cmd: string) {
  consoleInput.value = cmd;
}
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
      // 一次拿全：在不在跑、是不是休眠、唤醒失败没有。
      // 原来只问 `isHostedServerRunning` —— 休眠中的服务器 JVM 确实停了，
      // 于是它在界面上与「用户自己停掉的」长得一模一样，用户不知道该点「启动」还是「唤醒」。
      const rt = await api.hostedServerRuntime(s.id);
      const now = !!rt?.running;
      if (now !== servers.isRunning(s.id)) servers.setRunning(s.id, now);
      const sl = !!rt?.sleeping;
      if (sl !== servers.isSleeping(s.id)) servers.setSleeping(s.id, sl);
      const err = rt?.wakeError ?? null;
      if (sl !== sleepInfo.value.sleeping || err !== sleepInfo.value.error) {
        sleepInfo.value = { sleeping: sl, since: rt?.sleepSince ?? 0, error: err };
      }
      if (now) {
        const addr = await api.hostedServerAddress(s.id);
        if (addr !== address.value) address.value = addr;
      }
      // 日志页签打开且正在运行 → 自动追尾刷新。
      // 服务端在另一个进程里跑，不轮询的话日志就停在打开那一刻，
      // 用户看到的是「死」的日志，还以为服务器挂了。
      if (tab.value === "logs" && now) await loadLogs();
      // 发热也要跟着更新：不刷新的话「手机热到降频」这条提示要等用户切出去再回来才出现，
      // 而那正是最需要它的时候。取一次热状态只是个 JNI getter，比上面那次 IPC 便宜得多。
      refreshThermal();
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
    // **装完以后端的实查为准，不能直接置 true**：按钮一切到「启动」，点下去后端
    // core_installed() 不认就会报「还没下载服务端核心」（Fabric 只留下安装器的
    // 旧装法就出过这种「下载完成却启动不了」的反馈）。
    coreReady.value = await api.hostedServerCoreInstalled(s.id).catch(() => false);
    if (coreReady.value) {
      message.success($t("server-detail.core-ready"));
    } else {
      message.error($t("server-detail.core-verify-failed"));
    }
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
  // 先记下来：启动成功后状态会被清掉，之后就没法判断这次是「启动」还是「唤醒」了
  const wasSleeping = sleeping.value;
  try {
    if (running.value) {
      const note = await api.stopHostedServer(s.id);
      servers.setRunning(s.id, false);
      sleepInfo.value = { sleeping: false, since: 0, error: null };
      message.success(note || $t("multiplayer.server-stopped"));
      await loadLogs();
    } else {
      // 休眠中手动唤醒：把按钮文案与阶段提示都换成用户自己配的那句，
      // 别让「点了唤醒」看起来和「点了启动」一模一样。
      startStage.value = wasSleeping ? wakeText.value : $t("server-detail.starting");
      await watchStage();
      await servers.start(s.id);
      sleepInfo.value = { sleeping: false, since: 0, error: null };
      message.success(
        wasSleeping ? $t("server-detail.woken") : $t("multiplayer.server-started")
      );
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
      motd: form.value.motd.trim(),
      jvm_args: form.value.jvmArgs,
      stop_command: form.value.stopCommand,
      sleep_timeout_min: Number(form.value.sleepTimeout) || 0,
      // 空串表示「用界面自带文案」，后端会收成 null
      sleep_hint: form.value.sleepHint.trim(),
      wake_hint: form.value.wakeHint.trim(),
    });
    // `server.properties` 那几项单独写：它们是服务端自己读的文件，不是 server.json 的字段。
    // 以返回值为准刷新界面 —— 后端会做范围校验（距离 3–32、人数 1–100），
    // 用户填了超范围的值得让他看见真正落盘的是什么。
    world.value = await api.setHostedServerProperties(s.id, { ...world.value });
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
    form.value.motd = s.motd ?? "";
    form.value.jvmArgs = s.jvm_args ?? "";
    // 从没配过 JVM 参数（后端是 null）才预填；用户存过空串就尊重他的选择，别反复填回来
    if (s.jvm_args == null) form.value.jvmArgs = AIKAR_PHONE_FLAGS;
    form.value.stopCommand = s.stop_command ?? "stop";
    form.value.sleepTimeout = s.sleep_timeout_min ?? 30;
    form.value.sleepHint = s.sleep_hint ?? "";
    form.value.wakeHint = s.wake_hint ?? "";
    // 读设备可用内存（只用于提示，失败无所谓）
    deviceMemMb.value = await api.deviceAvailableMemory().catch(() => null);
    try {
      coreReady.value = await api.hostedServerCoreInstalled(s.id);
      address.value = await api.hostedServerAddress(s.id);
      // server.properties 那几项不在 server.json 里，单独读一次
      world.value = await api.getHostedServerProperties(s.id);
    } catch {
      /* 忽略 */
    }
  }
  // 电池优化状态：用户去系统设置点完「允许」再返回时，WebView 会重新拿到焦点，
  // 靠 focus 事件刷新按钮（不会去设置页就不会触发，无害）。
  refreshBattery();
  refreshThermal();
  window.addEventListener("focus", refreshDeviceState);
  startPolling();
});

onUnmounted(() => {
  window.removeEventListener("focus", refreshDeviceState);
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
      <span class="dot" :class="{ on: running, sleep: sleeping && !running }"></span>
    </div>

    <!-- 休眠提示条：必须说清「现在没在跑，但有人连就会自动起来」。
         少了这段，用户看到一个灰点只会以为服务器关了、地址失效了。 -->
    <div v-if="sleeping && !running" class="sleep-note glass">
      <div class="sleep-t">{{ $t("server-detail.sleeping") }}</div>
      <p class="sleep-h">{{ sleepText }}</p>
      <p v-if="sleepInfo.error" class="sleep-err">{{ sleepInfo.error }}</p>
    </div>

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
            : sleeping
              ? $t("server-detail.wake")
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
      <!-- ── 基本 ──────────────────────────────────────────────────────
           参照竞品 Anvil 的设置逻辑：设置按「主题」分组卡片，每项配一句玩家能看懂的解释，
           底部统一一个保存按钮（而不是改一项存一项）。 -->
      <section class="set-card">
        <h3 class="set-card-title">{{ $t("server-detail.sec-basic") }}</h3>
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
        <div class="field">
          <label>{{ $t("server-detail.motd") }}</label>
          <app-input v-model:value="form.motd" type="textarea" :rows="2" :placeholder="$t('server-detail.motd-hint')" />
          <p class="field-hint">{{ $t("server-detail.motd-hint2") }}</p>
        </div>
      </section>

      <!-- ── 内存与性能 ── -->
      <section class="set-card">
        <h3 class="set-card-title">{{ $t("server-detail.sec-memory") }}</h3>
        <!-- 先把后果说清楚，再给控件（竞品的做法：每个高级设置配一句「为什么/后果」） -->
        <p class="set-warn">{{ $t("server-detail.ram-warn") }}</p>
        <div class="field">
          <label>
            {{ $t("server-detail.max-memory") }}
            <b class="mem-live">{{ memGb }}</b>
          </label>
          <app-slider :value="memSlider" :min="MEM_MIN" :max="MEM_MAX" :step="MEM_STEP" @update:value="onMemSlider" />
          <div class="mem-range">
            <span>{{ memMinGb }}</span>
            <span>{{ memMaxGb }}</span>
          </div>
        </div>
        <!-- 内存钳制提示：只在「填的值超过设备能给」时出现 -->
        <p v-if="memClampHint" class="mem-hint">{{ memClampHint }}</p>
        <!-- 电池优化：没放行时给入口，否则开服容易被 ROM 省电策略悄悄掐掉 -->
        <div v-if="!batteryOk" class="choice">
          <div class="choice-info">
            <span class="choice-label">{{ $t("server-detail.battery-title") }}</span>
            <p class="choice-hint">{{ $t("server-detail.battery-hint") }}</p>
          </div>
          <van-button size="small" @click="askBattery">{{ $t("server-detail.battery-open") }}</van-button>
        </div>
        <!-- 设备发热：手机开服跑久了必被降频，服务器跟着变卡。
             说出来，用户才知道这不是服务器坏了（详见表盘脚本里的 refreshThermal）。 -->
        <p v-if="thermalHint" class="set-warn">{{ thermalHint }}</p>
      </section>

      <!-- ── 联机 ── -->
      <section class="set-card">
        <h3 class="set-card-title">{{ $t("server-detail.sec-online") }}</h3>
        <!-- 联机地址：朋友在电脑上填这个就能连进来。
             它属于「联机」，放进设置里 —— 主页面只留标题和启动按钮，上面那块空间别摆卡片。 -->
        <div class="addr">
          <div class="addr-l">
            <div class="addr-t">{{ $t("server-detail.address") }}</div>
            <div class="addr-v">{{ address ?? $t("server-detail.no-wifi") }}</div>
          </div>
          <van-button v-if="address" size="small" class="addr-b" @click="copyAddress">
            {{ $t("server-detail.copy-address") }}
          </van-button>
        </div>
        <p class="addr-hint">{{ $t("server-detail.address-hint") }}</p>
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

        <!-- 空闲休眠：手机开服「要常在线」与「要省电」的答案 ——
             无人在线时自动停服省电，游戏端口继续听着，有人连就自动起来。
             所以它放在「联机」卡里：改的是「别人能不能连进来」这件事的行为。 -->
        <div class="field">
          <label>{{ $t("server-detail.sleep-timeout") }}</label>
          <app-select v-model:value="form.sleepTimeout" :options="sleepOptions" />
          <p class="field-hint">{{ $t("server-detail.sleep-timeout-hint") }}</p>
        </div>
        <div class="field">
          <label>{{ $t("server-detail.sleep-hint") }}</label>
          <app-input v-model:value="form.sleepHint" :placeholder="$t('server-detail.sleep-default')" />
        </div>
        <div class="field">
          <label>{{ $t("server-detail.wake-hint") }}</label>
          <app-input v-model:value="form.wakeHint" :placeholder="$t('server-detail.wake-default')" />
        </div>
      </section>

      <!-- ── 世界与规则 ──
           这几项写在服务端自己的 `server.properties` 里，与后端白名单一一对应
           （见 servers.rs 的 editable_props）：只放玩家真会调的，其它键写错会让服务端起不来。 -->
      <section class="set-card">
        <h3 class="set-card-title">{{ $t("server-detail.sec-world") }}</h3>
        <p class="set-warn">{{ $t("server-detail.props-need-restart") }}</p>
        <div class="choice">
          <div class="choice-info">
            <span class="choice-label">{{ $t("server-detail.prop-online-mode") }}</span>
            <p class="choice-hint">{{ $t("server-detail.prop-online-mode-hint") }}</p>
          </div>
          <app-switch :value="world.onlineMode" @update:value="(v: boolean) => (world.onlineMode = v)" />
        </div>
        <div class="choice">
          <div class="choice-info">
            <span class="choice-label">{{ $t("server-detail.prop-white-list") }}</span>
            <p class="choice-hint">{{ $t("server-detail.prop-white-list-hint") }}</p>
          </div>
          <app-switch :value="world.whiteList" @update:value="(v: boolean) => (world.whiteList = v)" />
        </div>
        <!-- 视野/模拟距离用滑块：手机上填数字既看不见范围也看不见当前值。
             两项都会明显影响手机上的性能与内存，所以都在同一张卡里、配上范围端点。 -->
        <div class="field">
          <label>{{ $t("server-detail.prop-view-distance") }} <b class="mem-live">{{ world.viewDistance }}</b></label>
          <app-slider
            :value="world.viewDistance"
            :min="3"
            :max="32"
            :step="1"
            @update:value="(v: number) => (world.viewDistance = v)" />
          <p class="field-hint">{{ $t("server-detail.prop-view-distance-hint") }}</p>
        </div>
        <div class="field">
          <label>
            {{ $t("server-detail.prop-simulation-distance") }}
            <b class="mem-live">{{ world.simulationDistance }}</b>
          </label>
          <app-slider
            :value="world.simulationDistance"
            :min="3"
            :max="32"
            :step="1"
            @update:value="(v: number) => (world.simulationDistance = v)" />
        </div>
        <div class="field">
          <label>{{ $t("server-detail.prop-max-players") }}</label>
          <app-input v-model:value="world.maxPlayers" type="text" />
        </div>
        <div class="field">
          <label>{{ $t("server-detail.prop-difficulty") }}</label>
          <app-select v-model:value="world.difficulty" :options="difficultyOptions" />
        </div>
        <div class="field">
          <label>{{ $t("server-detail.prop-game-mode") }}</label>
          <app-select v-model:value="world.gameMode" :options="gameModeOptions" />
        </div>
      </section>

      <!-- ── 高级 ──
           低频/危险/需要背景知识的设置全收进这里，并打上「高级」徽标 ——
           新手不会误碰，老手也不用翻。 -->
      <section class="set-card">
        <h3 class="set-card-title">
          {{ $t("server-detail.sec-advanced") }}
          <span class="adv-badge">{{ $t("server-detail.advanced") }}</span>
        </h3>
        <div class="field">
          <label>{{ $t("server-detail.jvm-args") }}</label>
          <app-input v-model:value="form.jvmArgs" type="textarea" :rows="3" :placeholder="$t('server-detail.jvm-args-hint')" />
          <p class="field-hint">{{ $t("server-detail.jvm-hint") }}</p>
        </div>
        <div class="field">
          <label>{{ $t("server-detail.min-memory") }}</label>
          <app-input v-model:value="form.minMem" type="text" />
          <p class="field-hint">{{ $t("server-detail.min-memory-hint") }}</p>
        </div>
        <div class="field">
          <label>{{ $t("server-detail.stop-cmd") }}</label>
          <app-input v-model:value="form.stopCommand" placeholder="stop" />
          <p class="field-hint">{{ $t("server-detail.stop-hint") }}</p>
        </div>
      </section>

      <van-button class="save" block type="primary" :loading="busy === 'save'" @click="save">{{ $t("common.save") }}</van-button>
    </div>
    <div v-else-if="tab === 'files'" class="pane files">
      <!-- 「打开目录」放这里：它本来就是文件相关的操作，摆在主页面只是多占一张卡片。
           打不开（本机没有能认文件夹的应用）时脚本会提示一句，用户继续用下面的内置管理器。 -->
      <div class="dirbar">
        <span class="dirbar-h">{{ $t("server-detail.open-dir-hint") }}</span>
        <van-button size="small" :loading="openingDir" @click="openDir">
          {{ $t("server-detail.open-dir") }}
        </van-button>
      </div>
      <ServerFileManager :server-id="serverId" />
    </div>
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
      <!-- 快捷命令：高频命令打成 chips，免得每次都在手机键盘上敲 "/kick "。
           带参数的（/op、/kick）只**填进输入框**并带尾随空格，让用户接着补参数；
           无参的（/list、/help、/stop）点一下输入框就有了，再按发送即可。 -->
      <div class="qcmds">
        <button
          v-for="q in QUICK_CMDS"
          :key="q.cmd"
          class="qcmd"
          :class="{ danger: q.danger }"
          @click="useQuickCmd(q.cmd)"
        >{{ q.cmd }}</button>
      </div>
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
/* 休眠：绿色看着像「正在跑」、灰色看着像「已停止」，都不对。
   用琥珀色 —— 它既没在跑也不是被关掉，是「睡着等被叫醒」。 */
.dot.sleep {
  background: #e0a33e;
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
/* 文件页顶部那行「打开目录」：固定不动，说明文字弱一档 */
.dirbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}
.dirbar-h {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  line-height: 1.4;
  color: var(--text-3);
}
/* ── 休眠提示条 ──
   与联机地址卡同宽同风格；标题用琥珀色（与状态点一致），正文不用警告色 ——
   休眠是正常工作状态，不是错误。 */
.sleep-note {
  padding: 10px 14px;
  flex-shrink: 0;
}
.sleep-t {
  font-size: 13px;
  font-weight: 600;
  color: #e0a33e;
}
.sleep-h {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-2);
  word-break: break-all;
}
/* 唤醒失败的原因才需要显眼：用户点过「唤醒」或有人连过，服务器却没起来 */
.sleep-err {
  margin: 6px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: #e5534b;
  word-break: break-all;
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
/* 文件页：同日志页的道理——外层不再滚，顶上那行「打开目录」固定不动，
   滚动交给文件管理器内部。子组件根元素带着本组件的 scope id，所以能直接选到 `.fm`；
   它自带的 `height: 100%` 在这里要换成 flex，否则会跟上面那行一起把面板撑出滚动条。 */
.pane.files {
  overflow: hidden;
  gap: 8px;
}
.pane.files > .fm {
  flex: 1;
  min-height: 0;
  height: auto;
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
/* 「内存会被自动限制」的提示：紧跟在内存输入框下面。
   用弱一档的颜色 —— 它是常态信息，不是报错，别做成警告那样扎眼。 */
.mem-hint {
  margin: -6px 0 12px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-3);
}
/* ── 设置分组卡片（参照竞品 Anvil 的「图标 + 标题 / 控件 / 一句解释」模板）──
   原来 8 个字段平铺一列，没有主次：EULA、对外开房、内存、JVM 全长一个样。
   分成 4 张卡后，每张卡一个主题，低频/危险的收进「高级」。 */
.set-card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.set-card-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
  display: flex;
  align-items: center;
  gap: 6px;
}
/* 「高级」徽标：把低频/危险的设置标出来 —— 新手不会误碰，老手也不用翻 */
.adv-badge {
  padding: 1px 6px;
  border-radius: 5px;
  font-size: 10px;
  font-weight: 500;
  color: var(--text-3);
  background: var(--panel-hover);
  border: 1px solid var(--border);
}
/* 放在控件上方的「后果说明」：这是常态信息（不是报错），弱一档颜色、居中 */
.set-warn {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-3);
  text-align: center;
}
/* 控件下面的一句解释 */
.field-hint {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-3);
}
/* 滑杆当前值（跟在 label 后面，用主题色强调） */
.mem-live {
  color: var(--accent);
  font-size: 13px;
}
/* 滑杆两端的最小/最大值 */
.mem-range {
  display: flex;
  justify-content: space-between;
  margin-top: -2px;
  font-size: 11px;
  color: var(--text-3);
}
/* ── 控制台快捷命令 ── */
.qcmds {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  flex: 0 0 auto;
}
.qcmd {
  padding: 5px 10px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-2);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  cursor: pointer;
}
.qcmd.danger {
  color: #e5534b;
  border-color: #e5534b;
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

/* ── 手机横屏（矮窗口）：设置分组卡片排两列 ───────────────────────────────
 * 横屏只有 ~360px 高，五张设置卡一列排下去要滚很久；宽度反而空着 500px+。
 * 只对「设置」这一页生效 —— 文件 / 日志 / 控制台各有各的布局，显式退回 flex。 */
@media (orientation: landscape) and (max-height: 560px) {
  .pane {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-items: start;
    gap: 10px;
  }
  /* 保存按钮横跨两列（它是整页唯一的动作，压在最后一行的右边会很难找） */
  .pane > .save {
    grid-column: 1 / -1;
  }
  .pane.files,
  .pane.logs,
  .pane.console {
    display: flex;
  }
}
</style>
