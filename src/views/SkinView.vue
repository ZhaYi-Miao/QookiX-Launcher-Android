<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useMessage } from "../composables/message";
import { useSkinRenderer, type AnimationKind } from "../composables/useSkinRenderer";
import { useAccountsStore } from "../stores/accounts";
import { api } from "../api";
import { Button as VanButton, Empty as VanEmpty, Search as VanSearch } from "vant";
import { pickFile } from "../composables/filePicker";
import { convertFileSrc } from "@tauri-apps/api/core";
/* 顶栏右上动作已废弃（那个位置切页时会跳变），上传皮肤按钮在页面内容里已有 */

const accounts = useAccountsStore();
const message = useMessage();
const canvas = ref<HTMLCanvasElement | null>(null);
const renderer = useSkinRenderer(canvas);



const skins = ref<{ name: string; filename: string }[]>([]);
const thumbs = ref<Record<string, string>>({});
const model = ref<"default" | "slim">("default");
const anim = ref<AnimationKind>("idle");
const playerInput = ref("");
const busy = ref(false);

/**
 * 当前账号 = 「正在游玩的账号」（store 的 `current`：先看 settings.selected_account，再退回第一个）。
 * 以前这里取的是 `accounts.accounts[0]` —— 多账号时皮肤中心显示/应用在**错的账号**上。
 */
const currentAccount = computed(() => accounts.current);
const currentSrc = ref<string | null>(null);

async function loadSaved() {
  try {
    skins.value = await api.listSkins();
    for (const s of skins.value) {
      if (!thumbs.value[s.filename]) thumbs.value[s.filename] = await api.readSkinDataUrl(s.filename);
    }
  } catch (e) {
    message.error(String(e));
  }
}

async function preview(src: string | null) {
  currentSrc.value = src;
  await renderer.loadSkinFromSrc(src, model.value === "slim" ? "slim" : "default");
}

async function apply() {
  if (!currentSrc.value || !currentAccount.value) return;
  busy.value = true;
  try {
    if (currentAccount.value.type === "offline") {
      await api.applySkinOffline(currentSrc.value, model.value, currentAccount.value.uuid);
    } else {
      await api.applySkinToAccount(currentAccount.value.uuid, currentSrc.value, model.value);
    }
    /* 换肤后头像 URL 要变（avatarFor 带 v=avatarVersion），否则缓存住的旧头像不会更新 */
    accounts.bumpAvatar();
    message.success($t("common.done"));
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = false;
  }
}

async function fetchPlayer() {
  if (!playerInput.value.trim()) return;
  busy.value = true;
  try {
    const r = await api.fetchPlayerSkin(playerInput.value.trim());
    const name = playerInput.value.trim();
    await api.saveSkinFromData(name, r.data_url);
    await loadSaved();
    await preview(r.data_url);
    message.success($t("skins.skin-saved-to", { p1: name }));
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = false;
  }
}

async function upload() {
  try {
    const f = await pickFile({ multiple: false, filters: [{ name: $t("skins.skin-png"), extensions: ["png"] }] });
    if (!f) return;
    const r = await fetch(convertFileSrc(f as string));
    const data = await r.blob();
    const dataUrl = await new Promise<string>((res) => {
      const fr = new FileReader();
      fr.onload = () => res(String(fr.result));
      fr.readAsDataURL(data);
    });
    const entry = await api.saveSkinFromData(`skin-${Date.now()}`, dataUrl);
    await loadSaved();
    await preview(await api.readSkinDataUrl(entry.filename));
    message.success($t("skins.skin-uploaded"));
  } catch (e) {
    message.error(String(e));
  }
}

/**
 * 预览当前账号**正在用的**皮肤。
 *
 * 正版账号的皮肤保存在官方服务器上，本地那份 `getOfflineSkin` 是空的 ——
 * 所以以前进皮肤中心，正版账号永远是空白（只有离线账号能显示）。这里按用户名
 * 从官方查一张拿来预览（顺带用它返回的 model 把细长/经典也切对，免得应用时改错模型）。
 */
async function loadCurrentSkin() {
  const acc = currentAccount.value;
  if (!acc) return;
  try {
    if (acc.type === "microsoft") {
      const r = await api.fetchPlayerSkin(acc.username);
      if (r.data_url) {
        model.value = r.model === "slim" ? "slim" : "default";
        await preview(r.data_url);
      }
    } else {
      const s = await api.getOfflineSkin(acc.uuid);
      if (s?.src) await preview(s.src);
    }
  } catch { /* 取不到就留空（离线名 / 无网络），不打断页面 */ }
}

