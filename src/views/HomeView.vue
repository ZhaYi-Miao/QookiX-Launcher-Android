<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useInstancesStore } from "../stores/instances";
import { useAccountsStore } from "../stores/accounts";
import { usePinsStore, type PinItem } from "../stores/pins";
import { useSettingsStore } from "../stores/settings";
import { latencyInfo, loaderBadge } from "../utils/format";
import { api } from "../api";
import { supportsQuickPlay } from "../version";
import { useMessage, NDrawer, NDrawerContent, NModal } from "naive-ui";
import { useIsMobile } from "../composables/useMediaQuery";
import AppIcon from "../components/AppIcon.vue";
import PlaytimeCard from "../components/PlaytimeCard.vue";
import type { ServerStatus } from "../types";
import { IconClose, IconCompass, IconFolder, IconGlobe, IconPlay, IconRepeat, IconUser } from "../components/icons";

const router = useRouter();
const instances = useInstancesStore();
const accounts = useAccountsStore();
const message = useMessage();
/** 手机（含横屏）：选实例用底部抽屉，而不是桌面式居中弹窗。 */
const isMobile = useIsMobile();
const pinsStore = usePinsStore();
const settingsStore = useSettingsStore();
const launching = ref(false);
const showPicker = ref(false);
/** 首页英雄卡左下角的统计卡可点击 → 弹出完整统计（柱状图 + 每实例时长） */
const showPlaytime = ref(false);
const pinLaunching = ref<string>("");
const pinStatus = ref<Record<string, ServerStatus>>({});

const STORAGE_KEY = "qookix.home.selected";

const lastPlayed = computed(() =>
  [...instances.instances].sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0))[0] ?? null
);

// 常驻实例：默认取上次选择 / 最近游玩 / 第一个实例
const selectedId = ref<string | null>(null);

function resolveDefault(): string | null {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved && instances.get(saved)) return saved;
  return (lastPlayed.value ?? instances.instances[0])?.id ?? null;
}

const selected = computed(() =>
  selectedId.value ? (instances.get(selectedId.value) ?? null) : null
);

watch(
  () => instances.instances,
  () => {
    if (!selectedId.value) selectedId.value = resolveDefault();
  },
  { immediate: true }
);

watch(selectedId, (id) => {
  if (id) localStorage.setItem(STORAGE_KEY, id);
});

// 切换实例弹窗按分组展示（空分组不显示）
const pickerSections = computed(() => {
  const list = instances.groups
    .map((g) => ({
      key: g.id,
      name: g.name,
      color: g.color as string | null,
      items: instances.instances.filter((i) => i.group === g.id),
    }))
    .filter((s) => s.items.length);
  const rest = instances.instances.filter((i) => !i.group);
  if (rest.length) {
    list.push({ key: "__ungrouped__", name: "未分组", color: null, items: rest });
  }
  return list;
});

const greeting = computed(() => {
  const h = new Date().getHours();
  if (h >= 5 && h < 11) return "早上好";
  if (h >= 11 && h < 13) return "中午好";
  if (h >= 13 && h < 18) return "下午好";
  if (h >= 18 && h < 22) return "晚上好";
  return "夜深了";
});

const hasAccount = computed(() => accounts.accounts.length > 0);

async function launchSelected() {
  const target = selected.value;
  if (!target) {
    message.info("还没有实例，先创建一个吧");
    router.push("/instances");
    return;
  }
  if (!hasAccount.value) {
    message.warning("请先在左下角账号栏添加账号（正版或离线）");
    accounts.showManager = true;
    return;
  }
  launching.value = true;
  try {
    await instances.launch(target.id);
    message.success(`已启动 ${target.name}`);
  } catch (e) {
    message.error(String(e));
  } finally {
    launching.value = false;
  }
}

function pick(inst: { id: string }) {
  selectedId.value = inst.id;
  showPicker.value = false;
}

function openPicker() {
  if (!instances.instances.length) {
    message.info("还没有实例，先创建一个吧");
    router.push("/instances");
    return;
  }
  showPicker.value = true;
}

// —— 固定快捷启动 ——
// 只展示固定到首页的项（侧边栏固定不在此显示）；实例被删除后自动隐藏对应固定项
const validPins = computed(() =>
  pinsStore.items.filter((p) => p.target === "home" && instances.get(p.instanceId))
);

