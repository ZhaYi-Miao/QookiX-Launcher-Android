<script setup lang="ts">
import { onMounted, onBeforeUnmount, computed, watch, ref, provide } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { darkTheme, lightTheme, NConfigProvider, NDialogProvider, NLoadingBarProvider, NMessageProvider, NNotificationProvider } from "naive-ui";
import { api } from "./api";
import SideBar from "./components/SideBar.vue";
import MobileNav from "./components/MobileNav.vue";
import TitleBar from "./components/TitleBar.vue";
import LoadingBarBridge from "./components/LoadingBarBridge.vue";
import LaunchProgress from "./components/LaunchProgress.vue";
import CrashDialog from "./components/CrashDialog.vue";
import { useSettingsStore } from "./stores/settings";
import { MessageBridge } from "./composables/notify";

import { useAccountsStore } from "./stores/accounts";
import { useInstancesStore } from "./stores/instances";
import { useTasksStore } from "./stores/tasks";
import { usePinsStore } from "./stores/pins";
import {
  buildDarkOverrides,
  buildLightOverrides,
  DEFAULT_ACCENT,
  darken,
  lighten,
  rgba,
  ACCENT_ALPHAS,
} from "./theme";

const settings = useSettingsStore();
const accounts = useAccountsStore();
const instances = useInstancesStore();
const tasks = useTasksStore();
const pins = usePinsStore();

const isDark = computed(() => settings.settings?.theme !== "light");
const activeTheme = computed(() => (isDark.value ? darkTheme : lightTheme));

const accentColor = computed(() => settings.settings?.theme_color || DEFAULT_ACCENT);
const themeOverrides = computed(() =>
  isDark.value ? buildDarkOverrides(accentColor.value) : buildLightOverrides(accentColor.value),
);

// 首次应用主题不做过渡（否则页面加载时会闪一下颜色）
let themeReady = false;
let themeTransitionTimer: ReturnType<typeof setTimeout> | null = null;

watch(isDark, () => {
  const root = document.documentElement;
  if (themeReady) {
    // 切换瞬间临时开启全局颜色过渡，结束后移除，避免影响性能与 hover 动画
    root.classList.add("theme-transition");
    if (themeTransitionTimer) clearTimeout(themeTransitionTimer);
    themeTransitionTimer = setTimeout(() => {
      root.classList.remove("theme-transition");
      themeTransitionTimer = null;
    }, 320);
  }
  root.classList.toggle("light", !isDark.value);
  themeReady = true;
}, { immediate: true });

onBeforeUnmount(() => {
  if (themeTransitionTimer) clearTimeout(themeTransitionTimer);
});

// 将主题色应用到 CSS 变量上（accent / accent-deep / accent-soft / 各级 alpha）
watch(accentColor, (hex) => {
  const root = document.documentElement;
  root.style.setProperty("--accent", hex);
  root.style.setProperty("--accent-deep", darken(hex));
  root.style.setProperty("--accent-hover", lighten(hex));
  root.style.setProperty("--accent-soft", rgba(hex, 0.14));
  for (const a of ACCENT_ALPHAS) {
    root.style.setProperty(`--accent-${String(Math.round(a * 100)).padStart(2, "0")}`, rgba(hex, a));
  }
}, { immediate: true });

const bgStyle = computed(() => {
  const s = settings.settings;
  if (!s?.background_image) return {} as Record<string, string>;
  return {
    "--bg-image": `url("${convertFileSrc(s.background_image)}")`,
    "--bg-blur": `${s.background_blur}px`,
    "--bg-dim": String(s.background_dim / 100),
    "--bg-dim-light": String((s.background_dim / 100) * 0.45),
  } as Record<string, string>;
});

/** 手机导航栏的摆放位置："bottom"（底部横条）| "left"（左侧竖栏）。
 *  决定内容区与浮层给哪一侧留出导航空间（见下方移动端样式里的 --nav-inset-*）。 */
const navSide = computed<"bottom" | "left">(() => settings.settings?.nav_position ?? "bottom");

