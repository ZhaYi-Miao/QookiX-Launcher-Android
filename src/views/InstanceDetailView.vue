<script setup lang="ts">
/**
 * 实例详情页（壳）。
 * 只保留：页头（启动/固定/删除/打开目录）、tab 栏、截图 tab、确认与预览弹窗。
 * 各 tab 的领域逻辑已拆分到 src/components/instance/ 下的子组件：
 *   - ContentTab.vue   内容管理（mods / resourcepacks / shaders）
 *   - SavesTab.vue     世界（单人存档 + 多人服务器）
 *   - SettingsTab.vue  实例设置（Java / 内存 / 别名 / 参数 / 图标）
 *   - FileManager.vue / LogViewer.vue / CrashAnalyzer.vue（早已独立）
 */
import { t as $t } from "../i18n";
import { computed, h, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useInstancesStore } from "../stores/instances";
import { useAccountsStore } from "../stores/accounts";
import { usePinsStore, type PinTarget } from "../stores/pins";
import { useMessage, NButton, NMenu } from "naive-ui";
import { api } from "../api";
import { convertFileSrc } from "@tauri-apps/api/core";
import LogViewer from "../components/LogViewer.vue";
import FileManager from "../components/FileManager.vue";
import CrashAnalyzer from "../components/CrashAnalyzer.vue";
import ContentTab from "../components/instance/ContentTab.vue";
import SavesTab from "../components/instance/SavesTab.vue";
import SettingsTab from "../components/instance/SettingsTab.vue";
import KeysTab from "../components/instance/KeysTab.vue";
import { fmtDateLocale as fmtDate, fmtSize } from "../utils/format";
import AppSheet from "../ui/AppSheet.vue";
import {
  IconBox,
  IconCamera,
  IconExternal,
  IconFile,
  IconFolder,
  IconHardDrive,
  IconBug,
  IconImage,
  IconLayers,
  IconLayout,
  IconMapPin,
  IconPlay,
  IconSliders,
  IconTrash,
} from "../components/icons";

const route = useRoute();
const router = useRouter();
const instances = useInstancesStore();
const accounts = useAccountsStore();
const message = useMessage();
const pins = usePinsStore();

// ---- 确认弹窗（删除实例用）----
const confirmState = ref<{ title: string; content: string; positiveText: string; onOk: () => void | Promise<void> } | null>(null);
const confirmLoading = ref(false);
async function handleConfirm() {
  if (!confirmState.value) return;
  confirmLoading.value = true;
  try {
    await confirmState.value.onOk();
    confirmState.value = null;
  } finally {
    confirmLoading.value = false;
  }
}

const instanceId = route.params.id as string;
const tab = ref<string>(
  (Array.isArray(route.query.tab) ? route.query.tab[0] : route.query.tab) ?? "files"
);

const instance = computed(() => instances.get(instanceId));

// ---- 文件夹存在性（决定显示哪些 tab）----
const folders = ref<Record<string, boolean>>({});
async function loadFolders() {
  try {
    const r = await api.listInstanceFolders(instanceId);
    folders.value = Object.fromEntries(r.folders.map((f) => [f.name, f.exists]));
  } catch {
    /* ignore */
  }
}

const CONTENT_TABS = ["mods", "shaders", "resourcepacks"];
const KIND_BY_TAB: Record<string, string> = {
  mods: "mod",
  shaders: "shader",
  resourcepacks: "resourcepack",
};
function kindOf(t: string) {
  return KIND_BY_TAB[t] ?? t;
}

