<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useMessage } from "../composables/message";
import { Button as VanButton } from "vant";
import { useInstancesStore } from "../stores/instances";
import { useAccountsStore } from "../stores/accounts";
import { loaderBadge } from "../utils/format";
import AppIcon from "./AppIcon.vue";
import AppPopup from "../ui/AppPopup.vue";
import { IconPlay, IconMoreVertical } from "./icons";
import type { Instance } from "../types";

/** 手机形态的实例卡：竖版（图标/名称/版本在上，启动键通栏在下），
 *  其余操作收进「更多」底部弹层 —— 桌面的悬停菜单、右键菜单在手机上都没法用。 */
const props = defineProps<{ instance: Instance }>();
const emit = defineEmits<{ move: [instance: Instance] }>();

const router = useRouter();
const message = useMessage();
const instances = useInstancesStore();
const accounts = useAccountsStore();
const launching = ref(false);
const showMore = ref(false);

const lastPlayed = computed(() =>
  props.instance.last_played ? new Date(props.instance.last_played * 1000).toLocaleDateString() : ""
);

async function launch() {
  if (!accounts.accounts.length) {
    message.warning($t("home.add-account-hint"));
    accounts.showManager = true;
    return;
  }
  launching.value = true;
  try {
    const res = await instances.launch(props.instance.id);
    if (res) message.success($t("home.launched", { p1: props.instance.name }));
  } catch (e) {
    message.error(String(e));
  } finally {
    launching.value = false;
  }
}

function openDetail() {
  showMore.value = false;
  router.push(`/instance/${props.instance.id}`);
}

function move() {
  showMore.value = false;
  emit("move", props.instance);
}

async function remove() {
  showMore.value = false;
  try {
    await instances.remove(props.instance.id);
    message.success($t("instance-card.delete-instance"));
  } catch (e) {
    message.error(String(e));
  }
}
</script>

<template>
  <div class="card glass" @click="openDetail">
    <button class="more" :aria-label="$t('common.delete')" @click.stop="showMore = true"><IconMoreVertical /></button>
    <div class="ic"><AppIcon :name="instance.icon" /></div>
    <div class="nm">{{ instance.name }}</div>
    <div class="mt">
      <span class="badge">{{ loaderBadge(instance.loader) }}</span>
      <span class="ver">{{ instance.mc_version }}</span>
    </div>
    <div v-if="lastPlayed" class="lp">{{ lastPlayed }}</div>
    <van-button class="go" block type="primary" :loading="launching" @click.stop="launch">
      <IconPlay /> {{ $t("instance-card.launch") }}
    </van-button>
    <app-popup :show="showMore" position="bottom" round @update:show="(v: boolean) => (showMore = v)">
      <div class="sheet">
        <div class="stitle">{{ instance.name }}</div>
        <button class="act" @click="openDetail">{{ $t("instance-detail.files") }}</button>
        <button class="act" @click="move">{{ $t("instance-card.move-to-group") }}</button>
        <button class="act danger" @click="remove">{{ $t("instance-card.delete-instance") }}</button>
      </div>
    </app-popup>
  </div>
</template>

<style scoped>
.card {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 12px 10px 10px;
  cursor: pointer;
}
.more {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-3);
}
.ic {
  width: 56px;
  height: 56px;
  border-radius: 16px;
  overflow: hidden;
  background: linear-gradient(135deg, var(--accent-25), var(--accent-08));
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--accent);
}
.nm {
  font-size: 14px;
  font-weight: 700;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mt {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--text-3);
}
.badge {
  background: var(--accent-16);
  color: var(--accent);
  border-radius: 5px;
  padding: 1px 6px;
  font-weight: 600;
}
.lp {
  font-size: 10px;
  color: var(--text-3);
}
.go {
  margin-top: 4px;
  min-height: 38px;
}
.sheet {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px));
}
.stitle {
  font-size: 15px;
  font-weight: 700;
  margin-bottom: 4px;
}
.act {
  min-height: 46px;
  padding: 0 14px;
  text-align: left;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
  font-size: 15px;
}
.act.danger {
  color: #e5534b;
  border-color: rgba(229, 83, 75, 0.4);
}
</style>
