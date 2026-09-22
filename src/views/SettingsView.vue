<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { fmtMem, fmtSize, fmtTime } from "../utils/format";
import {
  NColorPicker,
  NInput,
  NRadio,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSlider,
  NSwitch,
  NTooltip,
  useDialog,
  useMessage,
} from "naive-ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import { pickFile as open } from "../composables/filePicker";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useMemoryInfo } from "../composables/useMemoryInfo";
import { useSettingsStore } from "../stores/settings";
import { api } from "../api";

/**
 * 「并行下载」两个滑杆的开关。
 *
 * 目前后端是**串行**下载（`download::download_libraries` 逐个 `await`），
 * 单文件分片也没实现，所以滑杆拖了不会有任何效果 —— 先隐藏，
 * 假开关比缺功能更伤（用户会以为自己调过了、然后去排查为什么没用）。
 * 后端实现并发/分片后打开它即可。
 */
const DOWNLOAD_SLIDERS_ENABLED = false;

/**
 * 「手柄死区」滑杆 —— 默认隐藏。
 *
 * 实体手柄在 QookiX 里**完全不可用**：移植时有意剥掉了手柄重映射那一半
 * （见 `MinecraftGLSurface` 的类注释：原版字段 `mGamepadHandler` / `mInputManager`、
 * `createGamepad`、`onDirectGamepadEnabled`，以及两处 `Gamepad.isGamepadEvent` 分支，
 * 都依赖 `fr.spse.gamepad_remapper` 这个只在 JitPack 发布的 AAR）。
 * 剩下的三条路径也都不通：generic motion 分支要找鼠标指针否则直接 return false、
 * `EfficientAndroidLWJGLKeycode` 里只有 DPAD（全文没有任何 `KEYCODE_BUTTON_*`）。
 *
 * 这个滑杆写进 `gamepad_deadzone_scale`，读它的只有 `GamepadJoystick` 的实例方法，
 * 而**全项目没有任何地方 new 过它** —— 也就是说拖了它不会改变任何行为。
 * 假开关比缺功能更伤：用户会以为自己调过了，然后去排查「为什么手柄还是不动」。
 *
 * 真要做手柄支持：补回上面那些分支 + 一串 `KEYCODE_BUTTON_*` 映射
 * （或者退一步只映射轴），并补一个重映射设置界面；做完把这个常量打开。
 * 注意开发机没接手柄 —— 这类改动需要真手柄才能验证。
 */
const GAMEPAD_DEADZONE_ENABLED = false;
import { useSlidingIndicator } from "../composables/useSlidingIndicator";
import {
  IconDownload,
  IconExternal,
  IconFile,
  IconGithub,
  IconGlobe,
  IconHeart,
  IconImage,
  IconList,
  IconUsers,
  IconRefresh,
  IconPlay,
  IconSliders,
  IconTrash,
} from "../components/icons";
import type { MirrorPreset, StorageStats } from "../types";
import devWeimoshengUrl from "../assets/dev-weimosheng.jpg";
import devZhayiUrl from "../assets/dev-zhayi.jpg";
import logoUrl from "../assets/logo.png";
import AboutShowcase from "../components/AboutShowcase.vue";

/* ---- 版本徽章彩蛋：长按 v1.0.0 约 2.5s → 全屏像素烟花 + 制作名单 ----
 * 按住期间徽章脉冲提示「正在积蓄」，松手即取消；触发后任意点击关闭。
 * 烟花 = 预生成的彩色像素方块（CSS 动画向外炸开再淡出，循环）。 */
const VER_HOLD_MS = 2000;
const verHolding = ref(false);
const verEgg = ref(false);
let verHoldTimer = 0;
function verDown() {
  if (verEgg.value) return;
  verHolding.value = true;
  clearTimeout(verHoldTimer);
  verHoldTimer = window.setTimeout(() => {
    verHolding.value = false;
    verEgg.value = true;
  }, VER_HOLD_MS);
}
function verCancel() {
  verHolding.value = false;
  clearTimeout(verHoldTimer);
}
/** 关闭彩蛋层（点击弹层任意处）。 */
function verClose() {
  verEgg.value = false;
  verHolding.value = false;
  clearTimeout(verHoldTimer);
}
const verSparks = Array.from({ length: 56 }, (_, i) => ({
  left: Math.random() * 100,
  top: Math.random() * 100,
  delay: Math.random() * 1.4,
  dur: 0.9 + Math.random() * 1.1,
  size: 4 + Math.round(Math.random() * 7),
  color: ["#7ee787", "#ffd166", "#79c0ff", "#f778ba", "#ffffff"][i % 5],
}));

const settings = useSettingsStore();
const message = useMessage();
const dialog = useDialog();

// 主题 seg 滑动高亮
const themeSegRef = ref<HTMLElement | null>(null);
const { indicatorStyle: themeSegStyle, refresh: refreshThemeSeg } = useSlidingIndicator(
  themeSegRef,
  () => Array.from(themeSegRef.value?.querySelectorAll<HTMLElement>(".seg button") ?? []),
  () => (settings.settings?.theme === "light" ? 1 : 0),
  { axis: "horizontal" }
);
watch(() => settings.settings?.theme, () => nextTick(() => refreshThemeSeg()));

// 主题色预设
const themeColorPresets = [
  "#e89a4b",
  "#ff6b35",
  "#e5534b",
  "#ec4899",
  "#8b5cf6",
  "#5aa2f0",
  "#22d3ee",
  "#4ec9a0",
  "#f5c518",
];
function onThemeColorInput(val: string) {
  if (val) settings.patch({ theme_color: val });
}

// 下载代理 seg 滑动高亮（系统代理 / 直连 / 自定义）
const proxyModeSegRef = ref<HTMLElement | null>(null);
const proxyModes = [
  { id: "system", label: "系统代理" },
  { id: "direct", label: "直连" },
  { id: "custom", label: "自定义" },
];
const { indicatorStyle: proxyModeSegStyle, refresh: refreshProxyModeSeg } = useSlidingIndicator(
  proxyModeSegRef,
  () => Array.from(proxyModeSegRef.value?.querySelectorAll<HTMLElement>(".seg button") ?? []),
  () => Math.max(0, proxyModes.findIndex((m) => m.id === settings.settings?.proxy_mode)),
  { axis: "horizontal" }
);
watch(() => settings.settings?.proxy_mode, () => nextTick(() => refreshProxyModeSeg()));

// 系统/VPN 下发的代理地址（原生请求不会自动走它，所以要在界面上告诉用户）
const systemProxy = ref<string | null>(null);
async function refreshSystemProxy() {
  try {
    systemProxy.value = await api.detectSystemProxy();
  } catch {
    systemProxy.value = null;
  }
}

// 屏幕方向 seg 滑动高亮（跟随系统 / 竖屏 / 横屏）
const orientationSegRef = ref<HTMLElement | null>(null);
const orientationModes = [
  { id: "landscape", label: "横屏" },
  { id: "portrait", label: "竖屏" },
  { id: "system", label: "跟随系统" },
];
const { indicatorStyle: orientationSegStyle, refresh: refreshOrientationSeg } = useSlidingIndicator(
  orientationSegRef,
  () => Array.from(orientationSegRef.value?.querySelectorAll<HTMLElement>(".seg button") ?? []),
  () =>
    Math.max(
      0,
      orientationModes.findIndex((m) => m.id === (settings.settings?.orientation ?? "system"))
    ),
  { axis: "horizontal" }
);
watch(() => settings.settings?.orientation, () => nextTick(() => refreshOrientationSeg()));

async function selectOrientation(id: string) {
  if ((settings.settings?.orientation ?? "system") === id) return;
  try {
    await settings.patch({ orientation: id });
    // 立即下发到原生层，无需重启应用
    await api.setOrientation(id);
  } catch (e) {
    message.error(String(e));
  }
}

async function selectProxyMode(id: string) {
  if (settings.settings?.proxy_mode === id) return;
  try {
    await settings.patch({ proxy_mode: id });
  } catch (e) {
    message.error(String(e));
  }
}

// 导航栏位置 seg（底部 / 左侧）。只影响手机端 —— 导航栏仅在手机断点显示，
// 桌面端一直是侧边栏。纯 CSS 切换，改完立即生效、无需重启。
const navPositionSegRef = ref<HTMLElement | null>(null);
const navPositionModes = [
  { id: "bottom", label: "底部" },
  { id: "left", label: "左侧" },
];
const { indicatorStyle: navPositionSegStyle, refresh: refreshNavPositionSeg } = useSlidingIndicator(
  navPositionSegRef,
  () => Array.from(navPositionSegRef.value?.querySelectorAll<HTMLElement>(".seg button") ?? []),
  () => Math.max(0, navPositionModes.findIndex((m) => m.id === settings.settings?.nav_position)),
  { axis: "horizontal" }
);
watch(() => settings.settings?.nav_position, () => nextTick(() => refreshNavPositionSeg()));

async function selectNavPosition(id: string) {
  if (settings.settings?.nav_position === id) return;
  try {
    await settings.patch({ nav_position: id });
  } catch (e) {
    message.error(String(e));
  }
}

/** 挖孔类型预设：打孔统一让开 44px，刘海用 nav_offset 自定义宽度，「自动」跟随系统。 */
const navCutoutModes = [
  { id: "auto", label: "自动" },
  { id: "none", label: "无" },
  { id: "center", label: "中置挖孔" },
  { id: "topleft", label: "左上角" },
  { id: "topright", label: "右上角" },
  { id: "notch", label: "刘海" },
] as const;

/**
 * 触控目标大小档位。
 *
 * 项目原本的按钮是桌面尺寸（如 `.tb-action` 89×28、`.icon-btn` 30×30），手指容易点错。
 * 但紧凑观感本身是设计的一部分，所以**默认不动**，只给需要的人一个放大档位：
 * 档位写在 `<html data-touch>` 上，具体规则在 `styles.css`。
 */
const touchTargetModes = [
  { id: "compact", label: "紧凑（默认）" },
  { id: "standard", label: "标准 40px" },
  { id: "large", label: "大 48px" },
] as const;

function selectTouchTarget(id: (typeof touchTargetModes)[number]["id"]) {
  if (settings.settings) settings.settings.touch_target = id;
  void settings.patch({ touch_target: id });
}