const ALL_TABS = [
  { key: "mods", label: $t("browse.mods"), icon: IconBox, folder: "mods" },
  { key: "shaders", label: $t("utils.categories.shader"), icon: IconLayers, folder: "shaderpacks" },
  { key: "resourcepacks", label: $t("instance-detail.resourcepacks"), icon: IconImage, folder: "resourcepacks" },
  { key: "screenshots", label: $t("instance-detail.screenshots"), icon: IconCamera, folder: "screenshots" },
  { key: "saves", label: $t("instance-detail.group"), icon: IconFolder, folder: "saves" },
  { key: "files", label: $t("instance-detail.files"), icon: IconHardDrive },
  { key: "logs", label: $t("common.logs"), icon: IconFile },
  { key: "crash", label: $t("crash-dialog.crash"), icon: IconBug },
  // 手机端的「按键」是屏幕上的触控控制层（控制布局全局共享，不按实例区分），
  // 所以不给它 folder —— 给了会被「文件夹存在才显示」的规则挡掉。
  { key: "keys", label: $t("instance-detail.keys"), icon: IconLayout },
  { key: "settings", label: $t("router.settings"), icon: IconSliders },
];
// 左侧竖向导航栏的选项。图标是组件，naive-ui 的 options 需要渲染函数。
const railOptions = computed(() =>
  tabs.value.map((t) => ({
    key: t.key,
    label: t.label,
    icon: () => h(t.icon),
  }))
);
// folder-backed tabs are only shown when the corresponding folder exists;
// vanilla instances have no mods and no shaders, so hide those tabs entirely.
const tabs = computed(() =>
  ALL_TABS.filter((t) => {
    if (
      (t.key === "mods" || t.key === "shaders") &&
      instance.value?.loader === "vanilla"
    )
      return false;
    return !t.folder || folders.value[t.folder] || t.key === tab.value;
  })
);

// 支持通过 URL query 切换 tab（崩溃弹窗「查看日志」跳转到日志页、「崩溃分析」跳转到崩溃分析页）
watch(
  () => route.query.tab,
  (v) => {
    const next = Array.isArray(v) ? v[0] : v;
    if (typeof next === "string" && ALL_TABS.some((t) => t.key === next)) {
      tab.value = next;
    }
  }
);

// If the active tab is hidden (e.g. "mods" on vanilla), switch to first visible
watch(tabs, (ts) => {
  if (!ts.some((t) => t.key === tab.value) && ts.length > 0) {
    tab.value = ts[0].key;
  }
});


function loaderLabel() {
  const i = instance.value;
  if (!i) return "";
  return i.loader === "vanilla" ? $t("utils.categories.vanilla") : i.loader.charAt(0).toUpperCase() + i.loader.slice(1);
}

// ---- 内容 tab：通过 ref 驱动子组件（tab 栏的检查更新/导入按钮）----

// ---- 截图 tab ----
const shotFiles = ref<
  { name: string; size: number; modified: number; isDir: boolean; path: string; icon: string | null }[]
>([]);
const loadingShots = ref(false);
const previewImg = ref("");
const showPreview = ref(false);

async function loadShots() {
  loadingShots.value = true;
  try {
    const r = await api.listInstanceFiles(instanceId, "screenshots");
    shotFiles.value = r.files;
  } catch (e) {
    message.error(String(e));
  } finally {
    loadingShots.value = false;
  }
}

function assetUrl(p: string) {
  return convertFileSrc(p);
}

// ---- 页头动作 ----
async function launch() {
  const i = instance.value;
  if (!i) return;
  if (!accounts.accounts.length) {
    message.warning($t("instance-saves.need-account"));
    accounts.showManager = true;
    return;
  }
  try {
    // 被渲染器确认弹窗拦下时返回 null，此时游戏还没启动
    const res = await instances.launch(i.id);
    if (res) message.success($t("instance-detail.launched-see-logs"));
  } catch (e) {
    message.error(String(e));
  }
}


// ---- 安装游戏本体（导入分享包后的实例需要）----
const installingGame = ref(false);
async function installGame() {
  installingGame.value = true;
  try {
    await instances.installGame(instanceId);
    message.success($t("instance-detail.game-installed"));
  } catch (e) {
    message.error(String(e));
  } finally {
    installingGame.value = false;
  }
}

function removeInstance() {
  const isSymlink = instance.value?.is_symlink;
  confirmState.value = {
    title: $t("instance-card.delete-instance"),
    content: isSymlink
      ? $t("instance-detail.delete-symlink-confirm", { p1: instance.value?.source_path ? `（${instance.value.source_path}）` : "" })
      : $t("instance-detail.delete-confirm"),
    positiveText: $t("common.delete"),
    onOk: async () => {
      try {
        await instances.remove(instanceId);
        message.success($t("instance-card.on-ok"));
        router.push("/instances");
      } catch (e) {
        message.error(String(e));
      }
    },
  };
}

