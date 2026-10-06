<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, ref, watch } from "vue";
import { useMessage } from "../composables/message";
import { Button as VanButton } from "vant";
import { useAccountsStore } from "../stores/accounts";
import AppPopup from "../ui/AppPopup.vue";
import AppInput from "../ui/AppInput.vue";
import MsLoginDialog from "./MsLoginDialog.vue";
import { IconPlus, IconTrash } from "./icons";

/** 顶栏的账号入口（手机形态）：一行头像+用户名，点开是**底部弹层**的账号列表
 *  —— 桌面的下拉气泡（n-popover）在手机上既难点又贴不住指头。 */
const accounts = useAccountsStore();
const message = useMessage();
const show = ref(false);
const offlineName = ref("");
const adding = ref(false);


const current = computed(() => accounts.current);
/**
 * 头像源，按真机实测可用性排序：
 * crafatar 已长期 500 挂掉，但会给 WebView 回一张 **200 的占位图** —— 占位图不触发 @error，
 * 整条回退链直接失效（用户看到的就是那张默认 Alex）；mc-heads 对非浏览器 UA 返 403，放兜底；
 * 只有 minotar 稳定，排第一。
 */
const AVATAR_SOURCES = [
  (u: string) => `https://minotar.net/helm/${u}/64`,
  (u: string) => `https://mc-heads.net/avatar/${u}/64`,
];

/** 顶栏当前账号头像：与列表共用同一套多源回退；离线账号没有正版头像 → 显示首字 */
const avatarUrl = computed(() => {
  const c = current.value;
  return c ? avatarFor(c.uuid, c.type) : null;
});

/** 离线账号显示名字首字（没有正版头像） */
const initial = computed(() => initialOf(current.value?.username ?? ""));

/**
 * 面板里**每个账号**的头像。
 * 原来只有顶栏当前账号有头像，列表里只有名字（用户反馈「把头像砍掉了」）。
 * 离线账号没有正版头像，显示名字首字；正版账号按 uuid 取头像，多源依次回退。
 * 带 `v=avatarVersion`：换肤后 URL 变化，绕过 WebView 缓存，否则换了皮头像也不变。
 */
const rowAvatar = ref<Record<string, number>>({});
function avatarFor(uuid: string, type: string): string | null {
  if (type !== "microsoft") return null;
  const n = rowAvatar.value[uuid] ?? 0;
  if (n >= AVATAR_SOURCES.length) return null;
  const base = AVATAR_SOURCES[n](uuid);
  return `${base}${base.includes("?") ? "&" : "?"}v=${accounts.avatarVersion}`;
}
function onRowAvatarError(uuid: string) {
  rowAvatar.value = { ...rowAvatar.value, [uuid]: (rowAvatar.value[uuid] ?? 0) + 1 };
}
function initialOf(name: string): string {
  return (name ?? "").slice(0, 1).toUpperCase();
}

/**
 * 删除账号。
 * store 里早就有 `remove()`，但**全项目没有任何地方调用它** —— 面板上也没有删除入口，
 * 所以账号删不掉（用户反馈）。这里补上：整行点击仍是「切换」，删除是右侧那颗独立的按钮。
 */
const removing = ref("");
async function removeAccount(uuid: string) {
  if (removing.value) return;
  removing.value = uuid;
  try {
    await accounts.remove(uuid);
    message.success($t("account-chip.account-removed"));
  } catch (e) {
    message.error(String(e));
  } finally {
    removing.value = "";
  }
}

/** 切号：手机上切完就关弹层，不要留一层遮罩让人再点一次 */
async function pick(uuid: string) {
  try {
    await accounts.select(uuid);
    show.value = false;
  } catch (e) {
    message.error(String(e));
  }
}

/**
 * 「先添加账号」这类流程（首页/实例卡的启动、存档页联机）会置 `accounts.showManager = true`。
 * 前端重写时这个监听被删掉了，而 `showManager` 的赋值还在 —— 于是那些地方点了完全没反应。
 */
watch(
  () => accounts.showManager,
  (v) => {
    if (v) show.value = true;
  }
);
watch(show, (v) => {
  if (!v) accounts.showManager = false;
});

/** 微软登录成功时给一句反馈 —— store 里 `msSuccess` 一直没人展示，登录完静悄悄的 */
watch(
  () => accounts.msSuccess,
  (v) => {
    if (!v) return;
    message.success(v);
    accounts.msSuccess = "";
  }
);