function selectNavCutout(id: (typeof navCutoutModes)[number]["id"]) {
  if (settings.settings) settings.settings.nav_cutout = id;
  void settings.patch({ nav_cutout: id });
  // 首次切到某个手动类型时给个合理起点：打孔 44、刘海 64（都不合适可拖下面的滑杆）
  if (id !== "auto" && id !== "none") {
    const cur = settings.settings?.nav_offset ?? 0;
    if (cur < 24) {
      if (settings.settings) settings.settings.nav_offset = id === "notch" ? 64 : 44;
      void settings.patch({ nav_offset: settings.settings?.nav_offset ?? 44 });
    }
  }
}

/** 避让宽度滑块：存进 nav_offset（所有手动模式都参与避让推导）。 */
function onNavOffset(v: number) {
  if (!Number.isFinite(v)) return;
  if (settings.settings) settings.settings.nav_offset = v;
  void settings.patch({ nav_offset: v });
}

function onCustomProxyInput() {
  if (settings.settings && settings.settings.proxy_mode !== "custom") {
    settings.settings.proxy_mode = "custom";
  }
}

// 测试代理连接
const testingProxy = ref(false);
async function testProxy() {
  if (testingProxy.value || !settings.settings) return;
  testingProxy.value = true;
  try {
    const { proxy_mode, proxy } = settings.settings;
    // 自定义模式必须填地址，否则后端会退化为直连而误报成功
    if (proxy_mode === "custom" && !(proxy ?? "").trim()) {
      message.warning("请先填写代理地址");
      return;
    }
    const res = await api.testProxy(
      proxy_mode,
      proxy_mode === "custom" ? proxy : null
    );
    message.success(`连接成功 ${res.ms} ms`);
  } catch (e) {
    message.error(`连接失败: ${e}`);
  } finally {
    testingProxy.value = false;
  }
}

// 下载镜像源
const mirrors = ref<MirrorPreset[]>([]);
/** 每个镜像最近一次测速结果（毫秒）；null 表示不可用 */
const mirrorLatency = ref<Record<string, number | null>>({});
const testingMirror = ref("");

async function loadMirrors() {
  try {
    mirrors.value = await api.listMirrors();
  } catch {
    mirrors.value = [];
  }
}

async function selectMirror(id: string) {
  if (settings.settings?.mirror === id) return;
  try {
    await settings.patch({ mirror: id });
  } catch (e) {
    message.error(String(e));
  }
}

async function testMirror(id: string, base: string) {
  if (testingMirror.value) return;
  testingMirror.value = id;
  try {
    const res = await api.testMirror(base);
    mirrorLatency.value = { ...mirrorLatency.value, [id]: res.ms };
  } catch (e) {
    mirrorLatency.value = { ...mirrorLatency.value, [id]: null };
    message.error(String(e));
  } finally {
    testingMirror.value = "";
  }
}

function onCustomMirrorInput() {
  if (settings.settings && settings.settings.mirror !== "custom") {
    settings.settings.mirror = "custom";
  }
}

const tab = ref("general");
/**
 * 刷新当前页的滑动指示器。
 * 面板切换有过渡动画（out-in），新面板在 enter 结束后才完成布局，
 * 因此必须在动画结束后测量，否则矩形为 0、指示器位置错乱。
 */
function refreshCurrentPaneIndicators() {
  const val = tab.value;
  if (val === "appearance") {
    refreshThemeSeg();
    refreshOrientationSeg();
  }
  // 「下载代理」seg 位于「内容服务」页，不是「下载」页
  if (val === "content") refreshProxyModeSeg();
}
watch(tab, () => {
  nextTick(refreshCurrentPaneIndicators);
});

const tabs = [
  { key: "general", label: "常规", icon: IconSliders },
  { key: "appearance", label: "外观", icon: IconImage },
  { key: "download", label: "下载", icon: IconDownload },
  { key: "content", label: "内容服务", icon: IconGlobe },
  { key: "game", label: "游戏内", icon: IconPlay },
  { key: "storage", label: "存储", icon: IconRefresh },
  { key: "about", label: "关于", icon: IconFile },
];

/** 界面缩放滑块：立刻改本地值（App.vue 的 watch 会即时应用）并持久化。 */
function onUiScale(v: number) {
  if (!Number.isFinite(v)) return;
  if (settings.settings) settings.settings.ui_scale = v;
  void settings.patch({ ui_scale: v });
}

const { memTotal, memUsed, memAvailable, startPolling, stopPolling } = useMemoryInfo();

// ---------------------------------------------------------------- 游戏内设置
/**
 * 移植过来的 Pojav 控制层设置。
 *
 * 这些项由 `LauncherPreferences` 从安卓 `SharedPreferences("launcher_preferences")`
 * 读取，和 QookiX 自己的 settings.json 是两套东西 —— 之前没有任何界面能改它们，
 * 所以「按钮大小 / 鼠标速度 / 渲染分辨率缩放 / 忽略刘海 / 长按判定 / 陀螺仪…」
 * 全都停在默认值。现在通过 `get_pojav_prefs` / `set_pojav_prefs` 这两个命令读写。
 */
const POJAV_DEFAULTS: Record<string, number | boolean | string> = {
  // 渲染器：opengles2 = GL4ES（默认，兼容性最好）；vulkan_zink = Zink + Turnip（实验性）
  renderer: "opengles2",
  resolutionRatio: 100,
  buttonscale: 100,
  mousescale: 100,
  mousespeed: 100,
  timeLongPressTrigger: 300,
  gamepad_deadzone_scale: 100,
  gyroSensitivity: 100,
  ignoreNotch: false,
  mouse_start: false,
  disableGestures: false,
  disableDoubleTap: false,
  alternate_surface: true,
  sustained_performance: false,
  buttonAllCaps: false,
  enableGyro: false,
  gyroInvertX: false,
  gyroInvertY: false,
};
const pojav = ref<Record<string, number | boolean | string>>({ ...POJAV_DEFAULTS });

/** 渲染后端选项。这三个都是「本包真的带了库」的：
 *  GL4ES = libgl4es_114.so；Zink = libOSMesa.so + libvulkan_freedreno.so（Turnip）；
 *  MobileGlues（MG）= libmobileglues.so —— ZL/FCL 同款渲染器（OpenGL → GLES 3.2）。 */
const rendererOptions = [
  { label: "GL4ES（兼容，推荐）", value: "opengles2" },
  { label: "MobileGlues（ZL/FCL 同款）", value: "mobileglues" },
  { label: "Zink + Turnip（实验）", value: "vulkan_zink" },
];

async function loadPojav() {
  try {
    const r = await api.getPojavPrefs();
    pojav.value = { ...POJAV_DEFAULTS, ...(r as Record<string, number | boolean | string>) };
  } catch {
    /* 原生桥未就绪时保持默认值，不打扰用户 */
  }
}

async function savePojav(key: string, value: number | boolean | string) {
  pojav.value[key] = value;
  try {
    await api.setPojavPrefs({ [key]: value });
  } catch (e) {
    message.error(String(e));
  }
}

  // 关于页：许可与版权声明（与桌面端同一种分组方式）。
  // 只列**随包分发**或**直接依赖**的开源项目；许可徽章以各项目官方声明为准。
  type AboutDep = { name: string; version: string; license: string; url: string; licenseUrl: string };
  const aboutDeps: Record<"frontend" | "rust" | "bundled", AboutDep[]> = {
    frontend: [
      { name: "Vue", version: "3.5", license: "MIT", url: "https://vuejs.org", licenseUrl: "https://github.com/vuejs/core/blob/main/LICENSE" },
      { name: "Vue Router", version: "4.4", license: "MIT", url: "https://router.vuejs.org", licenseUrl: "https://github.com/vuejs/router/blob/main/LICENSE" },
      { name: "Naive UI", version: "2.45", license: "MIT", url: "https://www.naiveui.com", licenseUrl: "https://github.com/tusen-design/naive-ui/blob/main/LICENSE" },
      { name: "Pinia", version: "2.2", license: "MIT", url: "https://pinia.vuejs.org", licenseUrl: "https://github.com/vuejs/pinia/blob/v2/LICENSE" },
      { name: "Tauri API", version: "2", license: "MIT/Apache-2.0", url: "https://tauri.app", licenseUrl: "https://github.com/tauri-apps/tauri/blob/dev/LICENSE" },
      { name: "skinview3d", version: "3.4", license: "MIT", url: "https://github.com/bs-community/skinview3d", licenseUrl: "https://github.com/bs-community/skinview3d/blob/master/LICENSE" },
    ],
    rust: [
      { name: "Tauri", version: "2", license: "MIT/Apache-2.0", url: "https://tauri.app", licenseUrl: "https://github.com/tauri-apps/tauri/blob/dev/LICENSE" },
      { name: "Tokio", version: "1", license: "MIT", url: "https://tokio.rs", licenseUrl: "https://github.com/tokio-rs/tokio/blob/master/LICENSE" },
      { name: "reqwest", version: "0.12", license: "MIT/Apache-2.0", url: "https://github.com/seanmonstar/reqwest", licenseUrl: "https://github.com/seanmonstar/reqwest/blob/master/LICENSE" },
      { name: "serde", version: "1", license: "MIT/Apache-2.0", url: "https://serde.rs", licenseUrl: "https://github.com/serde-rs/serde/blob/master/LICENSE" },
      { name: "jni", version: "0.21", license: "MIT/Apache-2.0", url: "https://github.com/jni-rs/jni-rs", licenseUrl: "https://github.com/jni-rs/jni-rs/blob/master/LICENSE" },
    ],
    bundled: [
      // 这些组件随 APK 分发，是启动器能跑起来的地基 —— 按各自协议保留署名。
      { name: "PojavLauncher", version: "", license: "GPL-3.0", url: "https://github.com/PojavLauncherTeam/PojavLauncher", licenseUrl: "https://github.com/PojavLauncherTeam/PojavLauncher" },
      { name: "GL4ES", version: "1.1.5", license: "MIT", url: "https://github.com/ptitSeb/gl4es", licenseUrl: "https://github.com/ptitSeb/gl4es/blob/master/LICENSE" },
      { name: "LWJGL 3", version: "3.3", license: "BSD-3-Clause", url: "https://lwjgl.org", licenseUrl: "https://github.com/LWJGL/lwjgl3/blob/master/LICENSE" },
      { name: "Caciocavallo", version: "", license: "GPL-2.0 + Classpath", url: "https://github.com/PojavLauncherTeam/caciocavallo", licenseUrl: "https://github.com/PojavLauncherTeam/caciocavallo" },
      { name: "OpenJDK", version: "17", license: "GPL-2.0 + Classpath", url: "https://openjdk.org", licenseUrl: "https://openjdk.org/legal/gplv2+ce.html" },
      { name: "Mesa（OSMesa / zink）", version: "", license: "MIT", url: "https://mesa3d.org", licenseUrl: "https://gitlab.freedesktop.org/mesa/mesa/-/blob/main/docs/license.rst" },
      { name: "Turnip（Mesa Vulkan 驱动）", version: "", license: "MIT", url: "https://gitlab.freedesktop.org/mesa/mesa", licenseUrl: "https://gitlab.freedesktop.org/mesa/mesa/-/blob/main/docs/license.rst" },
      { name: "FreeType", version: "2.13", license: "FreeType/GPL-2.0", url: "https://freetype.org", licenseUrl: "https://gitlab.freedesktop.org/freetype/freetype/-/blob/master/docs/FTL.TXT" },
      { name: "OpenAL Soft", version: "", license: "LGPL-2.1", url: "https://openal-soft.org", licenseUrl: "https://github.com/kcat/openal-soft/blob/master/COPYING" },
      { name: "MobileGlues", version: "", license: "", url: "https://github.com/MobileGL-Dev/MobileGlues-release", licenseUrl: "https://github.com/MobileGL-Dev/MobileGlues-release" },
    ],
  };
  const aboutGroupLabels: Record<"frontend" | "rust" | "bundled", string> = {
    frontend: "前端",
    rust: "Rust",
    bundled: "随包分发的组件",
  };

