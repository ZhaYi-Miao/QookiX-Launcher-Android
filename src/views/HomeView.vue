<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useInstancesStore } from "../stores/instances";
import { useAccountsStore } from "../stores/accounts";
import { usePinsStore } from "../stores/pins";
import { useTasksStore } from "../stores/tasks";
import { useSettingsStore } from "../stores/settings";

import { useMessage } from "../composables/message";
import { api } from "../api";
import type { NewsItem } from "../types";
import { Button as VanButton, Progress as VanProgress, Swipe as VanSwipe, SwipeItem as VanSwipeItem } from "vant";
import { loaderBadge, fmtSpeed } from "../utils/format";
import { taskPercent } from "../utils/task";
import AppIcon from "../components/AppIcon.vue";
import AppPopup from "../ui/AppPopup.vue";
import InstanceCard from "../components/InstanceCard.vue";
import PlaytimeCard from "../components/PlaytimeCard.vue";
import { IconPlay, IconRepeat, IconPlus } from "../components/icons";



const router = useRouter();
const message = useMessage();
const instances = useInstancesStore();
const accounts = useAccountsStore();
const pins = usePinsStore();
const tasks = useTasksStore();
const settings = useSettingsStore();

/* 三个分区各自有个总开关（设置 → 首页）；默认都开，关掉就彻底不显示 */
const showDownloads = computed(() => settings.settings?.show_home_downloads ?? true);
const showRecent = computed(() => settings.settings?.show_home_recent ?? true);
const showStats = computed(() => settings.settings?.show_home_stats ?? true);

// ── 新闻：首页主卡片下面原来一大片空白，把最新几条放进来（点整行进 /news）──
const news = ref<NewsItem[]>([]);
const showNews = computed(() => settings.settings?.show_news !== false);
async function loadNews() {
  if (!showNews.value) return;
  try {
    news.value = (await api.fetchNews()) ?? [];
  } catch {
    // 新闻拉不到不该影响首页（网络差 / 服务挂了时静默留空）
    news.value = [];
  }
}
onMounted(loadNews);

/**
 * 页面右上动作已废弃：那个位置的按钮会在切页时出现/消失，把账号块挤得左右跳。
 * 现在「切换实例」由首页卡片上的切换按钮 + 内容里的按钮承担（都走下面的切换面板）。
 */

const STORAGE_KEY = "qookix.home.selected";
const selectedId = ref<string | null>(localStorage.getItem(STORAGE_KEY));
const launching = ref(false);
const showPicker = ref(false);

const selected = computed(() => (selectedId.value ? instances.get(selectedId.value) : null) ?? instances.instances[0] ?? null);
watch(selected, (v) => { if (v) localStorage.setItem(STORAGE_KEY, v.id); });

/** 选一个实例作为首页主卡片（切换实例面板 / 卡片上的切换按钮都走它） */
function pickInstance(id: string) {
  selectedId.value = id;
  showPicker.value = false;
}

/** 固定到首页的快捷项（实例被删后自动消失） */
const pinsHere = computed(() => pins.items.filter((p) => p.target === "home" && instances.get(p.instanceId)));

/** 正在下载/安装的任务：有才显示卡片（数据一直在 tasks store 里收，之前首页没渲染） */
const activeTasks = computed(() => tasks.taskList.filter((t) => !t.finished));

/**
 * 最近玩过的实例：上面那张主卡片已经展示了当前选中的实例，这里排除掉避免重复，
 * 按 last_played 倒序取前 4 个（没有游玩记录的实例不出现）。
 */
const recentInstances = computed(() =>
  instances.instances
    .filter((i) => !!i.last_played && i.id !== selected.value?.id)
    .sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0))
    .slice(0, 4)
);

const greeting = computed(() => {
  const h = new Date().getHours();
  if (h < 5) return $t("home.ye-shen-le");
  if (h < 11) return $t("home.computed");
  if (h < 13) return $t("home.greeting");
  if (h < 18) return $t("home.xia-wu-hao");
  if (h < 22) return $t("home.wan-shang-hao");
  return $t("home.ye-shen-le");
});