// —— 实例本体可以分别固定到首页和侧边栏 ——
const homePinId = computed(() => pins.makeId("instance", instanceId, instanceId, "home"));
const sidebarPinId = computed(() => pins.makeId("instance", instanceId, instanceId, "sidebar"));
function toggleInstancePin(target: PinTarget) {
  const i = instance.value;
  if (!i) return;
  pins.toggle({
    id: pins.makeId("instance", instanceId, instanceId, target),
    type: "instance",
    target,
    instanceId,
    instanceName: i.name,
    instanceIcon: i.icon,
    mcVersion: i.mc_version,
    loader: i.loader,
    name: i.name,
    icon: null,
  });
}

// 点击遮罩关闭弹窗（document 委托兜底，naive-ui mask 机制在此环境不可靠）
const confirmCardRef = ref<HTMLElement | null>(null);
const previewCardRef = ref<HTMLElement | null>(null);
function onDocMouseDown(e: MouseEvent) {
  const t = e.target as Element | null;
  if (!t) return;
  if (t.closest(".v-binder-follower-container, .n-base-select-menu, .n-popover, .n-dropdown")) return;
  if (confirmState.value && confirmCardRef.value && !confirmCardRef.value.contains(t)) {
    confirmState.value = null;
    return;
  }
  if (showPreview.value && previewCardRef.value && !previewCardRef.value.contains(t)) {
    showPreview.value = false;
  }
}

onMounted(() => {
  document.addEventListener("mousedown", onDocMouseDown);
  loadFolders();
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocMouseDown);
});

// tab 切换时：只有截图需要父组件加载数据，其余 tab 由子组件自行加载
watch(
  () => tab.value,
  (t) => {
    if (t === "screenshots") loadShots();
  },
  { immediate: true }
);

// 实例安装完成后刷新文件夹列表（决定 tab 显示）
watch(
  () => instance.value?.installed,
  () => loadFolders(),
  { immediate: true }
);
</script>