// ── 挖孔 / 刘海避让推导 ──────────────────────────────────────────────
// 用户按**竖屏**方向描述挖孔类型（中置 / 左上 / 右上 / 刘海），这里负责
// 换算成当前横竖屏下各边的实际避让量。设计要点：
//   · 打孔的尺寸彼此都差不多，统一让开 44px（约一个图标的位置）即可，
//     不让用户抠像素；只有刘海长短不一，用 nav_offset 作为自定义宽度。
//   · "auto" 什么都不推导 —— 完全跟随系统 env() 安全区（大多数机型够用）。
//   · 图标列是连续排布的（见 MobileNav），横向让开后天然压不到左缘挖孔，
//     所以纵向劈列那套已经废弃；竖屏时挖孔在顶缘，避让体现为 --safe-t，
//     侧栏从标题栏以下开始（标题栏被 safe-t 压低），天然在挖孔下方。
// 与 --ui-scale 一样写到根元素上，MobileNav / 内容区 / 浮层都能取到。
const CUTOUT_DEFAULT = 44;

/** 当前是否竖屏（挖孔类型按竖屏描述，横屏时顶缘旋到左右侧）。 */
const isPortrait = ref(window.matchMedia("(orientation: portrait)").matches);
window
  .matchMedia("(orientation: portrait)")
  .addEventListener("change", (e) => (isPortrait.value = e.matches));

watch(
  [
    () => settings.settings?.nav_cutout,
    () => settings.settings?.nav_offset,
    isPortrait,
  ],
  ([mode, rawW, portrait]) => {
    const m = mode ?? "auto";
    // 打孔用固定默认值；只有刘海用用户给的宽度
    const w = m === "notch" ? Math.max(0, rawW ?? 0) : CUTOUT_DEFAULT;
    let left = 0;
    let right = 0;
    let top = 0;
    if (m === "center" || m === "topleft" || m === "notch") {
      // 竖屏：挖孔在顶缘（左上角还额外占左缘）；横屏：顶缘旋到左缘（左栏这一侧）
      if (portrait) {
        top = w;
        if (m === "topleft") left = w;
      } else {
        left = w;
      }
    } else if (m === "topright") {
      // 竖屏右上角：顶缘 + 右缘；横屏：右缘
      if (portrait) {
        top = w;
        right = w;
      } else {
        right = w;
      }
    }
    const root = document.documentElement.style;
    // --nav-offset 驱动左栏总宽与图标列位置（横屏左缘避让）；
    // 竖屏时挖孔不在左缘，栏不需要横向让开（顶部由 --safe-t 处理）。
    root.setProperty("--nav-offset", `${portrait ? 0 : left}px`);
    root.setProperty("--cutout-right", `${right}px`);
    root.setProperty("--cutout-top", `${top}px`);
  },
  { immediate: true }
);

// 界面缩放：写 --ui-scale 到根元素，并整体缩放 #app。
// 用 zoom 而不是逐个换算尺寸 —— WebView 是 Chromium，zoom 会连带字号/间距/控件一起缩放，
// 比手工维护每个尺寸可靠；100% 时清掉，避免多一层合成。
// 目的：不同机型 CSS 视口差很多（实测 853×384 与 792×360），与其为每台机写死尺寸，不如让用户自己调密度。
watch(
  () => settings.settings?.ui_scale,
  (v) => {
    const scale = Math.min(200, Math.max(60, v ?? 100)) / 100;
    document.documentElement.style.setProperty("--ui-scale", String(scale));
    const app = document.getElementById("app");
    if (app) app.style.zoom = scale === 1 ? "" : String(scale);
  },
  { immediate: true },
);

watch(() => settings.settings?.glass_blur, (v) => {
  if (v != null) document.documentElement.style.setProperty("--glass-blur", `${v}px`);
}, { immediate: true });