/**
 * 微软（正版）登录：设备码流程。
 * 先收起账号面板再发码 —— 登录弹窗由 `accounts.msFlow` 驱动会自动打开，
 * 两层弹层叠着不好看。
 */
async function startMs() {
  show.value = false;
  try {
    await accounts.startMs();
  } catch (e) {
    message.error(String(e));
  }
}

function onChipTap() {
  show.value = true;
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
  <!-- 单根包裹：这一层是多根 fragment → 单根的转换，本身不产生盒子（display: contents），
       布局与「.chip 直接作为 .top 的 flex item」完全一致。 -->
  <div class="acct-wrap">
  <div class="chip" @click="onChipTap">
    <span class="av">
      <img v-if="avatarUrl" :src="avatarUrl" alt="" @error="onRowAvatarError(current?.uuid ?? '')" />
      <span v-else class="ini">{{ initial }}</span>
    </span>
    <span class="nm">{{ current?.username ?? $t("account-chip.not-logged-in") }}</span>
  </div>
  <app-popup :show="show" position="bottom" round :lazy-render="false" @update:show="(v: boolean) => (show = v)">
    <div class="sheet">
      <div class="stitle">{{ $t("account-chip.current-account") }}</div>
      <div
        v-for="a in accounts.accounts"
        :key="a.uuid"
        class="row"
        :class="{ on: current?.uuid === a.uuid }"
      >
        <button class="row-main" @click="pick(a.uuid)">
          <span class="rav">
            <img
              v-if="avatarFor(a.uuid, a.type)"
              :src="avatarFor(a.uuid, a.type)!"
              alt=""
              @error="onRowAvatarError(a.uuid)"
            />
            <span v-else class="rini">{{ initialOf(a.username) }}</span>
          </span>
          <span class="rname">{{ a.username }}</span>
          <span class="rtype">{{ a.type === "microsoft" ? $t("account-chip.premium") : $t("account-chip.offline") }}</span>
        </button>
        <!-- 删除账号：整行点击是「切换」，删除必须是独立按钮，否则误触 -->
        <button
          class="row-del"
          :disabled="removing === a.uuid"
          :aria-label="$t('account-chip.remove-account')"
          @click="removeAccount(a.uuid)"
        >
          <IconTrash />
        </button>
      </div>
      <p v-if="!accounts.accounts.length" class="empty">{{ $t("account-chip.no-accounts") }}</p>

      <!-- 微软（正版）登录：设备码流程，点完弹窗由 accounts.msFlow 驱动自己出来。
           之前重构把入口删了，整个应用就没法加正版账号了。 -->
      <van-button class="add-ms" block type="primary" @click="startMs">
        <IconPlus /> {{ $t("account-chip.add-microsoft") }}
      </van-button>

      <div class="add-title">{{ $t("account-chip.add-offline") }}</div>
      <!-- 手机：输入框与「添加」按钮必须同高（原来按钮矮一截，看着很别扭） -->
      <div class="add">
        <app-input v-model:value="offlineName" :placeholder="$t('account-chip.game-username')" maxlength="16" />
        <van-button class="add-btn" type="primary" :loading="adding" @click="addOffline">{{ $t("account-chip.add") }}</van-button>
      </div>
    </div>
  </app-popup>
  <MsLoginDialog />
  </div>
</template>

<style scoped>
.acct-wrap {
  display: contents;
}
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
  gap: 4px;
  min-height: 48px;
  padding: 0 6px 0 0;
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
/* 行内「切换」按钮：占满除删除键以外的部分，整行都可点（手机上好按） */
.row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 46px;
  padding: 0 8px 0 12px;
  border: none;
  background: transparent;
  color: inherit;
  font-family: inherit;
  font-size: 15px;
  text-align: left;
}
/* 列表里的账号头像（原来只有顶栏当前账号有） */
.rav {
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
.rav img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.rini {
  font-size: 13px;
  font-weight: 700;
  color: var(--accent);
}
/* 删除账号：独立按钮，避免和「切换」互相误触 */
.row-del {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: #e5534b;
}
.row-del:disabled {
  opacity: 0.45;
}
/* 输入框与「添加」按钮同高 */
.add :deep(.van-field__control),
.add-btn {
  min-height: 44px;
}
/* 微软登录入口：放在离线那排上方，是「加正版账号」的唯一入口 */
.add-ms {
  min-height: 44px;
}
.add-title {
  font-size: 12px;
  color: var(--text-3);
  margin-top: 2px;
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
