<script setup lang="ts">
import { t as $t } from "./i18n";
import { computed, onBeforeUnmount, onMounted, provide, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { convertFileSrc } from "@tauri-apps/api/core";
import { darkTheme, lightTheme, NConfigProvider, NDialogProvider, NMessageProvider } from "naive-ui";
import { api } from "./api";
import { useSettingsStore } from "./stores/settings";
import { useAccountsStore } from "./stores/accounts";
import { useInstancesStore } from "./stores/instances";
import { useTasksStore } from "./stores/tasks";
import { usePinsStore } from "./stores/pins";
import { MessageBridge } from "./composables/notify";
import { buildDarkOverrides, buildLightOverrides, DEFAULT_ACCENT, darken, lighten, rgba, ACCENT_ALPHAS } from "./theme";
import { Tabbar as VanTabbar, TabbarItem as VanTabbarItem } from "vant";
import AccountChip from "./components/AccountChip.vue";
import LaunchProgress from "./components/LaunchProgress.vue";
import CrashDialog from "./components/CrashDialog.vue";
import FirstRunSetup from "./components/FirstRunSetup.vue";
import { IconHome, IconGrid, IconCompass, IconDownload, IconUser, IconSettings, IconNewspaper, IconUsers } from "./components/icons";

const route = useRoute();
const settings = useSettingsStore();
const accounts = useAccountsStore();
const instances = useInstancesStore();
const tasks = useTasksStore();
const pins = usePinsStore();

const isDark = computed(() => settings.settings?.theme !== "light");
const accent = computed(() => settings.settings?.theme_color || DEFAULT_ACCENT);
const themeOverrides = computed(() => (isDark.value ? buildDarkOverrides(accent.value) : buildLightOverrides(accent.value)));

watch(isDark, () => document.documentElement.classList.toggle("light", !isDark.value), { immediate: true });

watch(accent, (hex) => {
  const root = document.documentElement.style;
  root.setProperty("--accent", hex);
  root.setProperty("--accent-deep", darken(hex));
  root.setProperty("--accent-hover", lighten(hex));
  root.setProperty("--accent-soft", rgba(hex, 0.14));
  for (const a of ACCENT_ALPHAS) {
    root.setProperty(`--accent-${String(Math.round(a * 100)).padStart(2, "0")}`, rgba(hex, a));
  }
}, { immediate: true });

// 界面缩放：整块 zoom（既有约定：视口单位要除 scale，百分比不用）
watch(() => settings.settings?.ui_scale, (v) => {
  const scale = Math.min(200, Math.max(60, v ?? 100)) / 100;
  document.documentElement.style.setProperty("--ui-scale", String(scale));
  const app = document.getElementById("app");
  if (app) app.style.zoom = scale === 1 ? "" : String(scale);
}, { immediate: true });

watch(() => settings.settings?.background_image, (v) => {
  document.documentElement.classList.toggle("has-bg", !!v);
  if (v) document.documentElement.style.setProperty("--bg-image", `url("${convertFileSrc(v)}")`);
}, { immediate: true });

watch(() => settings.settings?.orientation, (m) => { if (m) void api.setOrientation(m); }, { immediate: true });

const tabs = computed(() => {
  const list = [
    { name: "home", label: $t("nav.home"), icon: IconHome, to: "/" },
    { name: "instances", label: $t("nav.instances"), icon: IconGrid, to: "/instances" },
    { name: "browse", label: $t("nav.browse"), icon: IconCompass, to: "/browse" },
    { name: "multiplayer", label: $t("nav.multiplayer"), icon: IconUsers, to: "/multiplayer" },
    { name: "skins", label: $t("nav.skins"), icon: IconUser, to: "/skins" },
    { name: "settings", label: $t("nav.settings"), icon: IconSettings, to: "/settings" },
  ];
  if (settings.settings?.show_news ?? true) {
    list.splice(3, 0, { name: "news", label: $t("nav.news"), icon: IconNewspaper, to: "/news" });
  }
  return list;
});

/** 当前页对应的入口名（Vant Tabbar 的 v-model） */
const activeTab = computed(() => {
  const p = route.path;
  if (p === "/") return "home";
  if (p.startsWith("/instance") || p === "/create") return "instances";
  return tabs.value.find((t) => t.to !== "/" && p.startsWith(t.to))?.name ?? "";
});

const downloadCount = computed(() => tasks.activeCount);
const pageTitle = computed(() => (route.meta.title as string) || "");

/** 页面右上动作：由页面通过 inject("pageAction") 声明 */
const pageAction = ref<{ text: string; run: () => void } | null>(null);
provide("pageAction", pageAction);

onMounted(async () => {
  try { await settings.load(); } catch { /* 不阻塞启动 */ }
  await Promise.all([instances.load().catch(() => {}), accounts.load().catch(() => {})]);
  tasks.init();
  await pins.init().catch(() => {});
});

let unlisten: (() => void) | null = null;
onMounted(async () => {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const { notifyError } = await import("./composables/notify");
    unlisten = await listen<{ instanceId: string; error: string }>("share-import://game-install-failed", (ev) => {
      notifyError($t("app.notify-error", { p1: ev.payload.error }));
    });
  } catch { /* 忽略 */ }
});
onBeforeUnmount(() => { unlisten?.(); unlisten = null; });
</script>