function pinIconSrc(p: PinItem): string | undefined {
  if (p.type !== "server") return undefined;
  const fav = pinStatus.value[p.id]?.favicon;
  if (fav) return fav;
  if (p.icon) return `data:image/png;base64,${p.icon}`;
  return undefined;
}

function worldIconSrc(p: PinItem): string | undefined {
  if (!p.icon) return undefined;
  if (p.icon.startsWith("http://") || p.icon.startsWith("https://") || p.icon.startsWith("data:")) return p.icon;
  return convertFileSrc(p.icon);
}

async function pingPin(p: PinItem) {
  if (p.type !== "server" || !p.address) return;
  try {
    const st = await api.pingServer(p.address);
    pinStatus.value = { ...pinStatus.value, [p.id]: st };
  } catch {
    pinStatus.value = {
      ...pinStatus.value,
      [p.id]: { online: false, address: p.address, name: null, version: null, players_online: null, players_max: null, motd: null, favicon: null, latency_ms: null, error: null },
    };
  }
}

async function launchPin(p: PinItem) {
  if (!hasAccount.value) {
    message.warning("请先在左下角账号栏添加账号（正版或离线）");
    accounts.showManager = true;
    return;
  }
  pinLaunching.value = p.id;
  if (p.type === "world" && !supportsQuickPlay(p.mcVersion)) {
    message.info(`此实例是 ${p.mcVersion}，不支持命令行直达存档，将启动游戏后手动进入存档`);
  }
  try {
    await instances.launch(p.instanceId, p.world, p.address);
    const msg =
      p.type === "server" ? `正在加入服务器「${p.name}」`
      : p.type === "world" ? `正在进入世界「${p.name}」`
      : `正在启动实例「${p.name}」`;
    message.success(msg);
  } catch (e) {
    message.error(String(e));
  } finally {
    pinLaunching.value = "";
  }
}

function unpin(p: PinItem) {
  pinsStore.remove(p.id);
}

function openInstance(p: PinItem) {
  if (p.type === "instance") {
    router.push(`/instance/${p.instanceId}`);
  } else {
    router.push({ path: `/instance/${p.instanceId}`, query: { tab: "saves" } });
  }
}

function pinTypeLabel(t: PinItem["type"]): string {
  return t === "server" ? "服务器" : t === "world" ? "存档" : "实例";
}

onMounted(() => {
  instances.load();
  accounts.load();
  settingsStore.load();
  pinsStore.items.filter((p) => p.type === "server").forEach((p) => pingPin(p));
});
</script>