<template>
  <div v-if="instance" class="detail">
    <!--
      实例头部整块 Teleport 进标题栏（见 TitleBar.vue 的 #tb-context）。
      横屏手机纵向最紧张：这张卡片原本占约 78px 高，
      搬进标题栏后这部分全部还给内容区（左侧导航栏因此能放下全部 9 项）。
      `defer` 让 Teleport 在整棵树渲染完后再挂载，不依赖 TitleBar 的挂载顺序。
    -->
    <Teleport defer to="#tb-context">
      <div class="tb-instance">
        <!-- 与标题栏原有的「(页面图标) 实例详情」拼成面包屑：实例详情 / 1.18.2 -->
        <span class="tb-sep">/</span>
        <h1>{{ instance.name }}</h1>
        <span class="badge">{{ loaderLabel() }}</span>
        <span v-if="instance.loader_version" class="lv">{{ instance.loader_version }}</span>        <div class="d-actions">
          <button class="btn primary" @click="launch">
            <IconPlay />{{ $t("instance-card.launch") }}</button>
          <button
            class="btn ghost pin"
            :class="{ active: pins.isPinned(homePinId) }"
            :title="pins.isPinned(homePinId) ? $t('instance-card.unpin-home') : $t('instance-saves.pin-home')" :aria-label="pins.isPinned(homePinId) ? $t('instance-card.unpin-home') : $t('instance-saves.pin-home')"
            @click="toggleInstancePin('home')"
          >
            <IconMapPin />
          </button>
          <button
            class="btn ghost pin"
            :class="{ active: pins.isPinned(sidebarPinId) }"
            :title="pins.isPinned(sidebarPinId) ? $t('instance-card.unpin-sidebar') : $t('instance-card.pin-sidebar')" :aria-label="pins.isPinned(sidebarPinId) ? $t('instance-card.unpin-sidebar') : $t('instance-card.pin-sidebar')"
            @click="toggleInstancePin('sidebar')"
          >
            <IconLayout />
          </button>
          <button class="btn danger" :title="$t('instance-card.delete-instance')" :aria-label="$t('instance-card.delete-instance')" @click="removeInstance">
            <IconTrash />
          </button>
        </div>
      </div>
    </Teleport>
    <div v-if="!instance.installed" class="not-installed glass">
      <div>
        <h3>{{ $t("instance-detail.game-not-installed") }}</h3>
        <p>{{ $t("instance-detail.install-to-launch", { p1: instance.mc_version }) }}</p>
      </div>
      <button class="btn primary" :disabled="installingGame" @click="installGame">
        <IconPlay /> {{ installingGame ? $t('instance-detail.installing') : $t('instance-detail.install-game') }}
      </button>
    </div>

    <div v-if="instance.is_symlink" class="symlink-notice glass">
      <IconExternal />
      <span>{{ $t("instance-detail.symlink-notice") }}<template v-if="instance.source_path">{{ $t("instance-detail.source", { p1: instance.source_path }) }}</template>{{ $t("instance-detail.symlink-notice-2") }}</span>
    </div>

    <!--
      手机横屏是「宽而矮」（约 873 × 393 CSS px）：横向富余、纵向紧张。
      所以选项卡从横着一条（吃掉 48px 纵向）改成左侧竖向导航栏 ——
      Material 的 navigation rail 思路，用 naive-ui 的 NMenu 实现；
      主题在 theme.ts 的 Menu 覆盖里，自动跟随强调色。
      顺带把原来挤在标签栏右侧的上下文按钮（检查更新/导入本地/打开文件夹）
      移到导航栏底部竖排。
    -->
    <div class="d-body">
      <aside class="d-rail glass">
        <n-menu
          class="d-rail-menu"
          :value="tab"
          :options="railOptions"
          :indent="16"
          :root-indent="8"
          mode="vertical"
          @update:value="(k: string) => (tab = k)"
        />
        <!-- 「检查更新 / 导入本地」已挪进右侧内容区自己的工具栏（ContentTab）：
             挂在侧栏底部既和内容区里的操作重复，又把侧栏撑到溢出屏幕
             （最底下的 tab 只露出一个头）。 -->
      </aside>

    <div class="tab-body">
      <!-- mods / resourcepacks / shaders -->
      <ContentTab
        v-if="CONTENT_TABS.includes(tab)"
        :instance-id="instanceId"
        :kind="kindOf(tab)"
      />

      <!-- screenshots -->
      <template v-if="tab === 'screenshots'">
        <div v-if="loadingShots" class="center">{{ $t("file-manager.loading") }}</div>
        <div v-else-if="!shotFiles.length" class="empty glass">
          <p>{{ $t("instance-detail.no-screenshot") }}</p>
          <button class="btn ghost" @click="tab = 'files'"><IconHardDrive />{{ $t("instance-detail.open-in-file-manager") }}</button>
        </div>
        <div v-else class="shot-grid">
          <div
            v-for="f in shotFiles.filter((x) => !x.isDir)"
            :key="f.name"
            class="shot-card glass clickable"
            @click="previewImg = assetUrl(f.path); showPreview = true"
          >
            <img :src="assetUrl(f.path)" class="shot-img" alt="" loading="lazy" />
            <div class="shot-info">
              <div class="shot-name text-ellipsis">{{ f.name }}</div>
              <div class="shot-meta">{{ fmtSize(f.size) }} · {{ fmtDate(f.modified) }}</div>
            </div>
          </div>
        </div>
      </template>

      <!-- 世界：单人游戏 / 多人游戏 -->
      <SavesTab v-if="tab === 'saves'" :instance-id="instanceId" />

      <!-- files -->
      <template v-if="tab === 'files'">
        <FileManager :instance-id="instanceId" />
      </template>

      <!-- logs -->
      <template v-if="tab === 'logs'">
        <LogViewer :instance-id="instanceId" />
      </template>

      <!-- crash analysis -->
      <template v-if="tab === 'crash'">
        <CrashAnalyzer :instance-id="instanceId" />
      </template>

      <!-- 按键（触控控制层布局管理） -->
      <KeysTab v-if="tab === 'keys'" :instance-id="instanceId" />

      <!-- settings -->
      <SettingsTab v-if="tab === 'settings'" :instance-id="instanceId" />
    </div>
    </div>

    <!-- confirm dialog -->
    <app-sheet
      :show="confirmState !== null"
      :title="confirmState?.title ?? ''"
      :mask-closable="true"
      @update:show="(v: boolean) => { if (!v) confirmState = null; }"
      @mask-click="confirmState = null"
    >
      <div v-if="confirmState" ref="confirmCardRef" style="display: flex; flex-direction: column; gap: 16px;">
        <div style="font-size: 14px; color: var(--text-2); line-height: 1.6;">{{ confirmState.content }}</div>
        <div style="display: flex; justify-content: flex-end; gap: 10px;">
          <n-button @click="confirmState = null">{{ $t("common.cancel") }}</n-button>
          <n-button type="error" :loading="confirmLoading" @click="handleConfirm">{{ confirmState.positiveText }}</n-button>
        </div>
      </div>
    </app-sheet>

    <!-- screenshot preview -->
    <app-sheet
      v-model:show="showPreview"
      :title="$t('instance-detail.screenshot-preview')"
      :mask-closable="true"
      @mask-click="showPreview = false"
    >
      <img ref="previewCardRef" :src="previewImg" class="preview-img" alt="" />
    </app-sheet>
  </div>
  <div v-else class="center">{{ $t("instance-detail.instance-gone") }}</div>