<template>
  <n-config-provider :theme="isDark ? darkTheme : lightTheme" :theme-overrides="themeOverrides" :inline-theme-disabled="true">
    <n-dialog-provider>
      <n-message-provider>
        <MessageBridge />
        <div class="shell app-bg">
          <header class="top">
            <h1 class="title">{{ pageTitle }}</h1>
            <AccountChip />
            <button v-if="pageAction" class="top-act" @click="pageAction.run()">{{ pageAction.text }}</button>
          </header>

          <main class="page">
            <router-view v-slot="{ Component }">
              <Transition name="page" mode="out-in">
                <component :is="Component" />
              </Transition>
            </router-view>
          </main>

          <van-tabbar v-model="activeTab" :fixed="false" :safe-area-inset-bottom="true" class="tabs">
            <van-tabbar-item v-for="t in tabs" :key="t.name" :name="t.name" :to="t.to">
              <template #icon><component :is="t.icon" class="tab-icon" /></template>
              {{ t.label }}
            </van-tabbar-item>
            <van-tabbar-item name="downloads" to="/downloads" :badge="downloadCount > 0 ? String(downloadCount) : ''">
              <template #icon><IconDownload class="tab-icon" /></template>
              {{ $t("nav.downloads") }}
            </van-tabbar-item>
          </van-tabbar>

          <LaunchProgress />
          <CrashDialog />
          <FirstRunSetup />
          <div id="van-layer"></div>
        </div>
      </n-message-provider>
    </n-dialog-provider>
  </n-config-provider>
</template>

<style scoped>
.shell {
  height: calc(100vh / var(--ui-scale, 1));
  display: flex;
  flex-direction: column;
  color: var(--text-1);
}
.top {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: calc(10px + env(safe-area-inset-top, 0px)) 16px 8px;
}
.title {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.top-act {
  flex-shrink: 0;
  min-height: 36px;
  padding: 8px 14px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--accent);
  font-family: inherit;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
/* 页面区：唯一收口处，页面自己滚（容器查询的参照物） */
.page {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  container-type: inline-size;
  container-name: page;
}
.page > * {
  height: 100%;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
}
.tab-icon {
  width: 20px;
  height: 20px;
}
.tabs {
  flex-shrink: 0;
}
.page-enter-active { transition: opacity .22s ease, transform .22s ease; }
.page-leave-active { transition: opacity .14s ease; }
.page-enter-from { opacity: 0; transform: translateY(8px); }
.page-leave-to { opacity: 0; }
</style>