<template>
  <div class="home">
    <section v-if="settingsStore.settings?.show_home_hero" class="hero glass">
      <div class="hero-glow"></div>
      <div class="hero-text">
        <div class="greeting">{{ greeting }}</div>
        <h1>开始你的 <span class="accent">方块之旅</span></h1>
        <p>选择一个实例，一键启动</p>
        <div class="hero-actions">
          <button class="btn ghost big" @click="router.push('/browse')">
            <IconCompass /> 浏览内容
          </button>
          <button class="btn ghost big" @click="accounts.showManager = true">
            <IconUser /> 切换账号
          </button>
        </div>
        <!-- 游戏统计：双栏布局下贴在英雄卡左下角，宽度与上面按钮行对齐；
             点击弹出完整统计（柱状图 + 每实例时长）。窄屏竖排时隐藏。 -->
        <div
          class="hero-playtime"
          role="button"
          title="点击查看详细统计"
          @click="showPlaytime = true"
        ><PlaytimeCard /></div>
      </div>
      <div class="hero-logo">
        <img src="/app-icon.png" class="hero-logo-img" draggable="false" alt="" />
      </div>
    </section>

    <section v-if="validPins.length" class="pin-block">
      <div class="pin-grid">
        <div v-for="p in validPins" :key="p.id" class="pin-card glass" @click="openInstance(p)">
          <div class="pin-icon">
            <template v-if="p.type === 'server'">
              <img v-if="pinIconSrc(p)" :src="pinIconSrc(p)" class="pin-icon-img" alt="" />
              <IconGlobe v-else />
            </template>
            <template v-else-if="p.type === 'world'">
              <img v-if="worldIconSrc(p)" :src="worldIconSrc(p)" class="pin-icon-img" alt="" />
              <IconFolder v-else />
            </template>
            <template v-else>
              <AppIcon :name="p.instanceIcon" />
            </template>
          </div>
          <div class="pin-info">
            <div class="pin-title text-ellipsis">{{ p.name }}</div>
            <div class="pin-meta">
              <span class="pin-type" :class="p.type">{{ pinTypeLabel(p.type) }}</span>
              <span v-if="p.type === 'instance'" class="pin-inst text-ellipsis">{{ p.mcVersion }} · {{ p.loader }}</span>
              <span v-else class="pin-inst text-ellipsis">{{ p.instanceName }}</span>
            </div>
            <div v-if="p.type === 'server'" class="pin-status">
              <span v-if="pinStatus[p.id]" :class="['latency', latencyInfo(pinStatus[p.id].latency_ms).tier]">
                <span class="bars">
                  <i v-for="n in 5" :key="n" :class="{ on: n <= latencyInfo(pinStatus[p.id].latency_ms).count }"></i>
                </span>
                <span v-if="pinStatus[p.id].latency_ms != null">{{ pinStatus[p.id].latency_ms }} ms</span>
                <span v-else-if="!pinStatus[p.id].online">离线</span>
                <span v-else>…</span>
              </span>
              <span v-if="pinStatus[p.id]?.players_online != null" class="players">
                {{ pinStatus[p.id].players_online }} 人在线
              </span>
            </div>
          </div>
          <div class="pin-actions">
            <button class="pin-unpin" title="取消固定" aria-label="取消固定" @click.stop="unpin(p)">
              <IconClose />
            </button>
            <button class="btn primary" :disabled="pinLaunching === p.id" @click.stop="launchPin(p)">
              <IconPlay /> {{ pinLaunching === p.id ? "启动中…" : "启动" }}
            </button>
          </div>
        </div>
      </div>
    </section>

    <section class="section">
      <div v-if="!instances.instances.length" class="empty glass">
        <p>还没有游戏实例</p>
        <button class="btn primary" @click="router.push('/instances')">创建第一个实例</button>
      </div>

      <div v-else-if="selected" class="resident glass">
        <div class="resident-icon"><AppIcon :name="selected.icon" /></div>
        <div class="resident-info">
          <div class="resident-name text-ellipsis">{{ selected.name }}</div>
          <div class="resident-meta">
            <span class="badge">{{ loaderBadge(selected.loader) }}</span>
            <span class="ver-text">{{ selected.mc_version }}</span>
            <span v-if="selected.loader_version" class="ver-text">· {{ selected.loader_version }}</span>
            <span v-if="selected.last_played" class="ver-text">
              · 最近 {{ new Date(selected.last_played * 1000).toLocaleDateString() }}
            </span>
          </div>
        </div>
        <div class="resident-actions">
          <button class="btn ghost" @click="openPicker">
            <IconRepeat /> 切换实例
          </button>
          <button class="btn primary big" :disabled="launching" @click="launchSelected">
            <IconPlay />
            <span>{{ launching ? "启动中…" : "启动游戏" }}</span>
          </button>
        </div>
      </div>
    </section>

    <!-- 手机用**底部抽屉**（拇指区、可下滑关闭），桌面仍是右侧抽屉 —— 居中弹窗在
         手机上很「电脑」，而且 853×384 的横屏里它挡住大半个屏幕。 -->
    <n-drawer
      :auto-focus="false"
      :show="showPicker"
      :placement="isMobile ? 'bottom' : 'right'"
      :width="isMobile ? undefined : 460"
      :height="isMobile ? '76%' : undefined"
      @update:show="(v: boolean) => (showPicker = v)"
    >
      <n-drawer-content title="切换实例" closable>
        <div class="pick-scroll">
        <section v-for="s in pickerSections" :key="s.key" class="pick-section">
          <div class="pick-group">
            <i class="dot" :style="{ background: s.color || 'var(--text-3)' }"></i>
            <span>{{ s.name }}</span>
            <span class="pick-group-count">{{ s.items.length }}</span>
          </div>
          <div class="pick-grid">
            <div
              v-for="inst in s.items"
              :key="inst.id"
              class="pick-card"
              :class="{ active: selected?.id === inst.id }"
              @click="pick(inst)"
            >
              <div class="pick-icon"><AppIcon :name="inst.icon" /></div>
              <div class="pick-info">
                <div class="pick-name text-ellipsis">{{ inst.name }}</div>
                <div class="pick-meta">
                  <span class="badge">{{ loaderBadge(inst.loader) }}</span>
                  <span class="ver-text">{{ inst.mc_version }}</span>
                </div>
              </div>
              <div v-if="selected?.id === inst.id" class="pick-current">当前</div>
            </div>
          </div>
        </section>
        </div>
      </n-drawer-content>
    </n-drawer>

    <!-- 完整游戏统计：柱状图 + 每实例时长（首页左下角统计卡的详情）。
         n-modal Teleport 到 body，不参与 #app 的 zoom，宽高用固定上限。 -->
    <n-modal
      v-model:show="showPlaytime"
      preset="card"
      title="游戏统计"
      :style="{ width: 'min(560px, 92vw)' }"
      :bordered="false"
    >
      <PlaytimeCard />
    </n-modal>
  </div>