// 触控目标档位：写到 <html data-touch>，由 styles.css 里的属性选择器决定
// 要不要给按钮/输入框加最小尺寸（compact = 不加，保持原始紧凑观感）。
watch(
  () => settings.settings?.touch_target,
  (v) => {
    const el = document.documentElement;
    const mode = v === "standard" || v === "large" ? v : "compact";
    if (mode === "compact") delete el.dataset.touch;
    else el.dataset.touch = mode;
  },
  { immediate: true }
);

watch(() => settings.settings?.background_image, (p) => {
  document.documentElement.classList.toggle("has-bg", !!p);
}, { immediate: true });

// 屏幕方向：设置变化时立刻下发到原生层（安卓端锁定横/竖屏；桌面端为空实现）
watch(() => settings.settings?.orientation, (mode) => {
  if (mode) void api.setOrientation(mode);
}, { immediate: true });

// 供 TitleBar 的"新建分组"按钮触发实例页的分组对话框
const groupDialogRequest = ref(0);
provide("groupDialogRequest", groupDialogRequest);

// —— 启动加载流程 ——
// **没有启动页**：点图标就直接进界面（用户明确要求去掉那一屏）。
// 这里只做数据预加载，各 store 的 load() 是「已有数据则后台静默刷新」模式，
// 视图在数据到达前先渲染空态即可。
async function boot() {
  try {
    await settings.load();
  } catch {
    /* 设置加载失败不阻塞启动 */
  }
  await Promise.all([
    instances.load().catch(() => {}),
    accounts.load().catch(() => {}),
  ]);
  tasks.init();
  await pins.init().catch(() => {});
}

// 全局监听：分享包导入后游戏本体自动安装失败（后端无法直接弹 toast）
let unlistenShareErr: (() => void) | null = null;
onMounted(async () => {
  void boot();
  // 对照探针：验证「前端 → log_debug → 落盘」这条日志通道本身可用
  api.logDebug("[fe] App 启动，日志通道自检");
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const { notifyError } = await import("./composables/notify");
    unlistenShareErr = await listen<{ instanceId: string; error: string }>(
      "share-import://game-install-failed",
      (ev) => {
        notifyError(`游戏本体自动安装失败：${ev.payload.error}`);
      }
    );
  } catch {
    /* 监听不可用不影响主流程 */
  }
});
onBeforeUnmount(() => {
  unlistenShareErr?.();
  unlistenShareErr = null;
});
</script>

<template>
  <n-config-provider :theme="activeTheme" :theme-overrides="themeOverrides" :inline-theme-disabled="true">
    <n-loading-bar-provider>
      <LoadingBarBridge>
        <n-dialog-provider>
          <n-message-provider>
            <MessageBridge />
            <n-notification-provider>
              <div
                class="app app-bg"
                :class="{ light: !isDark, 'nav-left': navSide === 'left' }"
                :style="bgStyle"
              >
                <!-- 顶部标题栏：承载「新建实例 / 新建分组 / 创建服务器 / 清除已完成」等页面动作 -->
                <TitleBar />
                <div class="body">
                  <SideBar />
                  <main class="content">
                    <router-view v-slot="{ Component, route }">
                      <Transition name="page-rise" mode="out-in">
                        <component :is="Component" :key="route.path" />
                      </Transition>
                    </router-view>
                  </main>
                </div>
                <MobileNav :side="navSide" />
                <LaunchProgress />
                <CrashDialog />
              </div>
            </n-notification-provider>
          </n-message-provider>
        </n-dialog-provider>
      </LoadingBarBridge>
    </n-loading-bar-provider>
  </n-config-provider>
</template>

