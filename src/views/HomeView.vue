<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useInstancesStore } from "../stores/instances";
import { useAccountsStore } from "../stores/accounts";
import { usePinsStore } from "../stores/pins";

import { useMessage } from "../composables/message";
import { Button as VanButton, Swipe as VanSwipe, SwipeItem as VanSwipeItem } from "vant";
import { loaderBadge } from "../utils/format";
import AppIcon from "../components/AppIcon.vue";
import AppPopup from "../ui/AppPopup.vue";
import { IconPlay, IconRepeat, IconPlus } from "../components/icons";



const router = useRouter();
const message = useMessage();
const instances = useInstancesStore();
const accounts = useAccountsStore();
const pins = usePinsStore();

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
</style>