</template>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-height: 100%;
}
.hero {
  position: relative;
  overflow: hidden;
  padding: 20px 24px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
}
.hero-glow {
  position: absolute;
  width: 420px;
  height: 420px;
  right: -80px;
  top: -180px;
  background: radial-gradient(circle, var(--accent-25), transparent 65%);
  pointer-events: none;
}
.hero-text {
  position: relative;
  z-index: 1;
}
.hero h1 {
  font-size: 30px;
  margin: 0 0 10px;
  letter-spacing: 0.3px;
  line-height: 1.1;
}
.hero p {
  color: var(--text-2);
  margin: 0 0 22px;
  max-width: 520px;
  line-height: 1.6;
}
.greeting {
  font-size: 18px;
  font-weight: 600;
  color: var(--accent);
  margin: 0;
  line-height: 1.2;
  letter-spacing: 0.5px;
}
.accent {
  color: var(--accent);
}
.hero-actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}
.btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  border: none;
  border-radius: 10px;
  padding: 9px 18px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
  font-family: inherit;
}
.btn.big {
  padding: 10px 22px;
  font-size: 14px;
}
.btn.primary {
  background: linear-gradient(135deg, var(--accent), var(--accent-deep));
  color: #1a1208;
  box-shadow: 0 6px 22px var(--accent-35);
}
.btn.primary:hover {
  filter: brightness(1.08);
}
.btn.primary:disabled {
  opacity: 0.7;
  cursor: default;
}
.btn.ghost {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-1);
  border: 1px solid var(--border);
}
.btn.ghost:hover {
  background: rgba(255, 255, 255, 0.1);
  transform: none;
}
.hero-logo {
  position: relative;
  z-index: 1;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.hero-logo-img {
  width: 128px;
  height: 128px;
  border-radius: 24px;
  object-fit: contain;
}
/* 游戏统计卡：只在双栏布局（宽屏/横屏）里显示，锚在英雄卡左下角 */
.hero-playtime {
  display: none;
}
/* 实例卡不再**单独**限宽居中。
   原来它自带 `max-width: 1080px; margin: auto`，而欢迎卡通栏 ——
   布局视口一变宽（界面缩放调到 100% 以下时布局视口会成比例变大：
   60% 缩放 → 853px 变成 1421 CSS px）两者宽度就对不上，
   实测英雄卡 839px、实例卡只剩 648px 还偏到 x=103，
   看起来就像「布局坏了 / 不自适应」。
   现在宽度由 `.home` 统一收口（见下），这里只管纵向。 */
.section {
  margin-top: auto;
  width: 100%;
}
/* 固定快捷启动 */
.pin-block {
  margin-top: 8px;
}
.pin-sub {
  color: var(--text-3);
  font-size: 12px;
}
.pin-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(280px, 100%), 1fr));
  gap: 16px;
}
.pin-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 18px;
  cursor: pointer;
  transition: border-color 0.15s, transform 0.15s, background 0.15s;
}
.pin-card:hover {
  border-color: var(--accent-45);
  background: rgba(255, 255, 255, 0.06);
  transform: translateY(-1px);
}
.pin-icon {
  width: 46px;
  height: 46px;
  border-radius: 12px;
  flex-shrink: 0;
  background: linear-gradient(135deg, var(--accent-22), var(--accent-08));
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  color: var(--accent);
  overflow: hidden;
}
.pin-icon-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.pin-info {
  flex: 1;
  min-width: 0;
}
.pin-title {
  font-size: 15px;
  font-weight: 600;
  margin-bottom: 6px;
}
.pin-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.pin-type {
  border-radius: 6px;
  padding: 0 6px;
  font-weight: 600;
  flex-shrink: 0;
}
.pin-type.server {
  background: rgba(90, 176, 255, 0.15);
  color: #5ab0ff;
}
.pin-type.world {
  background: rgba(122, 208, 138, 0.15);
  color: #7ad08a;
}
.pin-type.instance {
  background: var(--accent-16);
  color: var(--accent);
}
.pin-inst {
  color: var(--text-3);
}
.pin-status {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 6px;
  font-size: 12px;
}
.pin-status .latency {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.pin-status .bars {
  display: inline-flex;
  gap: 2px;
}
.pin-status .bars i {
  width: 4px;
  height: 10px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.15);
}
.pin-status .bars i.on {
  background: currentColor;
}
.pin-status .latency.good {
  color: #7ad08a;
}
.pin-status .latency.mid {
  color: #ffc34d;
}
.pin-status .latency.bad {
  color: #ff6b6b;
}
.pin-status .latency.off {
  color: var(--text-3);
}
.pin-status .players {
  color: var(--text-3);
}
.pin-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.pin-actions .btn {
  padding: 8px 14px;
  font-size: 13px;
}
.pin-unpin {
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--text-3);
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}
.pin-unpin:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #ff6b6b;
}
.section-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 14px;
}
.section-head h2 {
  font-size: 17px;
  margin: 0;
}
.link-btn {
  background: none;
  border: none;
  color: var(--text-3);
  cursor: pointer;
  font-size: 13px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.link-btn:hover {
  color: var(--accent);
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

/* 常驻实例卡片 */
.resident {
  padding: 20px 24px;
  display: flex;
  align-items: center;
  gap: 16px;
}
.resident-icon {
  width: 64px;
  height: 64px;
  border-radius: 16px;
  overflow: hidden;
  background: linear-gradient(135deg, var(--accent-25), var(--accent-08));
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 28px;
  color: var(--accent);
  flex-shrink: 0;
}
.resident-info {
  flex: 1;
  min-width: 0;
}
.resident-name {
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 8px;
}
.resident-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  flex-wrap: wrap;
}
.badge {
  background: var(--accent-16);
  color: var(--accent);
  border-radius: 6px;
  padding: 1px 7px;
  font-weight: 600;
}
.ver-text {
  color: var(--text-3);
}
.resident-actions {
  display: flex;
  gap: 10px;
  align-items: center;
  flex-shrink: 0;
}

/* 窄屏竖屏（手机）：一行放不下「图标 + 信息 + 两个按钮」，
   会让中间的 .resident-info 被压成 0 宽、按钮与文字互相重叠。
   这里改成两行：第一行图标 + 信息，第二行两个按钮等宽铺满。 */
@media (max-width: 1100px), (pointer: coarse) {
  .resident {
    flex-wrap: wrap;
    padding: 16px;
    gap: 12px;
  }
  .resident-info {
    flex: 1 1 calc(100% - 76px);
  }
  .resident-actions {
    flex: 1 1 100%;
  }
  .resident-actions .btn {
    flex: 1;
    justify-content: center;
  }
}

/* 切换弹窗 */
.pick-scroll {
  display: flex;
  flex-direction: column;
  gap: 16px;
  /* 这个列表在 n-modal 里（Teleport 到 body），不参与 #app 的 zoom */
  max-height: 56vh;
  overflow-y: auto;
  padding-right: 4px;
}
.pick-group {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 10px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}
.pick-group .dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
}
.pick-group-count {
  font-size: 11px;
  color: var(--text-3);
}
.pick-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(220px, 100%), 1fr));
  gap: 12px;
}
.pick-card {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--border);
  cursor: pointer;
  transition: border-color 0.15s, transform 0.15s, background 0.15s;
}
.pick-card:hover {
  background: rgba(255, 255, 255, 0.07);
}
.pick-card.active {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
  background: var(--accent-01);
}
.pick-icon {
  width: 42px;
  height: 42px;
  border-radius: 11px;
  overflow: hidden;
  background: linear-gradient(135deg, var(--accent-25), var(--accent-08));
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 19px;
  color: var(--accent);
  flex-shrink: 0;
}
.pick-info {
  min-width: 0;
  flex: 1;
}
.pick-name {
  font-weight: 600;
  font-size: 14px;
  margin-bottom: 5px;
}
.pick-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.pick-current {
  position: absolute;
  top: 8px;
  right: 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--accent);
  background: var(--accent-soft);
  border-radius: 6px;
  padding: 1px 6px;
}