</template>

<style scoped>
.detail {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.symlink-notice {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 20px;
  margin-top: 12px;
  font-size: 13px;
  color: #e8a33d;
  background: rgba(232, 163, 61, 0.1);
  border-radius: 12px;
}
.symlink-notice svg {
  flex-shrink: 0;
}
.d-icon {
  width: 56px;
  height: 56px;
  border-radius: 14px;
  overflow: hidden;
  background: transparent;
  position: relative;
  font-size: 26px;
  color: var(--accent);
  flex-shrink: 0;
  box-sizing: border-box;
}
.d-icon :deep(.app-icon) {
  position: absolute;
  inset: 0;
}
.d-info {
  flex: 1;
  min-width: 0;
}
.d-info h1 {
  margin: 0 0 6px;
  font-size: 21px;
}
.d-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.badge {
  background: var(--accent-16);
  color: var(--accent);
  border-radius: 6px;
  padding: 1px 8px;
  font-weight: 600;
}
.mc {
  color: var(--text-2);
  font-weight: 600;
}
.lv {
  color: var(--text-3);
}
.d-actions {
  display: flex;
  gap: 8px;
}
.btn {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  border: none;
  border-radius: 10px;
  padding: 9px 16px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.14s;
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
.btn.ghost.pin.active {
  color: var(--accent);
  border-color: var(--accent-04);
  background: var(--accent-soft);
}
.btn.danger {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-1);
  border: 1px solid var(--border);
}
.btn.danger:hover {
  color: #e5534b;
  border-color: rgba(229, 83, 75, 0.5);
}
.btn:disabled {
  opacity: 0.5;
  cursor: default;
}
.not-installed {
  padding: 20px 24px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  border-color: rgba(224, 160, 48, 0.35);
}
.not-installed h3 {
  margin: 0 0 4px;
  font-size: 15px;
  color: #e0a030;
}
.not-installed p {
  margin: 0;
  color: var(--text-2);
  font-size: 13px;
}
.center {
  padding: 60px;
  text-align: center;
  color: var(--text-3);
}
.shot-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(200px, 100%), 1fr));
  gap: 12px;
}
.shot-card {
  overflow: hidden;
  padding: 0;
}
.shot-img {
  width: 100%;
  aspect-ratio: 16 / 9;
  object-fit: cover;
  display: block;
  background: rgba(0, 0, 0, 0.3);
}
.shot-info {
  padding: 8px 10px;
}
.shot-name {
  font-size: 12px;
  font-weight: 600;
  margin-bottom: 3px;
}
.shot-meta {
  font-size: 11px;
  color: var(--text-3);
}
.preview-img {
  width: 100%;
  max-height: calc(70vh / var(--ui-scale, 1));
  object-fit: contain;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.4);
}
.empty {
  padding: 40px;
  text-align: center;
  color: var(--text-3);
  display: flex;
  flex-direction: column;
  gap: 14px;
  align-items: center;
}