async function launch(instId: string, world?: string, address?: string) {
  if (!accounts.accounts.length) {
    message.warning($t("home.add-account-hint"));
    accounts.showManager = true;
    return;
  }
  launching.value = true;
  try {
    const res = await instances.launch(instId, world, address);
    if (res) message.success($t("home.launched", { p1: instances.get(instId)?.name ?? "" }));
  } catch (e) {
    message.error(String(e));
  } finally {
    launching.value = false;
  }
}

onMounted(() => {
  // 拉一次实例列表（切换面板要列全部实例；原来只靠 pageAction 按钮，列表可能是旧的）
  void instances.refresh();
});
</script>

<template>
  <div class="home">
    <p class="greet">{{ greeting }}</p>

    <!-- 切换实例面板：原来只把 showPicker 置 true，模板里没有任何东西绑定它，
         所以右上角「切换实例」和卡片上的切换按钮点了都没反应。 -->
    <app-popup
      :show="showPicker"
      position="bottom"
      round
      @update:show="(v: boolean) => (showPicker = v)"
    >
      <div v-if="instances.instances.length" class="picker">
        <div class="picker-title">{{ $t("home.switch-instance") }}</div>
        <button
          v-for="inst in instances.instances"
          :key="inst.id"
          class="picker-row"
          :class="{ on: inst.id === selected?.id }"
          @click="pickInstance(inst.id)"
        >
          <AppIcon :name="inst.icon" class="picker-icon" />
          <span class="picker-name">{{ inst.name }}</span>
          <span class="picker-meta">{{ inst.mc_version }}</span>
        </button>
      </div>
    </app-popup>

    <template v-if="instances.instances.length">
      <section class="hero glass">
        <div class="hero-icon"><AppIcon :name="selected?.icon" /></div>
        <div class="hero-main">
          <div class="hero-name">{{ selected?.name }}</div>
          <div class="hero-meta">
            <span class="badge">{{ loaderBadge(selected?.loader) }}</span>
            <span>{{ selected?.mc_version }}</span>
          </div>
        </div>
        <van-button type="primary" class="launch" :loading="launching" @click="launch(selected!.id)">
          <IconPlay /> {{ $t("instance-card.launch") }}
        </van-button>
        <button class="switch" @click="showPicker = true"><IconRepeat /></button>
      </section>

      <!-- 正在下载：有任务才出现（整卡可点，进下载页看明细） -->
      <section v-if="showDownloads && activeTasks.length" class="dl glass" @click="router.push('/downloads')">
        <div class="dl-top">
          <span class="dl-title">{{ $t("nav.downloading", { count: activeTasks.length }) }}</span>
          <span v-if="activeTasks[0].speed > 0" class="dl-speed">{{ fmtSpeed(activeTasks[0].speed) }}</span>
        </div>
        <div class="dl-name text-ellipsis">{{ activeTasks[0].source ?? activeTasks[0].message }}</div>
        <van-progress :percentage="taskPercent(activeTasks[0])" :show-pivot="false" />
      </section>

      <!-- 最近游玩：复用实例卡片；当前实例已在上面那张主卡片里，这里排除掉 -->
      <section v-if="showRecent && recentInstances.length">
        <h2 class="sec">{{ $t("home.recent-played") }}</h2>
        <div class="grid">
          <InstanceCard v-for="i in recentInstances" :key="i.id" :instance="i" :movable="false" />
        </div>
      </section>

      <section v-if="pinsHere.length" class="pins">
        <h2 class="sec">{{ $t("nav.pinned") }}</h2>
        <van-swipe :loop="false" :show-indicators="false" class="pin-swipe">
          <van-swipe-item v-for="p in pinsHere" :key="p.id">
            <div class="pin glass" @click="router.push(`/instance/${p.instanceId}`)">
              <div class="pin-icon"><AppIcon :name="p.instanceIcon" /></div>
              <div class="pin-txt">
                <div class="pin-name">{{ p.name }}</div>
                <div class="pin-sub">{{ p.instanceName }}</div>
              </div>
              <van-button size="small" type="primary" @click.stop="launch(p.instanceId, p.world, p.address)">
                <IconPlay />
              </van-button>
            </div>
          </van-swipe-item>
        </van-swipe>
      </section>

      <!-- 新闻：主卡片与「最近游玩」下面原来一大片空白，这里填最新几条（点进新闻页看全文） -->
      <section v-if="showNews && news.length">
        <h2 class="sec">{{ $t("nav.news") }}</h2>
        <div class="news-list">
          <button
            v-for="n in news.slice(0, 3)"
            :key="n.title"
            class="news-row glass"
            @click="router.push('/news')"
          >
            {{ n.title }}
          </button>
        </div>
      </section>

      <!-- 游玩统计：没有游玩记录时组件自身不渲染 -->
      <PlaytimeCard v-if="showStats" />
    </template>

    <div v-else class="empty glass">
      <p>{{ $t("home.no-instances") }}</p>
      <van-button type="primary" @click="router.push('/instances')">
        <IconPlus /> {{ $t("home.create-first") }}
      </van-button>
    </div>
  </div>
