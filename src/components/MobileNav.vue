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
import { t as $t } from "../i18n";
import {
  Tabbar as VanTabbar,
  TabbarItem as VanTabbarItem,
  Sidebar as VanSidebar,
  SidebarItem as VanSidebarItem,
} from "../ui/vant";

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
    { name: "home", label: $t("nav.home"), icon: IconHome, to: "/" },
    { name: "instances", label: $t("nav.instances"), icon: IconGrid, to: "/instances" },
    { name: "browse", label: $t("nav.browse"), icon: IconCompass, to: "/browse" },
    { name: "multiplayer", label: $t("nav.multiplayer"), icon: IconUsers, to: "/multiplayer" },
    { name: "skins", label: $t("nav.skins"), icon: IconSkin, to: "/skins" },
    { name: "settings", label: $t("nav.settings"), icon: IconSettings, to: "/settings" },
  ];
  if (settingsStore.settings?.show_news ?? true) {
    list.splice(3, 0, { name: "news", label: $t("nav.news"), icon: IconNewspaper, to: "/news" });
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

/** 当前页对应的入口名（Vant 的 v-model 认这个）。都不匹配时留空 = 无高亮。 */
const activeName = computed(() => nav.value.find((n) => isActive(n))?.name ?? "");
</script>

<template>
  <nav class="mobile-nav" :class="{ 'nav-left': side === 'left' }">
    <!-- 底部横条：Vant Tabbar。
         定位与挖孔避让**仍由外层 .mobile-nav 负责**（fixed + 安全区 + 左右让开），
         所以这里 fixed / safe-area 都关掉，Tabbar 只负责铺满并画图标+文字。 -->
    <van-tabbar
      v-if="side !== 'left'"
      v-model="activeName"
      :fixed="false"
      :safe-area-inset-bottom="false"
      :border="false"
      class="mn-bar"
    >
      <van-tabbar-item v-for="n in nav" :key="n.name" :name="n.name" :to="n.to">
        <template #icon>
          <component :is="n.icon" class="mn-icon" />
        </template>
        {{ n.label }}
      </van-tabbar-item>
      <van-tabbar-item
        name="downloads"
        to="/downloads"
        :badge="downloadCount > 0 ? String(downloadCount) : ''"
      >
        <template #icon>
          <IconDownload class="mn-icon" />
        </template>
        {{ $t("nav.downloads") }}
      </van-tabbar-item>
    </van-tabbar>

    <!-- 左侧竖栏：Vant Sidebar（等分布满高度，永不滚动，见 .nav-left 的 overflow） -->
    <van-sidebar v-else v-model="activeName" class="mn-rail">
      <van-sidebar-item v-for="n in nav" :key="n.name" :name="n.name" :to="n.to">
        <template #title>
          <component :is="n.icon" class="mn-icon" />
        </template>
      </van-sidebar-item>
      <van-sidebar-item
        name="downloads"
        to="/downloads"
        :badge="downloadCount > 0 ? String(downloadCount) : ''"
      >
        <template #title>
          <IconDownload class="mn-icon" />
        </template>
      </van-sidebar-item>
    </van-sidebar>

    <!-- 账号：从标题栏搬到导航栏（横屏手机上底栏是固定的，标题栏还要留给当前页的操作） -->
    <div v-if="isMobile" class="mobile-nav-account">
      <AccountChip :collapsed="true" />
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
   栏体**顶到屏幕最左缘**，避让宽度 --nav-offset 由用户在校准遮罩里拖线决定
   （0 = 紧贴边缘）：它同时作用于「栏总宽」与「padding-left」，即挖孔区在
   **栏内部**让开、图标整体右移，而背景不断。
   顶部从标题栏（40px）以下开始，避免盖住页面标题。 */
.mobile-nav.nav-left {
  top: calc(40px + var(--safe-t, 0px));
  bottom: 0;
  left: 0;
  width: calc(var(--nav-w, 64px) + var(--nav-offset, 0px));
  height: auto;
  /* 图标列之外还要给账号头像留位置，所以整体是纵向 flex */
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

/* Tabbar / Sidebar 铺满外层容器 */
.mn-bar,
.mn-rail {
  width: 100%;
  background: transparent;
}
.mn-rail {
  /* 等分铺满剩余高度（左侧栏那一列），与账号头像共享纵向空间 */
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.mn-icon {
  width: 20px;
  height: 20px;
}

/* 侧栏项：等分高度 + 居中（Vant 默认是内容高度、左对齐，不适合竖排图标栏） */
.mn-rail :deep(.van-sidebar-item) {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  border-radius: 12px;
}
.mn-rail :deep(.van-sidebar-item__text) {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
}

/* 账号头像：折叠态容器是 48×48，塞进 ~35px 高的项里会被裁成**非正方形**，
   非常难看。左栏里强制小号正方形，并去掉卡片底色（贴着栏背景即可）。
   必须用 :deep() —— AccountChip 是子组件，它内部的 .avatar 不带本组件的
   scoped 属性，普通选择器根本匹配不上（实测头像仍是 48×48）。 */
.mobile-nav-account {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  min-width: 0;
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

/* 仅在「窄屏 + 竖屏」显示导航栏；横屏与宽屏继续用桌面侧边栏（阈值与 App.vue 保持一致） */
@media (max-width: 1100px), (pointer: coarse) {
  .mobile-nav {
    display: flex;
  }
}
</style>
