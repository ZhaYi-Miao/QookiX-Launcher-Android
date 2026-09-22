<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import {
  IconHome,
  IconCompass,
  IconGrid,
  IconSkin,
  IconSettings,
  IconDownload,
  IconUsers,
  IconNewspaper,
} from "./icons";
import { useTasksStore } from "../stores/tasks";
import { useSettingsStore } from "../stores/settings";
import AccountChip from "./AccountChip.vue";
import { useIsMobile } from "../composables/useMediaQuery";

const route = useRoute();
const isMobile = useIsMobile();
const tasks = useTasksStore();
const settingsStore = useSettingsStore();

/** 摆放位置："bottom"（底部横条，默认）| "left"（左侧竖栏）。由 App.vue 传入。 */
defineProps<{ side?: "bottom" | "left" }>();

// ── 挖孔避让：纯横向让开 ─────────────────────────────────────────────
// 栏总宽 = 图标区 + 用户校准的避让宽度（--nav-offset），图标列整体在挖孔
// 右侧 —— 挖孔无论在左上的什么高度，横向让开后图标都压不到它。
//
// **不要**再把图标列劈成上下两段、中间留竖向空档「绕开」挖孔（早期实现）：
// 空档一大 9 个图标就挤不进剩余高度 → min-height 兜底撑爆容器 → 出现滚动条，
// 图标还会跟着滚动穿过框选区，完全失去避让意义。图标列必须连续、等高分布、
// 永不滚动（见下方 nav-left 的 overflow: hidden）。

// 手机底部导航最多 6 项 + 「下载」1 项，与桌面侧边栏的主要入口保持一致；
// 新闻是可关闭的次要入口，启用时并入（总宽度靠 flex 压缩，不会溢出）。
const nav = computed(() => {
  const list = [
    { name: "home", label: "首页", icon: IconHome, to: "/" },
    { name: "instances", label: "实例", icon: IconGrid, to: "/instances" },
    { name: "browse", label: "内容", icon: IconCompass, to: "/browse" },
    { name: "multiplayer", label: "多人", icon: IconUsers, to: "/multiplayer" },
    { name: "skins", label: "皮肤", icon: IconSkin, to: "/skins" },
    { name: "settings", label: "设置", icon: IconSettings, to: "/settings" },
  ];
  if (settingsStore.settings?.show_news ?? true) {
    list.splice(3, 0, { name: "news", label: "新闻", icon: IconNewspaper, to: "/news" });
  }
  return list;
});

const downloadCount = computed(() => tasks.activeCount);

function isActive(n: { to: string }) {
  if (n.to === "/") return route.path === "/";
  if (n.to === "/instances") {
    return (
      route.path.startsWith("/instances") ||
      route.path.startsWith("/instance/") ||
      route.path === "/create"
    );
  }
  return route.path.startsWith(n.to);
}
</script>

<template>
  <nav class="mobile-nav" :class="{ 'nav-left': side === 'left' }">
    <router-link
      v-for="n in nav"
      :key="n.name"
      :to="n.to"
      class="mobile-nav-item"
      :class="{ active: isActive(n) }"
      :title="n.label"
      :aria-label="n.label"
    >
      <component :is="n.icon" class="mobile-nav-icon" />
      <span class="mobile-nav-label">{{ n.label }}</span>
    </router-link>
    <router-link
      to="/downloads"
      class="mobile-nav-item"
      :class="{ active: route.path.startsWith('/downloads') }"
      title="下载"
      aria-label="下载"
    >
      <IconDownload class="mobile-nav-icon" />
      <span class="mobile-nav-label">下载</span>
      <span
        v-if="downloadCount > 0"
        class="mobile-nav-badge"
      >{{ downloadCount }}</span>
    </router-link>

    <!-- 账号：从标题栏搬到底部导航（横屏手机上底栏是固定的，标题栏还要留给当前页的操作） -->
    <div class="mobile-nav-item mobile-nav-account">
      <AccountChip v-if="isMobile" :collapsed="true" />
    </div>
  </nav>
</template>

<style scoped>
.mobile-nav {
  /* 默认隐藏：仅在窄屏（手机）显示，宽屏继续使用桌面侧边栏 */
  display: none;
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  height: var(--nav-h, 56px);
  align-items: center;
  justify-content: space-around;
  background: color-mix(in srgb, var(--bg-1) 92%, transparent);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-top: 1px solid var(--border);
  /* 横屏时挖孔/刘海在屏幕左右两侧：横条两端的「首页 / 下载」不能钻到摄像头底下 */
  padding: 0 calc(4px + var(--safe-r, 0px)) 0 calc(4px + var(--safe-l, 0px));
  z-index: 100;
  padding-bottom: env(safe-area-inset-bottom, 0);
}