onMounted(async () => {
  await loadCurrentSkin();
  await loadSaved();
});
// 在皮肤中心切账号（或从别处切完回来）要跟着换预览
watch(() => currentAccount.value?.uuid, () => void loadCurrentSkin());
onBeforeUnmount(() => renderer.dispose());

const ANIMS: { key: AnimationKind; label: string }[] = [
  { key: "idle", label: $t("skins.idle") },
  { key: "walk", label: $t("skins.walking") },
  { key: "run", label: $t("skins.running") },
  { key: "none", label: $t("skins.none") },
];
</script>

<template>
  <div class="sk">
    <div class="stage glass">
      <canvas ref="canvas" class="cv" />
      <div class="anims">
        <button
          v-for="a in ANIMS" :key="a.key" class="anim"
          :class="{ on: anim === a.key }"
          @click="anim = a.key; renderer.setAnimation(a.key)"
        >{{ a.label }}</button>
      </div>
    </div>
    <div class="row">
      <div class="models">
        <button class="m" :class="{ on: model === 'default' }" @click="model = 'default'; renderer.setModel('default')">{{ $t('skins.classic') }}</button>
        <button class="m" :class="{ on: model === 'slim' }" @click="model = 'slim'; renderer.setModel('slim')">{{ $t('skins.slim') }}</button>
      </div>
      <van-button type="primary" class="apply" :loading="busy" :disabled="!currentSrc" @click="apply">{{ $t('skins.applied') }}</van-button>
    </div>
    <!-- 「上传皮肤」原来挂在顶栏（那个位置切页时按钮会消失、把账号块挤得跳），
         顶栏取消动作按钮后，入口挪到这里。 -->
    <div class="row">
      <van-button class="grow" :loading="busy" @click="upload">{{ $t("skins.upload-skin") }}</van-button>
    </div>
    <div class="row">
      <van-search v-model="playerInput" :placeholder="$t('skins.fetch-by-name')" class="grow" @search="fetchPlayer" />
      <van-button type="primary" :loading="busy" @click="fetchPlayer">{{ $t('skins.fetch-by-premium-name') }}</van-button>
    </div>
    <h2 class="sec">{{ $t("skins.saved-local") }}</h2>
    <div v-if="skins.length" class="grid">
      <button v-for="s in skins" :key="s.filename" class="skin" @click="preview(thumbs[s.filename] ?? null)">
        <img :src="thumbs[s.filename]" class="thumb" alt="" />
        <span class="sname">{{ s.name }}</span>
      </button>
    </div>
    <!-- 空状态文案原来复用了日志组件的「暂无日志内容」，皮肤页写日志很奇怪 -->
    <van-empty v-else :description="$t('skins.no-saved')" />
  </div>
</template>

<style scoped>
.sk {
  padding: 4px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.stage {
  position: relative;
  height: 300px;
  border-radius: 16px;
  overflow: hidden;
}
.cv {
  width: 100%;
  height: 100%;
}
.anims {
  position: absolute;
  left: 8px;
  right: 8px;
  bottom: 8px;
  display: flex;
  gap: 6px;
  justify-content: center;
}
.anim {
  min-height: 32px;
  padding: 4px 10px;
  border-radius: 16px;
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg-1) 82%, transparent);
  color: var(--text-2);
  font-family: inherit;
  font-size: 12px;
}
.anim.on {
  color: var(--accent);
  border-color: var(--accent);
}
.row {
  display: flex;
  gap: 8px;
  align-items: center;
}
.grow {
  flex: 1;
  min-width: 0;
  padding: 0;
  background: transparent;
}
.models {
  display: flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}
.m {
  min-height: 38px;
  padding: 6px 12px;
  border: none;
  background: transparent;
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
}
.m.on {
  background: var(--accent);
  color: #1a1208;
}
.sec {
  margin: 4px 0 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-2);
}
.grid {
  display: grid;
  /* min() 兜住窄容器（与设置页同一套写法） */
  grid-template-columns: repeat(auto-fill, minmax(min(90px, 100%), 1fr));
  gap: 10px;
}
.skin {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--panel);
  cursor: pointer;
}
.thumb {
  width: 100%;
  border-radius: 8px;
  image-rendering: pixelated;
}
.sname {
  font-size: 12px;
  color: var(--text-2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
