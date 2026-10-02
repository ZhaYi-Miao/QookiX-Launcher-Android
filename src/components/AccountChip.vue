<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, ref } from "vue";
import { useMessage } from "naive-ui";
import { Button as VanButton } from "vant";
import { useAccountsStore } from "../stores/accounts";
import AppPopup from "../ui/AppPopup.vue";
import AppInput from "../ui/AppInput.vue";
import MsLoginDialog from "./MsLoginDialog.vue";

/** 顶栏的账号入口（手机形态）：一行头像+用户名，点开是**底部弹层**的账号列表
 *  —— 桌面的下拉气泡（n-popover）在手机上既难点又贴不住指头。 */
const accounts = useAccountsStore();
const message = useMessage();
const show = ref(false);
const offlineName = ref("");
const adding = ref(false);


const current = computed(() => accounts.current);
const AVATAR_SOURCES = [
  (u: string) => `https://mc-heads.net/avatar/${u}/64`,
  (u: string) => `https://crafatar.com/avatars/${u}?size=64&overlay`,
  (u: string) => `https://minotar.net/helm/${u}/64`,
];
const attempt = ref(0);

/** 头像 URL：多源依次回退（@error 时 attempt++） */
const avatarUrl = computed(() => {
  const u = current.value?.uuid;
  if (!u || current.value?.type !== "microsoft" || attempt.value >= AVATAR_SOURCES.length) return null;
  return AVATAR_SOURCES[attempt.value](u);
});

/** 离线账号显示名字首字（没有正版头像） */
const initial = computed(() => (current.value?.username ?? "").slice(0, 1).toUpperCase());

/** 切号：手机上切完就关弹层，不要留一层遮罩让人再点一次 */
async function pick(uuid: string) {
  try {
    await accounts.select(uuid);
    show.value = false;
  } catch (e) {
    message.error(String(e));
  }
}

async function addOffline() {
  if (!offlineName.value.trim()) return;
  adding.value = true;
  try {
    await accounts.addOffline(offlineName.value.trim());
    offlineName.value = "";
    message.success($t("account-chip.add"));
  } catch (e) {
    message.error(String(e));
  } finally {
    adding.value = false;
  }
}
</script>

<template>
  <div class="chip" @click="show = true">
    <span class="av">
      <img v-if="avatarUrl" :src="avatarUrl" alt="" @error="attempt++" />
      <span v-else class="ini">{{ initial }}</span>
    </span>
    <span class="nm">{{ current?.username ?? $t("account-chip.not-logged-in") }}</span>
  </div>
  <app-popup :show="show" position="bottom" round @update:show="(v: boolean) => (show = v)">
    <div class="sheet">
      <div class="stitle">{{ $t("account-chip.current-account") }}</div>
      <button
        v-for="a in accounts.accounts"
        :key="a.uuid"
        class="row"
        :class="{ on: current?.uuid === a.uuid }"
        @click="pick(a.uuid)"
      >
        <span class="rname">{{ a.username }}</span>
        <span class="rtype">{{ a.type === "microsoft" ? "正版" : "离线" }}</span>
      </button>
      <p v-if="!accounts.accounts.length" class="empty">{{ $t("account-chip.no-accounts") }}</p>
      <div class="add">
        <app-input v-model:value="offlineName" :placeholder="$t('account-chip.game-username')" maxlength="16" />
        <van-button type="primary" :loading="adding" @click="addOffline">{{ $t("account-chip.add") }}</van-button>
      </div>
      <van-button block @click="accounts.showManager = true; show = false">{{ $t("home.switch-account") }}</van-button>
    </div>
  </app-popup>
  <MsLoginDialog />
</template>

<style scoped>
.chip {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  padding: 4px 12px 4px 4px;
  border-radius: 20px;
  border: 1px solid var(--border);
  background: var(--panel);
  cursor: pointer;
  max-width: 190px;
}
.av {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
  background: var(--accent-16);
  display: flex;
  align-items: center;
  justify-content: center;
}
.av img {
  width: 100%;
  height: 100%;
}
.ini {
  color: var(--accent);
  font-weight: 700;
  font-size: 13px;
}
.nm {
  font-size: 13px;
  color: var(--text-2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  color: var(--text-2);
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 48px;
  padding: 0 14px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
  font-size: 15px;
  text-align: left;
}
.row.on {
  border-color: var(--accent);
  color: var(--accent);
}
.rname {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rtype {
  font-size: 12px;
  color: var(--text-3);
}
.empty {
  text-align: center;
  color: var(--text-3);
  padding: 16px 0;
}
.add {
  display: flex;
  gap: 8px;
  align-items: center;
}
.add > :first-child {
  flex: 1;
  min-width: 0;
}
</style>