/* ── 手机首页：从「桌面营销页」改成「移动端一屏一件事」────────────────
   桌面那套（大标题 + 副标题 + 立绘 + 自动换行网格 + 居中弹窗）在手机上：
     · 大标题/副标题/立绘占掉一半纵向空间，真正的「启动游戏」反而被挤下去；
     · 固定项用网格换行，条目一多就吃掉整屏；
     · 选实例是桌面式居中弹窗，横屏里挡住大半个屏幕。
   这里按移动端习惯重排：① 问候压成一行 ② 固定项改横滑卡片条
   ③ 当前实例是唯一主操作、启动按钮通栏（触控高度 ≥46px）
   ④ 选实例用底部抽屉（见模板里的 n-drawer）。 */
@media (max-width: 1100px), (pointer: coarse) {
  .hero {
    padding: 12px 16px;
    gap: 10px;
  }
  .hero-text {
    font-size: 22px;
    margin: 0 0 4px;
  }
  .greeting {
    font-size: 14px;
  }
  .hero-logo-img {
    height: 84px;
  }
  .hero-actions {
    gap: 8px;
  }
  .hero-actions .btn {
    padding: 7px 12px;
    font-size: 12px;
  }
}

/* ── 手机上的欢迎卡 ────────────────────────────────────────────────────
   桌面那版（大标题 30px + 副标题 + 128px 立绘 + 按钮）在 384px 高的屏幕上占掉近一半；
   但整块删掉又太素（用户反馈「很难看，把欢迎卡片整回来」）。
   折中成一张**紧凑欢迎卡**：图标在左、问候 + 标题在右，底部两个通栏动作。 */