</template>

<style scoped>
/* 首页新闻：一屏内三条，点整行进新闻页（页面底部不再空一大片） */
.news-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.news-row {
  width: 100%;
  min-height: 48px;
  padding: 12px 14px;
  text-align: left;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
  font-size: 14px;
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.home {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 4px 16px 16px;
}
.greet {
  margin: 0;
  font-size: 13px;
  color: var(--text-3);
}
.hero {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  /* 允许换行：150% 缩放下可用宽度只有 ~256px，图标+信息+启动+切换放不下一行，
     原来的 fixed 单行会把信息列挤到 ~30px，meta 直接**画到启动键上**
     （cdp_scan 在 150% 下实测到重叠）。窄了就换行，不靠「刚好放得下」。 */
  flex-wrap: wrap;
}
.hero-icon {
  width: 56px;
  height: 56px;
  border-radius: 14px;
  overflow: hidden;
  background: linear-gradient(135deg, var(--accent-25), var(--accent-08));
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--accent);
  flex-shrink: 0;
}
.hero-main {
  /* 不能是裸的 `flex: 1`（= 1 1 0%）：那样它会被压到 0 宽，内容溢出盖到按钮上。
     给一个真实的最小基准，空间不足时由外层 flex-wrap 换行解决。 */
  flex: 1 1 140px;
  min-width: 0;
}
.hero-name {
  font-size: 17px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.hero-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-3);
  /* 版本 + 加载器 + 上次游玩三截字，窄屏自己折行，别横向溢出容器 */
  flex-wrap: wrap;
}
.badge {
  background: var(--accent-16);
  color: var(--accent);
  border-radius: 6px;
  padding: 1px 7px;
  font-weight: 600;
}
.launch {
  flex-shrink: 0;
  min-height: 44px;
}
.switch {
  flex-shrink: 0;
  width: 44px;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--panel);
  color: var(--text-2);
  display: flex;
  align-items: center;
  justify-content: center;
}
.sec {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-2);
}
.pin {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px;
}
.pin-icon {
  width: 42px;
  height: 42px;
  border-radius: 12px;
  overflow: hidden;
  background: var(--accent-10);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--accent);
  flex-shrink: 0;
}
.pin-txt {
  flex: 1;
  min-width: 0;
}
.pin-name {
  font-weight: 600;
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pin-sub {
  font-size: 12px;
  color: var(--text-3);
}
.empty {
  padding: 32px 16px;
  text-align: center;
  color: var(--text-3);
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: center;
}
/* ── 切换实例面板（手机底部弹层）── */
.picker {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px));
  max-height: 70vh;
  overflow-y: auto;
}
.picker-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--text-1);
  margin-bottom: 2px;
}
.picker-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 52px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
  font-size: 15px;
  text-align: left;
}
.picker-row.on {
  border-color: var(--accent);
  color: var(--accent);
}
.picker-icon {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
}
.picker-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.picker-meta {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-3);
}
/* ── 正在下载卡片 ── */
.dl {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 14px;
}
.dl-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.dl-title {
  font-size: 14px;
  font-weight: 600;
}
.dl-speed {
  font-size: 12px;
  color: var(--text-3);
}
.dl-name {
  font-size: 12px;
  color: var(--text-3);
}
/* ── 最近游玩：与实例页同一套网格（160px 起，窄屏自动落成 1~2 列） ── */
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(160px, 100%), 1fr));
  gap: 10px;
  grid-auto-rows: max-content;
}
</style>
