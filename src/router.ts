import { t as $t } from "./i18n";
import { createRouter, createWebHistory } from "vue-router";
import { useSettingsStore } from "./stores/settings";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "home", component: () => import("./views/HomeView.vue"), meta: { title: $t("router.home"), icon: "home" } },
    { path: "/news", name: "news", component: () => import("./views/NewsView.vue"), meta: { title: $t("router.newspaper"), icon: "newspaper" } },
    { path: "/browse", name: "browse", component: () => import("./views/BrowseView.vue"), meta: { title: $t("router.compass"), icon: "compass" } },
    { path: "/downloads", name: "downloads", component: () => import("./views/DownloadsView.vue"), meta: { title: $t("router.download"), icon: "download" } },
    { path: "/instances", name: "instances", component: () => import("./views/InstancesView.vue"), meta: { title: $t("router.grid"), icon: "grid", action: { text: $t("router.plus"), icon: "plus", to: "/create" } } },
    { path: "/instance/:id", name: "instance", component: () => import("./views/InstanceDetailView.vue"), meta: { title: $t("router.shi-li-xiang-qing"), icon: "grid" } },
    { path: "/create", name: "create", component: () => import("./views/CreateInstanceView.vue"), meta: { title: $t("router.chuang-jian-shi-li"), icon: "plus" } },
    { path: "/multiplayer", name: "multiplayer", component: () => import("./views/MultiplayerView.vue"), meta: { title: $t("instance-saves.users"), icon: "users" } },
    { path: "/multiplayer/:id", name: "server-detail", component: () => import("./views/ServerDetailView.vue"), meta: { title: $t("router.users"), icon: "users" } },
    { path: "/settings", name: "settings", component: () => import("./views/SettingsView.vue"), meta: { title: $t("router.settings"), icon: "settings" } },
    { path: "/skins", name: "skins", component: () => import("./views/SkinView.vue"), meta: { title: $t("router.user"), icon: "user" } },
  ],
});

// 关闭「新闻」后，直接访问 /news 会跳回首页（侧边栏入口本身也已隐藏）。
// 设置尚未加载时按「显示」处理，避免启动瞬间误跳转。
router.beforeEach((to) => {
  if (to.path !== "/news") return true;
  try {
    const s = useSettingsStore();
    if (s.settings && s.settings.show_news === false) return { path: "/", replace: true };
  } catch {
    // store 尚未初始化（Pinia 未激活）时放行
  }
  return true;
});

export default router;