/* ── 标题栏里的实例标识 ────────────────────────────────────────────
   原来实例头部是页面里的一张卡片（约 96px 高）。现在只把「实例名 + 加载器徽章」
   接到标题栏原有的路由标题后面，构成面包屑：`(页面图标) 实例详情 / 1.18.2`；
   操作按钮（启动游戏等）靠右贴边。这样纵向省下整张卡片，横向也读得顺。
   元素带 scoped 属性，所以样式仍写在组件里；容器布局在 TitleBar.vue。 */
.tb-instance {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}
.tb-sep {
  color: var(--text-3);
  font-size: 12px;
  flex-shrink: 0;
}
.tb-instance h1 {
  font-size: 13.5px;
  font-weight: 700;
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.tb-instance .badge,
.tb-instance .lv {
  font-size: 11px;
  white-space: nowrap;
  flex-shrink: 0;
}
.tb-instance .d-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  flex-shrink: 0;
}
.tb-instance .btn {
  padding: 6px 12px;
  font-size: 12px;
}
.tb-instance .btn.ghost {
  padding: 6px 9px;
}

/* 导航栏字号：之前直接吃 naive-ui 默认值（约 14px），在 146px 宽的竖排栏里偏大 */
.d-rail :deep(.n-menu-item-content) {
  padding-left: 10px !important;
}
.d-rail :deep(.n-menu-item-content-header) {
  font-size: 12.5px;
  font-weight: 600;
}
.d-rail :deep(.n-menu-item-content__icon) {
  font-size: 15px;
}/* ── 左侧竖向导航栏（替代原来的横向标签栏）──────────────────────────
   手机横屏宽 873 × 高 393 CSS px：横向富余、纵向紧张。
   原来横向标签栏占 48px 纵向；改成竖排后这 48px 全部还给内容区
   （文件浏览器因此能多显示两行）。
   菜单本体用 naive-ui 的 NMenu，配色在 theme.ts 的 Menu 覆盖里。 */
.d-body {
  display: flex;
  gap: 14px;
  /* 页面级滚动已取消，这里就能用 flex 填满剩余高度（不用再写 52vh 这类魔法数字） */
  flex: 1;
  min-height: 0;
  align-items: stretch;
}
.d-rail {
  /* 宽度与内容中心的类型栏共用 `--rail-w`（见 styles.css），不要在这里写死 */
  width: var(--rail-w);
  flex-shrink: 0;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  /* 9 项 × 26px 加上自身 padding ≈ 246px，而横屏手机可用高度约 240~260px，
     刚好贴边 —— 所以必须能滚。注意还要求子项 `.d-rail-menu` 一起加 min-height:0，
     否则 NMenu 会自己把溢出裁掉、`.d-rail` 的 scrollHeight 不变（有内容却拖不动）。 */
  overflow-y: auto;
  min-height: 0;
}
.d-rail-menu {
  /* NMenu 自带不透明背景，这里交给外层 .glass */
  background: transparent;
  /* 关键：flex 子项默认 `min-height: auto` 不会收缩，NMenu 会把超出的菜单项
     直接裁掉 —— 表现就是「侧栏看着还有内容、却怎么都拖不动」。 */
  flex: 0 1 auto;
  min-height: 0;
}
.tab-body {
  flex: 1;
  min-width: 0;
  min-height: 0;
  /* 三个 tab 组件现在各自内部滚动（`.tab-root` / `.settings-grid`），
     这里必须裁掉溢出，否则内容会顶到 `.detail` 让整页滚起来。 */
  overflow: hidden;
}

/* 横屏手机上收紧间距；导航栏再窄一点，把宽度让给内容 */
@media (max-width: 1100px), (pointer: coarse) {
  .detail {
    gap: 10px;
  }
  .d-rail {
    /* 宽度交给 `--rail-w` 自适应，这里只收紧内边距 */
    padding: 4px;
  }
}</style>