/* 左侧竖栏形态（设置 → 导航栏位置）。
   好处：横屏手机的可用高度只有约 384px，底栏要吃掉约 70px；挪到左侧后
   这段高度全部还给内容区，而横向空间（853px）本来就用不完。
   项目用等高分布（flex:1）而不是写死高度：9 个入口在 384px 高的机器上
   正好铺满、不用滚动；更矮的机器上等分自动压缩。
   顶部从标题栏（40px）以下开始，避免盖住页面标题。 */
.mobile-nav.nav-left {
  /* 标题栏高 40px + 顶部安全区（竖屏有顶部挖孔时标题栏会变高，跟着下移） */
  top: calc(40px + var(--safe-t, 0px));
  bottom: 0;
  /* **顶到屏幕最左缘**：栏体背景一直铺到 x=0（不留空白条）。
     避让宽度 --nav-offset 由用户在校准遮罩里拖线决定（0 = 紧贴边缘）：
     它同时作用于「栏总宽」与「padding-left」，即挖孔区在**栏内部**让开，
     图标整体右移，而背景不断 —— 两全。 */
  left: 0;
  width: calc(var(--nav-w, 64px) + var(--nav-offset, 0px));
  height: auto;
  flex-direction: column;
  align-items: stretch;
  justify-content: flex-start;
  gap: 2px;
  border-top: none;
  border-right: 1px solid var(--border);
  padding: 8px 4px calc(8px + env(safe-area-inset-bottom, 0));
  padding-left: calc(4px + var(--nav-offset, 0px));
  /* **绝不滚动**：图标列一旦滚动，就谈不上「避开挖孔」——内容会滑进挖孔带。
     入口等分压缩已经保证 9 项在最低的机器上也放得下。 */
  overflow: hidden;
}
.mobile-nav.nav-left .mobile-nav-item {
  /* 弹性等分：无论屏幕多矮都平均分，永不溢出、永不出现滚动条 */
  flex: 1 1 0;
  height: auto;
  min-height: 0;
  padding: 2px;
  border-radius: 12px;
}
/* 没有文字说明，激活态必须比底栏更明显：加一层品牌色胶囊 */
.mobile-nav.nav-left .mobile-nav-item.active {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
}
.mobile-nav.nav-left .mobile-nav-label {
  display: none;
}
.mobile-nav.nav-left .mobile-nav-badge {
  top: 3px;
  right: 6px;
}
/* 账号头像：折叠态的容器是 48×48，塞进 ~35px 高的项里会被裁成**非正方形**，
   非常难看。左栏里强制小号正方形，并去掉卡片底色（贴着栏背景即可）。
   必须用 :deep() —— AccountChip 是子组件，它内部的 .avatar 不带本组件的
   scoped 属性，普通选择器根本匹配不上（实测头像仍是 48×48）。 */
.mobile-nav.nav-left .mobile-nav-account {
  padding: 2px;
}
.mobile-nav.nav-left :deep(.acct-chip) {
  height: auto;
  min-height: 0;
  background: transparent;
  border: none;
  padding: 0;
}
.mobile-nav.nav-left :deep(.acct-chip.collapsed .avatar) {
  width: 22px;
  height: 22px;
  aspect-ratio: 1 / 1;
  border-radius: 6px;
}
:global(.has-bg) .mobile-nav {
  background: var(--panel);
}
.mobile-nav-account {
  align-items: center;
  justify-content: center;
}
.mobile-nav-item {
  position: relative;
  display: flex;
  flex: 1 1 0;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  /* 手机上最多 7 个入口，必须可压缩，否则图标会溢出被裁切 */
  min-width: 0;
  overflow: hidden;
  height: 48px;
  padding: 4px 2px;
  border-radius: 10px;
  color: var(--text-3);
  font-size: 10px;
  font-weight: 500;
  text-decoration: none;
  transition: color 0.14s, background 0.14s;
  -webkit-tap-highlight-color: transparent;
}
.mobile-nav-item:active {
  transform: scale(0.92);
}
.mobile-nav-item.active {
  color: var(--accent);
}
.mobile-nav-icon {
  width: 20px;
  height: 20px;
}
.mobile-nav-label {
  line-height: 1;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mobile-nav-badge {
  position: absolute;
  top: 2px;
  right: 4px;
  min-width: 14px;
  height: 14px;
  padding: 0 3px;
  border-radius: 7px;
  background: var(--accent);
  color: #fff;
  font-size: 9px;
  font-weight: 600;
  line-height: 14px;
  text-align: center;
  pointer-events: none;
}

/* 仅在「窄屏 + 竖屏」显示底部导航；横屏与宽屏继续用桌面侧边栏（阈值与 App.vue 保持一致） */
@media (max-width: 1100px), (pointer: coarse) {
  .mobile-nav {
    display: flex;
  }
}
</style>