function onPojavRange(key: string, v: number) {
  if (!Number.isFinite(v)) return;
  void savePojav(key, v);
}

const effectiveMemory = computed(() => {
  if (settings.settings?.memory_mode === "auto") {
    // Base: 40% of available (min 2048 MB), cap at 75% of available
    const cap = Math.max(512, Math.floor(memAvailable.value * 3 / 4));
    return Math.max(512, Math.min(Math.max(2048, Math.floor(memAvailable.value * 40 / 100)), cap, 8192));
  }
  return settings.settings?.max_memory_mb ?? 4096;
});
const usedPercent = computed(() => {
  if (!memTotal.value) return 0;
  return Math.min(100, Math.round((memUsed.value / memTotal.value) * 100));
});
const allocPercent = computed(() => {
  if (!memTotal.value) return 0;
  return Math.min(100, Math.round((effectiveMemory.value / memTotal.value) * 100));
});
const allocStart = computed(() => usedPercent.value);
const allocWidth = computed(() =>
  Math.max(0, Math.min(allocPercent.value, 100 - usedPercent.value))
);

let saveTimer: ReturnType<typeof setTimeout> | null = null;

async function save() {
  try {
    skipNextSave = true;
    await settings.save();
  } catch (e) {
    message.error(String(e));
  }
}

let skipNextSave = true;
watch(
  () => settings.settings,
  () => {
    if (skipNextSave) {
      skipNextSave = false;
      return;
    }
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(save, 500);
  },
  { deep: true }
);

async function pickBackground() {
  try {
    const picked = await open({
      multiple: false,
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp"] }],
    });
    if (!picked || typeof picked !== "string") return;
    const path = await api.importBackgroundImage(picked);
    await settings.patch({ background_image: path });
  } catch (e) {
    message.error(String(e));
  }
}

const bgPreviewUrl = computed(() => {
  const p = settings.settings?.background_image;
  return p ? convertFileSrc(p) : "";
});

// ---------------------------------------------------------------------------
// 存储统计
// ---------------------------------------------------------------------------
const stats = ref<StorageStats | null>(null);
const loadingStats = ref(false);
const clearing = ref(false);

const DONUT_COLORS: Record<string, string> = {
  instances: "var(--accent)",
  servers: "#f59e0b",
  libraries: "#5aa2f0",
  assets: "#4ec9a0",
  versions: "#8b5cf6",
  runtime: "#ec4899",
  logs: "#94a3b8",
  other: "#64748b",
  launcher: "#22d3ee",
};
const DONUT_R = 74;
const DONUT_C = 2 * Math.PI * DONUT_R;

function pct(size: number): number {
  if (!stats.value || !stats.value.total) return 0;
  return Math.round((size / stats.value.total) * 1000) / 10;
}

const visibleCats = computed(() => (stats.value?.categories ?? []).filter((c) => c.size > 0));

/** 环形饼图各分段：start 偏移累积，返回 dash/offset 供 SVG stroke-dasharray 使用 */
const donutSegs = computed(() => {
  const total = stats.value?.total ?? 0;
  if (!total) return [];
  let acc = 0;
  return visibleCats.value.map((c) => {
    const frac = c.size / total;
    const dash = Math.max(0, frac * DONUT_C - 1.5);
    const seg = { key: c.key, dash, offset: -acc, color: DONUT_COLORS[c.key] ?? "#64748b" };
    acc += frac * DONUT_C;
    return seg;
  });
});

async function loadStats() {
  loadingStats.value = true;
  try {
    stats.value = await api.getStorageStats();
  } catch (e) {
    message.error("加载存储统计失败：" + String(e));
  } finally {
    loadingStats.value = false;
  }
}

async function refreshStats() {
  loadingStats.value = true;
  try {
    stats.value = await api.refreshStorageStats();
    message.success("已更新存储统计");
  } catch (e) {
    message.error("更新存储统计失败：" + String(e));
  } finally {
    loadingStats.value = false;
  }
}

function confirmClear() {
  dialog.warning({
    title: "清除缓存",
    content:
      "将清理 Java 下载临时文件、Java 检测缓存等可安全删除的缓存，不会影响任何实例、库、资源或版本文件。确定继续吗？",
    positiveText: "清除",
    negativeText: "取消",
    onPositiveClick: async () => {
      clearing.value = true;
      try {
        const res = await api.clearCache();
        message.success(`已清除缓存，释放 ${fmtSize(res.freed)}`);
        await refreshStats();
      } catch (e) {
        message.error("清除缓存失败：" + String(e));
      } finally {
        clearing.value = false;
      }
    },
  });
}

onMounted(() => {
  settings.load();
  loadPojav();
  loadMirrors();
  void refreshSystemProxy();
  startPolling();
  loadStats();
});
onUnmounted(() => {
  stopPolling();
  if (saveTimer) clearTimeout(saveTimer);
});
</script>