@media (max-width: 1100px), (pointer: coarse) {
  .home > .hero {
    flex: 0 0 auto;
    padding: 12px;
    gap: 12px;
    align-items: center;
  }
  /* 图标留一个够认的尺寸即可（原来 128px 会把卡片顶高） */
  .hero-logo {
    display: block;
    order: -1;
    flex-shrink: 0;
  }
  .hero-logo-img {
    width: 52px;
    height: 52px;
  }
  .hero-text {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 2px;
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .greeting {
    font-size: 12px;
    margin: 0;
  }
  .hero-text h1,
  .hero h1 {
    font-size: 17px;
    margin: 0;
    line-height: 1.25;
  }
  /* 副标题是纯营销文案，手机上省掉；换成卡片底部的两个动作 */
  .hero-text p {
    display: none;
  }
  .hero-actions {
    width: 100%;
    gap: 8px;
    margin-top: 6px;
  }
  /* 两个动作等宽通栏：手机上是拇指主要落点，别做成小胶囊 */
  .hero-actions .btn {
    flex: 1 1 0;
    justify-content: center;
    min-height: 40px;
    padding: 8px 10px;
    font-size: 13px;
  }

  /* ── 固定项：网格换行 → 横滑卡片条 ──────────────────────────────
     网格是「有多少排多少」，固定几个就把整屏吃掉；横滑只占一行高度，
     拇指左右划就能翻（scroll-snap 让卡片吸附对齐）。 */
  .pin-block {
    flex: 0 0 auto;
  }
  .pin-grid {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scroll-snap-type: x mandatory;
    padding-bottom: 2px;
    /* 隐藏滚动条：横滑靠手势，不需要占位 */
    scrollbar-width: none;
  }
  .pin-grid::-webkit-scrollbar {
    display: none;
  }
  .pin-card {
    flex: 0 0 246px;
    scroll-snap-align: start;
    padding: 12px;
    gap: 10px;
  }
  .pin-icon {
    width: 40px;
    height: 40px;
    border-radius: 11px;
  }

  /* ── 当前实例：唯一主操作 ────────────────────────────────────────
     启动按钮通栏、高度 ≥46px（拇指友好）；切换实例降为等宽次级按钮。 */
  .resident {
    padding: 14px;
    gap: 12px;
    flex-wrap: wrap;
  }
  .resident-icon {
    width: 52px;
    height: 52px;
    border-radius: 14px;
    font-size: 24px;
  }
  .resident-name {
    font-size: 17px;
  }
  .resident-actions {
    width: 100%;
    gap: 8px;
  }
  .resident-actions .btn {
    min-height: 46px;
    justify-content: center;
  }
  .resident-actions .btn.primary {
    flex: 1 1 auto;
    font-size: 15px;
  }
  .resident-actions .btn.ghost {
    flex: 0 0 auto;
  }
}
/* 首页的「内容栏」宽度在这里统一收口：所有块（欢迎卡 / 实例卡 / 固定项）
   拿到的宽度完全一致，不会再出现「一块通栏、一块居中限宽」的错位。
   上限给 1400px —— 超宽屏（低缩放、外接屏）不至于把卡片拉成一条长横条。 */
.home {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 100%;
  max-width: 1400px;
  margin-inline: auto;
}
.home > section {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* ── 大屏密度/小视口设备的收尾（实测 OnePlus Ace 5：792×360 CSS，内容区仅 240px）──
   英雄卡 + 实例/空状态卡在 240px 里放不下。原先是只让卡片内部滚，
   结果「创建第一个实例」按钮被底部导航切成一半 —— 这是新用户看到的第一屏，
   而且没有滚动提示，很容易以为按钮坏了。
   首页只有两张卡、没有长列表，改成整页滚动更自然，按钮也完整可见。 */
@media (max-width: 1100px), (pointer: coarse) {
  .home {
    overflow-y: auto;
    /* 装饰元素（英雄卡的 hero-glow 等）绝不允许撑出横向滚动条。
       实测踩过：下面 `.home > section { overflow: visible }` 的特异性
       高于 `.hero { overflow: hidden }`，把英雄卡的裁剪覆盖掉了 ——
       420px 的装饰光斑直接冲出卡片，`.home` 就变成能左右滑的容器
       （overflow-y:auto 会把另一轴也算成 auto）。 */
    overflow-x: hidden;
  }
  .home > section {
    /* 卡片保持「自然高度」，不再被压到内部裁切；真放不下时由整页滚动兜底。
       这是自适应的：内容装得下就完全不滚，装不下才滚。 */
    flex: 0 0 auto;
    min-height: 0;
    overflow: visible;
  }
  /* 英雄卡保留自己的装饰裁剪（要用 .home > .hero 提特异性，见上）。 */
  .home > .hero {
    overflow: hidden;
  }
  /* 空状态的间距按**视口高度**伸缩，而不是写死像素：
     360px 高的机器自动收紧，高屏取上限（等于原来的观感）。 */
  .empty {
    padding: clamp(8px, calc(2.6vh / var(--ui-scale, 1)), 20px) 16px;
    gap: clamp(6px, calc(2vh / var(--ui-scale, 1)), 14px);
  }
}

/* ── 手机首页的最终排布：竖排 + 主次分明 ────────────────────────────────
   曾经为了塞进 240px 高度用过「英雄卡 | 实例卡」左右两栏 —— 那是针对
   桌面式大英雄卡的权宜之计。现在英雄卡已经压成一行问候条（见上），
   竖排完全放得下，而且更符合手机习惯：**主操作在上，次要的在下面**。

   顺序：① 问候条 → ② 当前实例（含通栏「启动游戏」）→ ③ 固定项横滑条。
   固定项从「顶部网格」挪到最下面：手机首屏应该先给「开始玩」，
   而不是一堆快捷方式。 */
@media (max-width: 1100px), (pointer: coarse) {
  .home {
    flex-direction: column;
    gap: 10px;
  }
  .home > .hero {
    order: 1;
  }
  .home > section {
    order: 2;
  }
  .home > .pin-block {
    order: 3;
  }
  /* 一律按内容高度占位：谁也不用被拉伸或压缩，放不下就整页滚 */
  .home > .hero,
  .home > section,
  .home > .pin-block {
    flex: 0 0 auto;
  }
  /* 基础样式里 `.section { margin-top: auto }` 是「把卡片推到底部」的老规则，
     竖排时会让实例卡与问候条之间裂开一大段空白，必须清零。 */
  .home > section {
    margin-top: 0;
  }
  /* 选实例的底部抽屉：内容区自己滚，标题栏固定 */
  .pick-scroll {
    max-height: 62vh;
  }
  /* 内容不足一屏时居中，别全堆在顶上（safe 保证溢出时不裁掉顶部）。
     注意**不要**在这里按 vh 放大卡片/字号 —— 界面缩放调小时布局视口高度会变大，
     那样写会让首页文字在低缩放下反而更大（其它页面都跟着缩小，只有首页不变，
     非常突兀）。项目的其它尺寸都是固定 px、统一跟随 zoom，首页也照此办理。 */
  .home {
    justify-content: safe center;
  }
}

/* ── 宽视图（≥720 CSS px 且横屏）：欢迎卡 + 内容分左右两栏 ──────────────────
   什么时候会出现「宽视图」：
     · 手机**横屏** —— 安卓端主页要求与桌面端同款双栏布局
       （用户 2026-09-22：照桌面截图改；原来 ≥1150px 的断点横屏手机永远够不到）；
     · 界面缩放调到 100% 以下 —— 布局视口会成比例变大
       （实测 60% 时 853px 的屏幕变成 1421 CSS px）；
     · 平板 / 外接屏 / 桌面端窗口拉大。
   竖屏手机（宽度 <720px）仍走上面的竖排规则。游戏统计卡锚在英雄卡左下角。
   注意这个块放在最后：粗指针设备即使视口变宽也会同时命中上面的手机规则，
   靠「后来居上」覆盖掉竖排相关的属性（display:grid 下那些 flex 属性本来就无效）。 */
@media (min-width: 720px) and (orientation: landscape) {
  .home {
    display: grid;
    /* 左栏欢迎卡、右栏内容；右栏给更多空间（实例卡 + 固定项都在右边）。
       行高用 auto（内容自然高）而不是 1fr 撑满：固定高度遇上界面缩放
       会把英雄卡里塞不下的重要内容裁掉（用户「调缩放就露馅」），自然高
       放不下时由 .home 的整页滚动兜底，任何缩放下都完整。 */
    grid-template-columns: minmax(300px, 0.9fr) minmax(0, 1.6fr);
    grid-template-rows: auto auto;
    align-content: start;
    gap: 12px;
  }
  /* 左栏整列都是欢迎卡：竖过来（图标在上、文案与动作在下）并整体居中，
     当作「欢迎面板」用。游戏统计卡用 margin-top:auto 推到卡片底部左侧
     （绝对定位会跟居中的按钮叠在一起，实测踩过）。 */
  .home > .hero {
    grid-column: 1;
    grid-row: 1 / span 2;
    align-self: stretch;
    flex-direction: column;
    justify-content: flex-start;
    align-items: center;
    gap: 6px;
    padding: 12px 14px;
  }
  .home > .hero .hero-text {
    align-items: center;
    text-align: center;
    flex: 0 0 auto;
  }
  /* 桌面截图同款：副标题在横屏双栏下显示（手机竖排规则里它是 display:none）。
     一行放不下就省略号，别折两行把卡片顶爆。 */
  .home > .hero .hero-text p {
    display: block;
    font-size: 12px;
    margin: 0 0 8px;
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hero-logo-img {
    width: 48px;
    height: 48px;
    border-radius: 12px;
  }
  .home > .hero .hero-actions .btn {
    min-height: 38px;
    padding: 7px 12px;
    font-size: 12px;
  }
  /* 游戏统计：放在 hero-text 内部 → 宽度天然与上面按钮行对齐；
     margin-top:auto 在右栏更高时把它推到卡片左下角。可点击。 */
  .home > .hero .hero-playtime {
    display: block;
    margin-top: auto;
    padding-top: 10px;
    width: 100%;
    cursor: pointer;
    zoom: 0.8;
  }
  .home > .hero .hero-playtime :deep(.pt-card) {
    padding: 10px 12px;
    transition: border-color 0.15s;
  }
  .home > .hero .hero-playtime:hover :deep(.pt-card) {
    border-color: var(--accent-45);
  }
  /* 英雄卡高度由右栏内容决定（实测 263px），塞不下 30 天柱状图（实测溢出 51px）
     —— 首页这个紧凑位只显示「累计游玩 + Top 实例」，完整图表仍在实例/设置页 */
  .home > .hero .hero-playtime :deep(.pt-chart) {
    display: none;
  }
  .home > section {
    grid-column: 2;
    grid-row: 1;
    margin-top: 0;
    overflow: visible;
  }
  /* 右栏当前实例卡：压成单行 —— 图标 + （名称/版本两行）在左，
     切换/启动两个按钮跟在右边（手机竖排规则折成的两行通栏按钮在这里改回单行），
     卡片更矮，下面能多放固定到首页的实例。 */
  .home .resident {
    flex-wrap: nowrap;
    padding: 14px 16px;
    gap: 12px;
  }
  .home .resident-actions {
    width: auto;
    flex: 0 0 auto;
  }
  .home .resident-actions .btn {
    flex: 0 0 auto;
    min-height: 44px;
  }
  .home > .pin-block {
    grid-column: 2;
    grid-row: 2;
    margin-top: 0;
    overflow-y: auto;
    min-height: 0;
  }
}
</style>