<style scoped>
.app {
  /* 挖孔/刘海安全区。启动器默认横屏：中置挖孔、刘海、左上打孔在横屏下
     都会变成**屏幕左右两侧**的遮挡（本机实测 left=44px），标题栏 / 内容区 /
     导航栏三处都要避开。竖屏时则体现在 --safe-t 上。
     桌面端这些 env() 全是 0，无副作用。

     三条都取「系统安全区」与「用户选择的挖孔类型推导值」的较大者：
     env() 有的机型报得不准或干脆报 0；用户选了明确类型就以他为准 —— 谁大听谁。
     推导逻辑见上方 script 里的「挖孔 / 刘海避让推导」。 */
  --safe-t: max(env(safe-area-inset-top, 0px), var(--cutout-top, 0px));
  --safe-l: max(env(safe-area-inset-left, 0px), var(--nav-offset, 0px));
  --safe-r: max(env(safe-area-inset-right, 0px), var(--cutout-right, 0px));
  /* 根布局高度：**必须按界面缩放折算**。
     用裸 `100vh` 的话，缩到 80% 时只画出 288px 高、下面留空（露白底）；
     而用 `100%` 也不行 —— 这一层外面还套着 naive-ui 的 provider div（没有高度），
     百分比拿不到确定基准，会退化成「内容撑开」（实测 100% 时量到 994px、130% 时 3147px）。
     所以显式用视口高度再除以缩放倍率：100vh / scale 再乘 scale = 正好一屏。 */
  height: calc(100vh / var(--ui-scale, 1));
  display: flex;
  flex-direction: column;
}
.app.light {
  background: radial-gradient(900px 480px at 85% -10%, var(--accent-12), transparent 60%),
    linear-gradient(180deg, #f7f6f4, #eceef2);
}
.body {
  flex: 1;
  display: flex;
  min-height: 0;
}
.content {
  flex: 1;
  min-height: 0;
  /* 页面本身不滚动：横屏手机纵向只有约 393px，整页滚动会让标题栏/底部导航
     之外的可用高度变得不可预测。改成「每个视图自己滚」——
     放得下的页面完全不滑，放不下的也只是页内滚动，观感与原来一致，
     而且不会出现内容被裁掉够不着的情况。 */
  overflow: hidden;
  padding: 22px 26px 30px;
}
/* 每个页面自己承担滚动（列表页内部通常还有自己的滚动区） */
.content > * {
  height: 100%;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
}

/* —— 移动端适配 ——
   仅在「窄屏 + 竖屏」时隐藏桌面侧边栏，改用手机导航栏。
   导航栏可以摆在底部（默认）或左侧（设置里可切换），两种摆放只差「往哪边让空间」，
   这里统一用一对变量表达，内容区与浮层（LaunchProgress）都引用它们：
     --nav-inset-bottom / --nav-inset-left
   数值上与旧实现完全一致：底栏时 padding-bottom = 14px 间距 + 56px 栏高 + 底部安全区
   （原来写死的 70px 就是 56+14）。 */
@media (max-width: 1100px), (pointer: coarse) {
  .body > :deep(.sidebar) {
    display: none;
  }
  .app {
    --nav-inset-bottom: calc(var(--nav-h, 56px) + env(safe-area-inset-bottom, 0px));
    /* 底栏模式：内容与标题栏**贴左**。
       挖孔在左缘时给整列内容留一条空边非常难看（用户实测反馈），
       而且底栏本身高度上离挖孔带很远，真正贴着挖孔的只有底栏两端的图标，
       那里由底栏自己的 safe-l/r 内边距负责。标题栏同此（--tb-inset-l）。 */
    --nav-inset-left: 0px;
    --tb-inset-l: 0px;
  }
  .app.nav-left {
    --nav-inset-bottom: 0px;
    /* 栏总宽 = 图标区 + 挖孔避让宽度；标题栏在栏上方，仍要避开左缘挖孔 */
    --nav-inset-left: calc(var(--nav-w, 64px) + var(--nav-offset, 0px));
    --tb-inset-l: var(--safe-l);
  }
  .content {
    padding-top: 10px;
    /* 右侧：中置挖孔/刘海在横屏可能落在右边（部分机型 / 180° 旋转） */
    padding-right: calc(12px + var(--safe-r, 0px));
    padding-bottom: calc(14px + var(--nav-inset-bottom, 0px));
    padding-left: calc(12px + var(--nav-inset-left, 0px));
  }
}
</style>