<template>
  <div v-if="settings.settings" class="settings-view">
    <aside class="settings-nav">
      <nav class="nav-list">
        <button
          v-for="t in tabs"
          :key="t.key"
          class="nav-item"
          :class="{ active: tab === t.key }"
          @click="tab = t.key"
        >
          <component :is="t.icon" class="nav-icon" />
          <span>{{ t.label }}</span>
        </button>
      </nav>
    </aside>

    <Transition name="settings-pane" mode="out-in" @after-enter="refreshCurrentPaneIndicators">
    <div :key="tab" class="settings-body">
      <!-- 常规 -->
      <div v-show="tab === 'general'" class="settings-pane">
        <!-- 游戏统计已挪到首页英雄卡左下角（用户 2026-09-22 要求） -->
        <div class="grid">
          <div class="card glass">
            <h3>内存分配（默认值）</h3>
            <div class="mem-mode-row">
              <n-radio-group v-model:value="settings.settings.memory_mode" size="small">
                <n-radio-button value="auto">自动配置</n-radio-button>
                <n-radio-button value="custom">手动配置</n-radio-button>
              </n-radio-group>
            </div>
            <div v-if="settings.settings.memory_mode !== 'auto'" class="mem-row">
              <div>
                <label>最大内存</label>
                <!-- 表单控件一律用 UI 库（naive-ui）组件，别再手搓原生 input -->
                <n-slider
                  v-model:value="settings.settings.max_memory_mb"
                  :min="1024"
                  :max="16384"
                  :step="256"
                />
                <div class="mem-val">{{ settings.settings.max_memory_mb }} MB</div>
              </div>
            </div>
            <div class="mem-gauge">
              <div class="mem-gauge-track">
                <div class="mem-gauge-used" :style="{ width: usedPercent + '%' }"></div>
                <div
                  class="mem-gauge-alloc"
                  :style="{ left: allocStart + '%', width: allocWidth + '%' }"
                ></div>
              </div>
              <div class="mem-gauge-labels">
                <span><i class="dot used"></i>已使用 {{ fmtMem(memUsed) }}（{{ usedPercent }}%）</span>
                <span><i class="dot alloc"></i>游戏分配 {{ fmtMem(effectiveMemory) }}（{{ allocPercent }}%）</span>
                <span><i class="dot total"></i>总内存 {{ fmtMem(memTotal) }} / 可用 {{ fmtMem(memAvailable) }}</span>
              </div>
            </div>
          </div>

          <div class="card glass">
            <h3>JVM 参数（额外，默认值）</h3>
            <n-input
              v-model:value="settings.settings.jvm_args"
              type="textarea"
              :rows="3"
              placeholder="例如：-XX:+UseG1GC -XX:MaxGCPauseMillis=50"
            />
          </div>

          <div class="card glass">
            <h3>游戏参数（额外，默认值）</h3>
            <n-input
              v-model:value="settings.settings.game_args"
              placeholder="例如：--fullscreen"
            />
          </div>
        </div>
      </div>

      <!-- 外观 -->
      <div v-show="tab === 'appearance'" class="settings-pane">
        <div class="card glass">
          <h3>主题</h3>
          <div class="choice-row">
            <span>主题</span>
            <div ref="themeSegRef" class="seg">
              <div class="indicator" :style="themeSegStyle"></div>
              <button
                :class="{ active: settings.settings.theme === 'dark' }"
                @click="settings.patch({ theme: 'dark' })"
              >
                深色
              </button>
              <button
                :class="{ active: settings.settings.theme === 'light' }"
                @click="settings.patch({ theme: 'light' })"
              >
                浅色
              </button>
            </div>
          </div>
          <div class="appearance-divider"></div>
          <div class="choice-row">
            <span>主题色</span>
            <div class="theme-color-row">
              <button
                v-for="c in themeColorPresets"
                :key="c"
                type="button"
                class="color-swatch"
                :class="{ active: settings.settings.theme_color === c }"
                :style="{ background: c }"
                :title="c" :aria-label="c"
                @click="settings.patch({ theme_color: c })"
              ></button>
              <label class="color-custom" title="自定义颜色">
                <span class="color-custom-ring" :style="{ background: settings.settings.theme_color }"></span>
                <n-color-picker
                  :value="settings.settings.theme_color"
                  :show-alpha="false"
                  size="small"
                  style="width: 100%"
                  @update:value="onThemeColorInput"
                />
              </label>
            </div>
          </div>
        </div>
        <div class="card glass">
            <h3>界面</h3>
            <!-- 界面缩放：机型之间可视高度差别很大（853×384 / 792×360），
                 与其为每台机器写死尺寸，不如让用户自己调舒服的密度。 -->
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">界面缩放</span>
                <p class="choice-hint">
                  整体放大或缩小整个界面（字号、间距、控件一起变）。屏幕小就调小、看着累就调大，改完立即生效。
                </p>
              </div>
              <div class="scale-ctl">
                <n-slider
                  :value="settings.settings?.ui_scale ?? 100"
                  :min="60"
                  :max="150"
                  :step="5"
                  @update:value="onUiScale"
                />
                <div class="mem-val">{{ settings.settings?.ui_scale ?? 100 }}%</div>
              </div>
            </div>
          <div class="choice-row">
            <div class="choice-info">
              <span class="choice-label">导航栏位置</span>
              <p class="choice-hint">
                仅手机端生效。横屏时底部导航会占掉约 70px 高度，放到左侧可以把这段还给内容区。
              </p>
            </div>
            <div ref="navPositionSegRef" class="seg">
              <div class="indicator" :style="navPositionSegStyle"></div>
              <button
                v-for="m in navPositionModes"
                :key="m.id"
                :class="{ active: (settings.settings.nav_position ?? 'bottom') === m.id }"
                @click="selectNavPosition(m.id)"
              >
                {{ m.label }}
              </button>
            </div>
          </div>
          <div class="choice-row">
            <div class="choice-info">
              <span class="choice-label">挖孔 / 刘海</span>
              <p class="choice-hint">
                按手机竖屏时的摄像头位置选择（横屏会自动换算到对应侧边）。
                打孔默认让开 44px，可用下方「避让宽度」微调；
                刘海长短不一，建议按实际深度拖；「自动」跟随系统安全区，大多数机型够用。
              </p>
            </div>
            <div class="seg">
              <button
                v-for="m in navCutoutModes"
                :key="m.id"
                :class="{ active: (settings.settings.nav_cutout ?? 'auto') === m.id }"
                @click="selectNavCutout(m.id)"
              >
                {{ m.label }}
              </button>
            </div>
          </div>
          <div class="choice-row">
            <div class="choice-info">
              <span class="choice-label">触控目标大小</span>
              <p class="choice-hint">
                按钮 / 输入框的点击区域大小。界面本来就是紧凑的桌面尺寸，
                手指点不准就调大；默认「紧凑」= 保持原样。
              </p>
            </div>
            <n-radio-group
              :value="settings.settings.touch_target ?? 'compact'"
              size="small"
              @update:value="selectTouchTarget"
            >
              <n-radio-button v-for="m in touchTargetModes" :key="m.id" :value="m.id">
                {{ m.label }}
              </n-radio-button>
            </n-radio-group>
          </div>
          <div
            v-if="!['auto', 'none'].includes(settings.settings.nav_cutout ?? 'auto')"
            class="choice-row"
          >
            <div class="choice-info">
              <span class="choice-label">避让宽度</span>
              <p class="choice-hint">
                让开的宽度。打孔默认 44px 基本够用；刘海建议按实际深度拖。
              </p>
            </div>
            <div class="scale-ctl">
              <n-slider
                :value="settings.settings?.nav_offset ?? 44"
                :min="24"
                :max="120"
                :step="2"
                @update:value="onNavOffset"
              />
              <div class="mem-val">{{ settings.settings?.nav_offset ?? 44 }}px</div>
            </div>
          </div>
          <div class="choice-row">
            <div class="choice-info">
              <span class="choice-label">首页主标题卡片</span>
              <p class="choice-hint">控制首页顶部的主标题卡片是否显示，关闭后首页更加简洁。</p>
            </div>
            <n-switch
              :value="settings.settings.show_home_hero"
              @update:value="(v: boolean) => settings.patch({ show_home_hero: v })"
            />
          </div>
          <div class="choice-row">
            <div class="choice-info">
              <span class="choice-label">侧边栏折叠按钮</span>
              <p class="choice-hint">控制侧边栏底部的展开/收缩按钮是否显示，关闭后可保持侧边栏固定。</p>
            </div>
            <n-switch
              :value="settings.settings.show_sidebar_collapse_btn"
              @update:value="(v: boolean) => settings.patch({ show_sidebar_collapse_btn: v })"
            />
          </div>
          <div class="choice-row">
            <div class="choice-info">
              <span class="choice-label">侧边栏新闻入口</span>
              <p class="choice-hint">关闭后隐藏侧边栏的「新闻」入口与新闻页面。</p>
            </div>
            <n-switch
              :value="settings.settings.show_news ?? true"
              @update:value="(v: boolean) => settings.patch({ show_news: v })"
            />
          </div>
          <div class="choice-row">
            <div class="choice-info">
              <span class="choice-label">屏幕方向</span>
              <p class="choice-hint">启动器界面默认锁定横屏，切换后立即生效（游戏内方向由游戏自身控制）。</p>
            </div>
            <div ref="orientationSegRef" class="seg">
              <div class="indicator" :style="orientationSegStyle"></div>
              <button
                v-for="m in orientationModes"
                :key="m.id"
                :class="{ active: (settings.settings.orientation ?? 'system') === m.id }"
                @click="selectOrientation(m.id)"
              >
                {{ m.label }}
              </button>
            </div>
          </div>
        </div>
        <div class="card glass">
          <h3>背景图片</h3>
          <div v-if="settings.settings.background_image" class="bg-preview">
            <img :src="bgPreviewUrl" alt="背景预览" />
          </div>
          <div class="choice-row">
            <span>背景图片</span>
            <div class="bg-actions">
              <button class="mini-btn" @click="pickBackground">选择图片</button>
              <button
                v-if="settings.settings.background_image"
                class="mini-btn"
                @click="settings.patch({ background_image: null })"
              >
                清除
              </button>
            </div>
          </div>
          <div v-if="settings.settings.background_image" class="tune-block">
            <div class="tune-row">
              <label>背景模糊</label>
              <n-slider
                v-model:value="settings.settings.background_blur"
                :min="0"
                :max="50"
                :step="1"
              />
              <span class="tune-val">{{ settings.settings.background_blur }} px</span>
            </div>
            <div class="tune-row">
              <label>背景遮罩</label>
              <n-slider
                v-model:value="settings.settings.background_dim"
                :min="0"
                :max="100"
                :step="5"
              />
              <span class="tune-val">{{ settings.settings.background_dim }}%</span>
            </div>
          </div>
        </div>
        <div class="card glass">
          <h3>磨砂卡片</h3>
          <div class="tune-row">
            <label>磨砂强度</label>
            <n-slider
              v-model:value="settings.settings.glass_blur"
              :min="0"
              :max="30"
              :step="1"
            />
            <span class="tune-val">{{ settings.settings.glass_blur }} px</span>
          </div>
          <p class="hint">调节卡片毛玻璃模糊半径，数值越大磨砂越强。</p>
        </div>
      </div>

      <!-- 下载 -->
      <div v-show="tab === 'download'" class="settings-pane">
        <div class="grid">
          <!-- 「并行下载」整张卡片先隐藏。
               这两个滑杆目前是**假开关**：`download::download_libraries` 是逐个 `await`
               的串行循环，单文件分片（HTTP Range 并行）根本没实现 —— 拖动它们不会
               改变任何下载行为。假开关比缺功能更伤（用户会以为自己调过了、在排查为什么没用）。
               真要做的话：给 `download_libraries` 上并发（信号量限流），分片下载则要
               在 `download::stream_to_file` 里按 Range 拆段并合并哈希。
               实现完把 DOWNLOAD_SLIDERS_ENABLED 打开即可。 -->
          <div v-if="DOWNLOAD_SLIDERS_ENABLED" class="card glass">
            <h3>并行下载</h3>
            <label class="row-label">
              同时下载文件数：{{ settings.settings.download_threads }}
              <n-slider
                v-model:value="settings.settings.download_threads"
                :min="1"
                :max="32"
                :step="1"
              />
            </label>
            <p class="hint">同时从服务器下载的文件数量。值越大并发越高，但对服务器压力也越大。</p>
            <label class="row-label" style="margin-top: 16px;">
              单文件分片线程数：{{ settings.settings.download_chunk_threads }}
              <n-slider
                v-model:value="settings.settings.download_chunk_threads"
                :min="1"
                :max="16"
                :step="1"
              />
            </label>
            <p class="hint">对单个大文件使用 HTTP Range 分片并行下载的线程数。仅对支持断点续传的服务器生效，小文件始终单线程。</p>
          </div>
          <div class="card glass">
            <h3><IconGlobe /> 下载镜像源</h3>
            <div class="mirror-list">
              <button
                v-for="m in mirrors"
                :key="m.id"
                type="button"
                class="mirror-item"
                :class="{ active: settings.settings.mirror === m.id }"
                @click="selectMirror(m.id)"
              >
                <span class="mirror-main">
                  <span class="mirror-name">{{ m.label }}</span>
                  <span class="mirror-base">{{ m.base || "直接使用各官方地址" }}</span>
                </span>
                <span class="mirror-side">
                  <span
                    v-if="mirrorLatency[m.id] !== undefined"
                    class="mirror-ms"
                    :class="{ bad: mirrorLatency[m.id] === null }"
                  >
                    {{ mirrorLatency[m.id] === null ? "不可用" : `${mirrorLatency[m.id]} ms` }}
                  </span>
                  <span
                    class="mirror-btn"
                    :class="{ disabled: testingMirror === m.id }"
                    @click.stop="testMirror(m.id, m.base)"
                  >
                    {{ testingMirror === m.id ? "测试中…" : "测速" }}
                  </span>
                </span>
              </button>
              <div
                class="mirror-custom"
                :class="{ active: settings.settings.mirror === 'custom' }"
              >
                <label class="mirror-custom-head">
                  <n-radio
                    :checked="settings.settings.mirror === 'custom'"
                    @update:checked="selectMirror('custom')"
                  />
                  <span>自定义镜像</span>
                </label>
                <n-input
                  v-model:value="settings.settings.mirror_custom"
                  placeholder="https://your-mirror.example.com"
                  @update:value="onCustomMirrorInput"
                />
                <span
                  class="mirror-btn"
                  :class="{ disabled: testingMirror === 'custom' || !settings.settings.mirror_custom }"
                  @click="testMirror('custom', settings.settings.mirror_custom)"
                >
                  {{ testingMirror === 'custom' ? "测试中…" : "测速" }}
                </span>
              </div>
            </div>
            <p class="hint">
              加速游戏本体、资源文件与依赖库（Forge / Fabric / NeoForge）的下载，
              <b>切换后立即生效，无需重启</b>；镜像缺失文件时会自动回退官方地址。
              自定义镜像需兼容 BMCLAPI 接口，直接填写根地址即可。
            </p>
          </div>
          <div class="card glass">
            <h3>下载中心</h3>
            <p class="hint">所有安装与下载任务可在左侧「下载中心」实时查看进度、速度与剩余文件。</p>
          </div>
        </div>
      </div>

      <!-- 内容服务 -->
      <div v-show="tab === 'content'" class="settings-pane">
        <div class="grid">
          <div class="card glass">
            <h3>CurseForge API Key</h3>
            <n-input
              v-model:value="settings.settings.curseforge_api_key"
              placeholder="在 console.curseforge.com 免费申请"
            />
            <p class="hint">可选。不填使用默认key 可能会导致 CurseForge 内容中心不可用，Modrinth 不受影响。</p>
          </div>
          <div class="card glass">
            <h3>下载代理</h3>
            <div class="proxy-row">
              <div ref="proxyModeSegRef" class="seg">
                <div class="indicator" :style="proxyModeSegStyle"></div>
                <button
                  v-for="m in proxyModes"
                  :key="m.id"
                  :class="{ active: settings.settings.proxy_mode === m.id }"
                  @click="selectProxyMode(m.id)"
                >
                  {{ m.label }}
                </button>
              </div>
              <button
                class="mirror-btn proxy-test-btn"
                :class="{ disabled: testingProxy }"
                @click="testProxy"
              >
                {{ testingProxy ? "测试中…" : "测试连接" }}
              </button>
            </div>
            <n-input
              v-if="settings.settings.proxy_mode === 'custom'"
              v-model:value="settings.settings.proxy"
              placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:1080"
              @update:value="onCustomProxyInput"
            />
            <p class="hint">
              {{
                settings.settings.proxy_mode === "system"
                  ? "使用系统网络代理设置（默认）。"
                  : settings.settings.proxy_mode === "direct"
                    ? "直连，不经过任何代理。"
                    : "自定义代理。用于绕过 CDN 下载失败（404/连接失败）。修改后需重启启动器生效。"
              }}
            </p>
            <!-- 原生请求不会自动走系统代理：代理软件开着却选了直连时，下载会大面积失败 -->
            <p v-if="systemProxy" class="hint" :class="{ 'hint-warn': settings.settings.proxy_mode === 'direct' }">
              已检测到系统代理：{{ systemProxy }}<template
                v-if="settings.settings.proxy_mode === 'direct'"
              >　—　当前为直连，下载与延迟测试可能全部失败，建议改选「系统代理」。</template>
            </p>
            <p v-else class="hint">未检测到系统代理。</p>
          </div>
        </div>
      </div>

      <!-- 存储 -->
      <div v-show="tab === 'storage'" class="settings-pane">
        <div class="grid storage-grid">
          <div class="card glass storage-card">
            <div class="storage-header">
              <h3>存储统计</h3>
              <div class="storage-actions">
                <span class="hint-inline">
                  <template v-if="stats">{{ stats.cached ? "上次更新" : "已更新" }}：{{ fmtTime(stats.updated_at) }}</template>
                  <template v-else>尚未扫描</template>
                </span>
                <button class="mini-btn" :disabled="loadingStats" @click="refreshStats">
                  <IconRefresh class="btn-icon" />
                  {{ loadingStats ? "扫描中…" : "更新" }}
                </button>
              </div>
            </div>

            <div v-if="stats && stats.total > 0" class="storage-body">
              <div class="donut-wrap">
                <svg viewBox="0 0 200 200" class="donut">
                  <circle
                    v-for="seg in donutSegs"
                    :key="seg.key"
                    cx="100"
                    cy="100"
                    r="74"
                    fill="none"
                    :stroke="seg.color"
                    stroke-width="30"
                    :stroke-dasharray="`${seg.dash} ${DONUT_C - seg.dash}`"
                    :stroke-dashoffset="seg.offset"
                    transform="rotate(-90 100 100)"
                  />
                </svg>
                <div class="donut-center">
                  <span class="donut-total">{{ fmtSize(stats.total) }}</span>
                  <span class="donut-label">总占用</span>
                </div>
              </div>

              <ul class="storage-legend">
                <li v-for="cat in visibleCats" :key="cat.key">
                  <span class="legend-dot" :style="{ background: DONUT_COLORS[cat.key] ?? '#64748b' }"></span>
                  <span class="legend-name">{{ cat.label }}</span>
                  <span class="legend-size">{{ fmtSize(cat.size) }}</span>
                  <span class="legend-pct">{{ pct(cat.size) }}%</span>
                </li>
              </ul>
            </div>

            <div v-if="stats && stats.instances.length" class="instance-storage">
              <h4 class="instance-storage-title">
                每个实例
                <span class="hint-inline">{{ stats.instances.length }} 个</span>
              </h4>
              <ul class="instance-storage-list">
                <li v-for="inst in stats.instances" :key="inst.id">
                  <span class="instance-name" :title="inst.name">{{ inst.name }}</span>
                  <span class="legend-size">{{ fmtSize(inst.size) }}</span>
                  <span class="legend-pct">{{ pct(inst.size) }}%</span>
                </li>
              </ul>
            </div>

            <div v-if="stats && stats.servers.length" class="instance-storage">
              <h4 class="instance-storage-title">
                每个服务器
                <span class="hint-inline">{{ stats.servers.length }} 个</span>
              </h4>
              <ul class="instance-storage-list">
                <li v-for="srv in stats.servers" :key="srv.id">
                  <span class="instance-name" :title="srv.name">{{ srv.name }}</span>
                  <span class="legend-size">{{ fmtSize(srv.size) }}</span>
                  <span class="legend-pct">{{ pct(srv.size) }}%</span>
                </li>
              </ul>
            </div>
            <p v-else-if="!stats?.instances.length" class="hint">{{ stats ? "暂无可统计的数据" : "正在加载存储统计…" }}</p>

            <div class="storage-footer">
              <button class="mini-btn danger" :disabled="clearing" @click="confirmClear">
                <IconTrash class="btn-icon" />
                {{ clearing ? "清理中…" : "清除缓存" }}
              </button>
              <span class="hint">清理 Java 下载临时文件、Java 检测缓存等可安全删除的缓存，不会影响实例、库、资源或版本文件。</span>
            </div>
          </div>
        </div>
      </div>
      <!-- 游戏内（移植自 Pojav 的控制层设置） -->
      <div v-show="tab === 'game'" class="settings-pane">
        <div class="grid">
          <div class="card glass">
            <h3><IconPlay /> 渲染</h3>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">渲染器</span>
                <p class="choice-hint">
                  <b>GL4ES</b>：把 OpenGL 翻译成 GLES，兼容性最好，默认选它。<br />
                  <b>MobileGlues</b>：Zalith/FCL 同款新渲染器（OpenGL → GLES 3.2），
                  性能与画质都不错，部分老模组可能不兼容。<br />
                  <b>Zink + Turnip</b>：OpenGL → Vulkan（Mesa Zink + Turnip 驱动），
                  在 Adreno 设备上通常帧率更高、支持更多现代特性；但部分设备会黑屏/花屏，
                  遇到问题就切回 GL4ES。切换后需要重新启动游戏生效。
                </p>
              </div>
              <n-select
                :value="(pojav.renderer as string) ?? 'opengles2'"
                :options="rendererOptions"
                size="small"
                style="width: 190px"
                @update:value="(v: string) => savePojav('renderer', v)"
              />
            </div>
            <div class="mem-row">
              <div>
                <label>渲染分辨率缩放</label>
                <n-slider
                  :value="Number(pojav.resolutionRatio ?? 100)"
                  :min="50"
                  :max="150"
                  :step="5"
                  @update:value="(v: number) => onPojavRange('resolutionRatio', v)"
                />
                <div class="mem-val">{{ pojav.resolutionRatio }}% —— 越低越流畅，画面越糊</div>
              </div>
            </div>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">忽略刘海</span>
                <p class="choice-hint">把屏幕顶部刘海区域也让给游戏画面</p>
              </div>
              <n-switch :value="!!pojav.ignoreNotch" @update:value="(v: boolean) => savePojav('ignoreNotch', v)" />
            </div>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">备选渲染表面</span>
                <p class="choice-hint">画面异常（黑屏 / 花屏 / 切后台回来不刷新）时可以切换试试</p>
              </div>
              <n-switch
                :value="pojav.alternate_surface !== false"
                @update:value="(v: boolean) => savePojav('alternate_surface', v)"
              />
            </div>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">持续性能模式</span>
                <p class="choice-hint">向系统申请持续性能，减少长时间游玩后的降频（部分设备无效）</p>
              </div>
              <n-switch
                :value="!!pojav.sustained_performance"
                @update:value="(v: boolean) => savePojav('sustained_performance', v)"
              />
            </div>
          </div>

          <div class="card glass">
            <h3>操作</h3>
            <div class="mem-row">
              <div>
                <label>按钮大小</label>
                <n-slider
                  :value="Number(pojav.buttonscale ?? 100)"
                  :min="50"
                  :max="200"
                  :step="10"
                  @update:value="(v: number) => onPojavRange('buttonscale', v)"
                />
                <div class="mem-val">{{ pojav.buttonscale }}%</div>
              </div>
            </div>
            <div class="mem-row">
              <div>
                <label>鼠标（视角）速度</label>
                <n-slider
                  :value="Number(pojav.mousespeed ?? 100)"
                  :min="50"
                  :max="200"
                  :step="10"
                  @update:value="(v: number) => onPojavRange('mousespeed', v)"
                />
                <div class="mem-val">{{ pojav.mousespeed }}%</div>
              </div>
            </div>
            <div class="mem-row">
              <div>
                <label>长按判定时间</label>
                <n-slider
                  :value="Number(pojav.timeLongPressTrigger ?? 300)"
                  :min="150"
                  :max="800"
                  :step="50"
                  @update:value="(v: number) => onPojavRange('timeLongPressTrigger', v)"
                />
                <div class="mem-val">{{ pojav.timeLongPressTrigger }} ms —— 拖拽/放下方块的长按阈值</div>
              </div>
            </div>
            <!-- 手柄死区（默认隐藏，见 script 里 GAMEPAD_DEADZONE_ENABLED 的说明） -->
            <div v-if="GAMEPAD_DEADZONE_ENABLED" class="mem-row">
              <div>
                <label>手柄死区</label>
                <n-slider
                  :value="Number(pojav.gamepad_deadzone_scale ?? 100)"
                  :min="0"
                  :max="200"
                  :step="10"
                  @update:value="(v: number) => onPojavRange('gamepad_deadzone_scale', v)"
                />
                <div class="mem-val">{{ pojav.gamepad_deadzone_scale }}%</div>
              </div>
            </div>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">进游戏自动开虚拟鼠标</span>
                <p class="choice-hint">适合在游戏里点界面 / 配置整合包</p>
              </div>
              <n-switch :value="!!pojav.mouse_start" @update:value="(v: boolean) => savePojav('mouse_start', v)" />
            </div>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">双击交换左右手</span>
                <p class="choice-hint">关闭后双击不会把主手/副手对调</p>
              </div>
              <n-switch
                :value="!!pojav.disableDoubleTap"
                @update:value="(v: boolean) => savePojav('disableDoubleTap', v)"
              />
            </div>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">禁用手势</span>
                <p class="choice-hint">关闭所有滑动 / 双击手势，只保留按钮与触控板</p>
              </div>
              <n-switch
                :value="!!pojav.disableGestures"
                @update:value="(v: boolean) => savePojav('disableGestures', v)"
              />
            </div>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">按钮文字全大写</span>
                <p class="choice-hint">控制布局里按钮标签是否强制大写</p>
              </div>
              <n-switch
                :value="!!pojav.buttonAllCaps"
                @update:value="(v: boolean) => savePojav('buttonAllCaps', v)"
              />
            </div>
          </div>

          <div class="card glass">
            <h3>陀螺仪</h3>
            <div class="choice-row">
              <div class="choice-info">
                <span class="choice-label">启用陀螺仪</span>
                <p class="choice-hint">用手机姿态控制视角（需要设备带陀螺仪）</p>
              </div>
              <n-switch :value="!!pojav.enableGyro" @update:value="(v: boolean) => savePojav('enableGyro', v)" />
            </div>
            <div class="mem-row">
              <div>
                <label>灵敏度</label>
                <n-slider
                  :value="Number(pojav.gyroSensitivity ?? 100)"
                  :min="50"
                  :max="300"
                  :step="10"
                  @update:value="(v: number) => onPojavRange('gyroSensitivity', v)"
                />
                <div class="mem-val">{{ pojav.gyroSensitivity }}%</div>
              </div>
            </div>
            <div class="choice-row">
              <div class="choice-info"><span class="choice-label">反转 X 轴</span></div>
              <n-switch :value="!!pojav.gyroInvertX" @update:value="(v: boolean) => savePojav('gyroInvertX', v)" />
            </div>
            <div class="choice-row">
              <div class="choice-info"><span class="choice-label">反转 Y 轴</span></div>
              <n-switch :value="!!pojav.gyroInvertY" @update:value="(v: boolean) => savePojav('gyroInvertY', v)" />
            </div>
          </div>
        </div>
      </div>

      <!-- 关于 -->
      <div v-show="tab === 'about'" class="settings-pane">
        <div class="card glass about-showcase">
          <AboutShowcase />
          <div class="about-hero-title">
            <span class="about-name about-hero-name">QookiX Launcher Android</span>
            <span
              class="about-ver"
              :class="{ 'ver-arming': verHolding }"
              title="…别松手"
              @pointerdown="verDown"
              @pointerup="verCancel"
              @pointercancel="verCancel"
              @pointerleave="verCancel"
              @contextmenu.prevent
            >v1.0.0</span>
          </div>
          <p class="about-hero-slogan">现代化、简洁、无广告的 Minecraft 启动器</p>
        </div>
        <div class="grid about-grid">
          <div class="card glass about-card">
            <div class="about-devs-title">开发者</div>
            <div class="dev-list">
              <div class="dev-line">
                <img class="dev-avatar" :src="devZhayiUrl" alt="ZhaYi" />
                <div class="dev-meta">
                  <div class="dev-head">
                    <span class="dev-name">ZhaYi</span>
                    <n-tooltip trigger="hover" placement="top">
                      <template #trigger>
                        <button class="dev-github-btn" @click="openUrl('https://github.com/ZhaYi-Miao')">
                          <IconGithub />
                        </button>
                      </template>
                      GitHub 主页
                    </n-tooltip>
                  </div>
                  <span class="dev-role">QookiX-Launcher-Android 的开发者</span>
                </div>
              </div>
              <div class="dev-line">
                <img class="dev-avatar" :src="devWeimoshengUrl" alt="维墨笙" />
                <div class="dev-meta">
                  <div class="dev-head">
                    <span class="dev-name">维墨笙</span>
                    <n-tooltip trigger="hover" placement="top">
                      <template #trigger>
                        <button class="dev-github-btn" @click="openUrl('https://github.com/weimosheng')">
                          <IconGithub />
                        </button>
                      </template>
                      GitHub 主页
                    </n-tooltip>
                  </div>
                  <span class="dev-role">QookiX-Launcher-Android 的协力开发者</span>
                </div>
              </div>
            </div>
          </div>
          <div class="about-links-row">
            <button class="about-link" @click="openUrl('https://qookix.swkj1.cn/')">
              <span class="link-left"><IconGlobe /> 官方网站</span>
              <span class="link-arrow">→</span>
            </button>
            <button class="about-link" @click="openUrl('https://github.com/weimosheng/QookiX-Launcher')">
              <span class="link-left"><IconGithub /> GitHub 仓库</span>
              <span class="link-arrow">→</span>
            </button>
            <button class="about-link" @click="openUrl('https://github.com/weimosheng/QookiX-Launcher/issues')">
              <span class="link-left"><IconExternal /> 问题反馈</span>
              <span class="link-arrow">→</span>
            </button>
            <button class="about-link" @click="openUrl('https://github.com/weimosheng/QookiX-Launcher/releases')">
              <span class="link-left"><IconList /> 更新日志</span>
              <span class="link-arrow">→</span>
            </button>
            <button class="about-link" @click="openUrl('https://qm.qq.com/q/91keQnJ8dy')">
              <span class="link-left"><IconUsers /> 官方 Q 群</span>
              <span class="link-arrow">→</span>
            </button>
            <button class="about-link" @click="openUrl('https://afdian.com/a/qookix')">
              <span class="link-left"><IconHeart /> 爱发电赞助</span>
              <span class="link-arrow">→</span>
            </button>
          </div>
          <div class="card glass about-license-card">
            <h3>许可证</h3>
            <p class="license-text">
              QookiX Launcher 基于
              <span class="license-accent">GPL-3.0</span>
              开源协议发布。图标、名称与品牌归属 QookiX 开发组所有，未经许可请勿用于商业用途。
            </p>
            <button class="about-link" @click="openUrl('https://github.com/weimosheng/QookiX-Launcher/blob/main/LICENSE')">
              <span class="link-left"><IconFile /> 查看 GPL-3.0 完整文本</span>
              <span class="link-arrow">→</span>
            </button>
          </div>
          <div class="card glass about-deps-card">
            <h3>许可与版权声明</h3>
            <p class="license-text">QookiX Launcher 的构建得益于以下优秀的开源项目。</p>
            <div class="deps-groups">
              <div class="deps-group" v-for="(list, group) in aboutDeps" :key="group">
                <div class="deps-group-title">{{ aboutGroupLabels[group] }}</div>
                <div v-for="d in list" :key="d.name" class="about-dep-row">
                  <div class="dep-info">
                    <span class="dep-name">{{ d.name }}<span class="dep-ver" v-if="d.version">v{{ d.version }}</span></span>
                    <span class="dep-license" v-if="d.license">{{ d.license }}</span>
                  </div>
                  <div class="dep-links">
                    <button class="dep-link" @click="openUrl(d.url)">来源 ↗</button>
                    <button class="dep-link" @click="openUrl(d.licenseUrl)">许可 ↗</button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    </Transition>
    <!-- 版本彩蛋：全屏像素烟花 + 制作名单，点击任意处关闭。
         Teleport 到 body：脱离 #app 的 zoom/层叠上下文，才能真正盖住底部导航、
         且点击事件不会被设置页的滚动容器吞掉。 -->
    <Teleport to="body">
      <Transition name="veregg">
        <div v-if="verEgg" class="ver-egg" @pointerdown="verClose" @click="verClose">
          <div class="ver-sparks" aria-hidden="true">
            <span
              v-for="(s, i) in verSparks"
              :key="i"
              class="spark"
              :style="{
                left: s.left + '%',
                top: s.top + '%',
                width: s.size + 'px',
                height: s.size + 'px',
                background: s.color,
                animationDelay: s.delay + 's',
                animationDuration: s.dur + 's',
              }"
            ></span>
          </div>
          <div class="ver-credits">
            <div class="ver-credits-inner">
              <img class="vc-logo" :src="logoUrl" alt="" />
              <div class="vc-title">QookiX Launcher Android</div>
              <div class="vc-line">制作 · ZhaYi / Weimosheng</div>
              <div class="vc-line">感谢大家使用 QookiX，感谢你们的支持</div>
              <div class="vc-line">欢迎随时反馈问题与建议，一起把它做得更好</div>
            </div>
          </div>
          <div class="ver-hint">点击任意处关闭</div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.settings-view {
  display: flex;
  gap: 18px;
  align-items: flex-start;
}
.settings-nav {
  flex-shrink: 0;
  /* 与内容中心 / 实例详情的侧栏共用 `--rail-w`（见 styles.css）：
     三处侧栏宽度一致，且随视口自适应、不写死。
     顺带把原来 188px 收窄了 50px，还给右边卡片（竖屏下卡片最小 340px 更容易放得下）。 */
  width: var(--rail-w);
  position: sticky;
  top: 0;
  padding: 16px 14px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 14px;
  backdrop-filter: blur(var(--glass-blur, 8px));
  -webkit-backdrop-filter: blur(var(--glass-blur, 8px));
}
.nav-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.nav-item {
  display: flex;
  align-items: center;
  gap: 9px;
  text-align: left;
  border: none;
  background: transparent;
  color: var(--text-2);
  padding: 9px 12px;
  border-radius: 9px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
  transition: background 0.12s, color 0.12s;
}
.nav-icon {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  opacity: 0.85;
}
.nav-item:hover {
  background: var(--panel-hover);
  color: var(--text-1);
}
.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}
.nav-item.active .nav-icon {
  opacity: 1;
}
.settings-body {
  flex: 1;
  min-width: 0;
}
/* 子标签页切换动画 */
.settings-pane-enter-active {
  transition: opacity 0.22s ease, transform 0.26s cubic-bezier(0.22, 1, 0.36, 1);
}
.settings-pane-leave-active {
  transition: opacity 0.13s ease, transform 0.13s ease-in;
}
.settings-pane-enter-from {
  opacity: 0;
  transform: translateY(10px);
}
.settings-pane-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}
.settings-pane {
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  border: none;
  border-radius: 10px;
  padding: 10px 18px;
  font-size: 14px;
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
.btn:disabled {
  opacity: 0.6;
}
.settings-pane > .grid {
  margin-top: 0;
}
.grid {
  display: grid;
  /* `min(340px, 100%)` 是本轮修 P1-13 的关键：
     原来写死 `minmax(340px, 1fr)`，容器比 340px 窄时（手机竖屏内容区只有 230px）
     列宽仍按 340px 算 → **横向溢出**，卡片右侧的开关要横向滚才够得到。
     加上 min() 之后列宽永远不会超过容器：宽容器下 min(340,100%) == 340px，
     行为与原来完全一致；窄容器下退化成 100%，不再溢出。
     （全仓还有 10 处同形状的 `repeat(auto-fill, minmax(Npx, 1fr))`，一并加了 min()） */
  grid-template-columns: repeat(auto-fill, minmax(min(340px, 100%), 1fr));
  gap: 16px;
  margin-top: 14px;
}
.card {
  padding: 18px;
}
.card h3 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 14px;
  font-size: 14px;
}
.card h3 svg {
  color: var(--accent);
}
.text-input {
  flex: 1;
  width: 100%;
  min-width: 0;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid var(--border);
  border-radius: 9px;
  color: var(--text-1);
  padding: 8px 12px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  transition: border-color 0.12s;
}
.text-input:focus {
  border-color: var(--accent-05);
}
textarea.text-input {
  resize: vertical;
  width: 100%;
}
.mini-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 13px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-2);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  white-space: nowrap;
}
.mini-btn:hover {
  background: rgba(255, 255, 255, 0.08);
}
.mini-btn:disabled {
  opacity: 0.5;
}
.mini-btn.danger {
  border-color: rgba(229, 83, 75, 0.35);
  color: #e5534b;
  background: rgba(229, 83, 75, 0.08);
}
.mini-btn.danger:hover {
  background: rgba(229, 83, 75, 0.16);
}
.btn-icon {
  width: 14px;
  height: 14px;
}
.storage-grid {
  grid-template-columns: 1fr;
}
.storage-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}
.storage-header h3 {
  margin: 0;
}
.storage-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
.storage-footer {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
  flex-wrap: wrap;
}
.storage-footer .hint {
  margin: 0;
  flex: 1;
  min-width: 200px;
}
.storage-body {
  display: flex;
  align-items: center;
  gap: 26px;
}
.donut-wrap {
  position: relative;
  flex: none;
  width: 190px;
  height: 190px;
}
.donut {
  width: 100%;
  height: 100%;
}
.donut-center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.donut-total {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-1);
  line-height: 1.2;
}
.donut-label {
  font-size: 12px;
  color: var(--text-3);
  margin-top: 2px;
}
.storage-legend {
  list-style: none;
  margin: 0;
  padding: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 9px;
  min-width: 0;
}
.storage-legend li {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}
.legend-dot {
  flex: none;
  width: 10px;
  height: 10px;
  border-radius: 50%;
}
.legend-name {
  color: var(--text-2);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.legend-size {
  color: var(--text-1);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.legend-pct {
  color: var(--text-3);
  width: 48px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.instance-storage {
  margin-top: 18px;
  border-top: 1px solid var(--border);
  padding-top: 12px;
}
.instance-storage-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 10px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}
.instance-storage-list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 220px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.instance-storage-list li {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
}
.instance-name {
  color: var(--text-1);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mem-row {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.mem-row label {
  display: block;
  font-size: 13px;
  color: var(--text-2);
  margin-bottom: 8px;
}
.mem-val {
  font-size: 13px;
  color: var(--accent);
  font-weight: 600;
  margin-top: 4px;
}
.mem-mode-row {
  display: flex;
  gap: 16px;
  gap: 6px;
  font-size: 13px;
  color: var(--text-2);
  cursor: pointer;
  user-select: none;
}
.radio-label input {
  accent-color: var(--accent);
  cursor: pointer;
}
.radio-label.active {
  color: var(--accent);
  font-weight: 600;
}
.mem-gauge {
  margin-top: 14px;
}
.mem-gauge-track {
  position: relative;
  height: 10px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}
.mem-gauge-used,
.mem-gauge-alloc {
  position: absolute;
  top: 0;
  bottom: 0;
  height: 100%;
  transition: width 0.2s, left 0.2s;
}
.mem-gauge-used {
  left: 0;
  background: linear-gradient(90deg, #5a8ef0, #8ab4ff);
}
.mem-gauge-alloc {
  background: linear-gradient(90deg, #e89a4b, #f2c079);
}
.mem-gauge-labels {
  display: flex;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 4px;
  font-size: 11px;
  color: var(--text-3);
  margin-top: 6px;
}
.mem-gauge-labels span {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}
.dot.used {
  background: #8ec4ff;
}
.dot.alloc {
  background: #e89a4b;
}
.dot.total {
  background: #9aa4b2;
}
.range {
  width: 100%;
  accent-color: var(--accent);
}
.row-label {
  display: block;
  font-size: 13px;
  color: var(--text-2);
  margin-bottom: 10px;
}
.choice-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  margin-bottom: 14px;
  font-size: 13px;
  color: var(--text-2);
}
.choice-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.choice-label {
  color: var(--text-1);
  font-weight: 500;
}
.choice-hint {
  font-size: 12px;
  color: var(--text-3);
  margin: 0;
  line-height: 1.5;
}
.seg {
  position: relative;
  display: flex;
  /* 不被同行说明文字挤压，否则「横屏/跟随系统」这类两字标签会被竖着折行 */
  flex-shrink: 0;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 9px;
  padding: 3px;
}
.seg .indicator {
  position: absolute;
  top: 3px;
  bottom: 3px;
  border-radius: 7px;
  background: var(--accent-soft);
  pointer-events: none;
}
.seg button {
  border: none;
  background: transparent;
  color: var(--text-3);
  padding: 6px 13px;
  border-radius: 7px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  white-space: nowrap;
}
.seg button.active {
  color: var(--accent);
}
.theme-color-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.color-swatch {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 2px solid transparent;
  background: transparent;
  cursor: pointer;
  padding: 0;
  position: relative;
  transition: transform 0.12s ease, border-color 0.12s ease;
}
.color-swatch:hover {
  transform: scale(1.12);
}
.color-swatch.active {
  border-color: var(--text-1);
  box-shadow: 0 0 0 2px var(--accent-soft);
}
.color-custom {
  position: relative;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  cursor: pointer;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px var(--border);
}
.color-custom-ring {
  position: absolute;
  inset: 0;
  border-radius: 50%;
}
.color-custom input[type="color"] {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  cursor: pointer;
  border: none;
  padding: 0;
}
.about-license-card {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.about-license-card h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text-1);
}
.license-text {
  margin: 0;
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-3);
}
.license-accent {
  color: var(--accent);
  font-weight: 600;
}
/* 许可与版权声明：开源项目清单（与桌面端同一种排版） */
.about-deps-card {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.about-deps-card h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text-1);
}
.deps-groups {
  display: grid;
  grid-template-columns: 1fr;
  gap: 16px;
  margin-top: 4px;
}
.deps-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.deps-group-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-3);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 2px;
}
.about-dep-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 7px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--panel);
  -webkit-backdrop-filter: blur(var(--glass-blur, 8px));
  backdrop-filter: blur(var(--glass-blur, 8px));
  transition: border-color 0.15s;
  flex-wrap: wrap;
}
.about-dep-row:hover {
  border-color: var(--accent);
}
.dep-info {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}
.about-dep-row .dep-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-1);
  display: flex;
  align-items: baseline;
  gap: 5px;
  white-space: nowrap;
}
.dep-ver {
  font-size: 11px;
  color: var(--text-3);
  font-weight: 400;
}
.dep-license {
  font-size: 10px;
  color: var(--accent);
  background: var(--accent-soft);
  padding: 2px 6px;
  border-radius: 5px;
  font-weight: 500;
  white-space: nowrap;
}
.dep-links {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.dep-link {
  font-size: 11px;
  font-family: inherit;
  padding: 3px 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: transparent;
  color: var(--text-3);
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}
.dep-link:hover {
  border-color: var(--accent);
  color: var(--text-1);
}
.about-grid {
  grid-template-columns: 1fr 1fr;
}
.about-card {
  display: flex;
  flex-direction: column;
}
/* 顶部展示卡：画布与卡片保持留白，桌布做斜向动画 + 曲奇咬口 */
.about-showcase {
  padding: 16px;
}
.about-showcase .about-show-wrap {
  height: 200px;
  border-radius: 10px;
  overflow: hidden;
  border: 1px solid var(--border);
}
.about-hero-title {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  margin-top: 14px;
}
.about-hero-name {
  font-size: 20px;
}
.about-hero-slogan {
  text-align: center;
  margin: 6px 0 2px;
  font-size: 13px;
  color: var(--text-2);
  letter-spacing: 0.2px;
}
.about-name {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-1);
}
.about-ver {
  font-size: 13px;
  font-weight: 600;
  color: var(--accent);
  background: var(--accent-soft);
  padding: 2px 8px;
  border-radius: 6px;
  cursor: pointer;
  user-select: none;
  -webkit-user-select: none;
  touch-action: none; /* 安卓 WebView：长按手势别被页面滚动掐断 */
}
/* 长按积蓄中：脉冲提示「别松手」 */
.about-ver.ver-arming {
  animation: ver-pulse 0.6s ease-in-out infinite;
}
@keyframes ver-pulse {
  0%,
  100% {
    transform: scale(1);
    filter: brightness(1);
  }
  50% {
    transform: scale(1.12);
    filter: brightness(1.35);
  }
}

/* ---- 版本彩蛋全屏层 ---- */
.ver-egg {
  position: fixed;
  inset: 0;
  z-index: 2000;
  background: rgba(4, 6, 10, 0.94);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  cursor: pointer;
}
.ver-sparks {
  position: absolute;
  inset: 0;
}
.spark {
  position: absolute;
  image-rendering: pixelated;
  animation-name: spark-burst;
  animation-timing-function: cubic-bezier(0.2, 0.7, 0.3, 1);
  animation-iteration-count: infinite;
  opacity: 0;
}
@keyframes spark-burst {
  0% {
    opacity: 0;
    transform: scale(0.4) rotate(0deg);
  }
  12% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: scale(1.7) rotate(140deg) translateY(-36px);
  }
}
/* 名单：逐行浮入（不再用滚动+遮罩 —— WebView 里遮罩不生效，文字会硬切出现）。
 * 所有行共享同一 8s 周期，靠 animation-delay 依次出现，结尾一起淡出、循环。 */
.ver-credits {
  position: relative;
  width: min(560px, 84vw);
  text-align: center;
}
.ver-credits-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
}
.vc-logo {
  width: 64px;
  height: 64px;
  margin-bottom: 16px;
  filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.55));
  animation: vc-line 8s ease infinite;
}
.vc-title {
  font-size: 30px;
  font-weight: 800;
  color: #fff;
  letter-spacing: 1px;
  margin-bottom: 18px;
}
.vc-line {
  font-size: 15px;
  color: #cfd6e4;
  margin: 10px 0;
}
/* 依次浮入：logo → 标题 → 三行文字，间隔 0.55s */
.vc-logo,
.vc-title,
.vc-line {
  opacity: 0;
  animation-name: vc-line;
  animation-duration: 8s;
  animation-timing-function: ease;
  animation-iteration-count: infinite;
}
.vc-title {
  animation-delay: 0.55s;
}
.vc-title + .vc-line {
  animation-delay: 1.1s;
}
.vc-title + .vc-line + .vc-line {
  animation-delay: 1.65s;
}
.vc-title + .vc-line + .vc-line + .vc-line {
  animation-delay: 2.2s;
}
@keyframes vc-line {
  0% {
    opacity: 0;
    transform: translateY(18px);
  }
  9% {
    opacity: 1;
    transform: translateY(0);
  }
  80% {
    opacity: 1;
    transform: translateY(0);
  }
  90%,
  100% {
    opacity: 0;
    transform: translateY(-12px);
  }
}
.vc-small {
  font-size: 13px;
  color: #8a93a6;
}
.ver-hint {
  position: absolute;
  bottom: 26px;
  left: 0;
  right: 0;
  text-align: center;
  font-size: 12px;
  color: #6d7688;
}
/* 进出场：淡入 + 轻微放大 */
.veregg-enter-active,
.veregg-leave-active {
  transition: opacity 0.3s ease;
}
.veregg-enter-from,
.veregg-leave-to {
  opacity: 0;
}
.dev-role {
  font-size: 12px;
  color: var(--text-3);
  line-height: 1.3;
}
/* 开发者区块：一人一行，不套卡片 */
.about-devs-title {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-3);
  margin: 0 0 10px;
  letter-spacing: 0.3px;
}
.dev-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 18px;
}
.dev-line {
  display: flex;
  align-items: center;
  gap: 12px;
}
.dev-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.dev-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.dev-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  object-fit: cover;
  border: 2px solid var(--accent-35);
  flex-shrink: 0;
}
.dev-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-1);
}
.dev-github-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: transparent;
  color: var(--text-3);
  font-size: 13px;
  cursor: pointer;
  transition: color 0.14s, border-color 0.14s, background 0.14s;
}
.dev-github-btn:hover {
  color: var(--accent);
  border-color: var(--accent-35);
  background: var(--accent-soft);
}
.about-links-row {
  grid-column: 1 / -1;
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
}
.about-links-row .about-link {
  justify-content: flex-start;
  padding: 18px 16px;
  min-height: 56px;
  font-size: 14px;
}
.about-link {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 9px;
  /* 与 .card.glass 一致的磨砂背景，避免和卡片质感不一致 */
  background: var(--panel);
  -webkit-backdrop-filter: blur(var(--glass-blur, 8px));
  backdrop-filter: blur(var(--glass-blur, 8px));
  color: var(--text-2);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
  margin-bottom: 8px;
  transition: all 0.15s;
}
.about-link:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--text-1);
}
.link-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.link-left :deep(svg) {
  font-size: 15px;
  color: var(--accent);
  flex-shrink: 0;
}
.link-arrow {
  color: var(--text-3);
  font-size: 14px;
  transition: transform 0.15s;
}
.about-link:hover .link-arrow {
  transform: translateX(3px);
  color: var(--accent);
}
.appearance-divider {
  height: 1px;
  background: var(--border);
  margin: 4px 0 14px;
}
.bg-preview {
  margin-bottom: 12px;
  border-radius: 10px;
  overflow: hidden;
  border: 1px solid var(--border);
}
.bg-preview img {
  display: block;
  width: 100%;
  height: 92px;
  object-fit: cover;
}
.bg-actions {
  display: flex;
  gap: 8px;
}
.tune-block {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 14px;
}
.tune-row {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 13px;
  color: var(--text-2);
}
.tune-row label {
  flex-shrink: 0;
  width: 96px;
}
.tune-row .range {
  flex: 1;
}
.tune-val {
  flex-shrink: 0;
  width: 46px;
  text-align: right;
  font-size: 12px;
  color: var(--text-3);
  font-variant-numeric: tabular-nums;
}
.mirror-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.mirror-item,
.mirror-custom {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-2);
  cursor: pointer;
  font-family: inherit;
  font-size: 13px;
  text-align: left;
  transition: border-color 0.15s, background 0.15s;
}
.mirror-item:hover,
.mirror-custom:hover {
  border-color: var(--accent-05);
}
.mirror-item.active,
.mirror-custom.active {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.mirror-main {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.mirror-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
}
.mirror-base {
  font-size: 11px;
  color: var(--text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mirror-side {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}
.mirror-ms {
  font-size: 12px;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}
.mirror-ms.bad {
  color: #e5534b;
}
.mirror-btn {
  font-size: 12px;
  color: var(--text-2);
  padding: 4px 10px;
  border-radius: 7px;
  border: 1px solid var(--border);
  flex-shrink: 0;
  transition: color 0.15s, border-color 0.15s;
}
.mirror-btn:hover {
  color: var(--accent);
  border-color: var(--accent);
}
.mirror-btn.disabled {
  opacity: 0.5;
  pointer-events: none;
}
/* 下载代理「测试连接」：跟随主题色 */
.proxy-test-btn {
  color: var(--accent);
  border-color: var(--accent-35);
  background: var(--accent-soft);
  font-weight: 600;
  transition: color 0.15s, border-color 0.15s, background 0.15s;
}
.hint-warn {
  color: #e5534b;
}
.proxy-test-btn:hover:not(.disabled) {
  background: var(--accent);
  border-color: var(--accent);
  color: #1a1208;
}
.proxy-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.proxy-row .seg {
  flex: 1;
  min-width: 0;
}
.proxy-row + .text-input {
  margin-top: 8px;
}
.mirror-custom {
  flex-wrap: wrap;
}
.mirror-custom-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
  cursor: pointer;
  flex-shrink: 0;
}
.mirror-custom-head input {
  accent-color: var(--accent);
  margin: 0;
}
.mirror-custom .text-input {
  flex: 1;
  min-width: 150px;
}

/* 设置页是「左导航 + 右内容」双栏（与实例页的竖向导航栏同一套思路）。
   注意不能改成纵向 flex：那样 268px 高的导航会把内容区挤成 0。 */
.settings-view {
  min-height: 0;
  /* 根样式里是 align-items: flex-start，子项按内容高度撑开，
     结果 .settings-body 永远拿不到可视高度、滚动条不出现。必须拉伸。 */
  align-items: stretch;
}
.settings-body {
  min-height: 0;
  overflow-y: auto;
}

/* ── 设置页左侧导航在矮视口下会被裁 ──────────────────────────────
   7 个导航项（常规/外观/下载/内容服务/游戏内/存储/关于）+ 容器内边距原本约 268px，
   而 OnePlus Ace 5 的内容区只有 240px → 末尾的「存储 / 关于」被裁掉、点不到。
   压紧项高并允许滚动。 */
@media (max-width: 1100px), (pointer: coarse) {
  /* 导航项高度/字号按**视口高度**伸缩：360px 高的机器上 7 项刚好放得下，
     高屏则保持原来的手感；再给 overflow 兜底（项数将来变多也不会被裁）。 */
  .settings-nav {
    padding: clamp(6px, calc(1.8vh / var(--ui-scale, 1)), 16px) clamp(6px, 1.6vw, 14px);
    overflow-y: auto;
  }
  .settings-nav .nav-item {
    padding: clamp(4px, calc(1.7vh / var(--ui-scale, 1)), 9px) 10px;
    font-size: clamp(11.5px, calc(3.4vh / var(--ui-scale, 1)), 13px);
  }
}
/* ── 窄屏（典型是手机竖屏）：左侧导航改成横向 tab 条 ────────────────────
   设置页提供「屏幕方向 → 竖屏」，所以竖屏布局必须可用。
   竖屏 384px 下实测：导航占 112px（`--rail-w` 的下限）、内容只剩 230px，
   双栏在这个宽度下没有意义。这里改成纵向：导航变成一条可横向滚的 tab 条，
   内容独占整宽。

   用 `max-width` 而不是 `orientation: portrait`：平板竖屏（~800px）用双栏完全没问题，
   按方向切会把那里也改坏。 */
@media (max-width: 600px) {
  .settings-view {
    flex-direction: column;
    gap: 10px;
  }
  .settings-nav {
    width: 100%;
    /* 上面 position: sticky 在纵向布局里没有意义，还可能盖住内容 */
    position: static;
    padding: 6px 8px;
    /* 覆盖 (pointer: coarse) 那条里的 overflow-y: auto —— 现在是横向条 */
    overflow-y: visible;
  }
  .settings-nav .nav-list {
    flex-direction: row;
    gap: 4px;
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scrollbar-width: none;
  }
  /* 横向滚动条在手机上只是视觉噪音（内容可滑） */
  .settings-nav .nav-list::-webkit-scrollbar {
    display: none;
  }
  .settings-nav .nav-item {
    flex-shrink: 0;
    white-space: nowrap;
  }
}

/* 界面缩放滑块：固定一个舒服的宽度，别被 flex 挤成一条线 */
.scale-ctl {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  flex-shrink: 0;
  width: clamp(120px, 26vw, 200px);
}
</style>

