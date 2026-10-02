<script setup lang="ts">
import { t as $t } from "../i18n";
import { ref } from "vue";
import { useSettingsStore } from "../stores/settings";
import { Cell as VanCell, CellGroup as VanCellGroup, Button as VanButton } from "vant";
import AppSwitch from "../ui/AppSwitch.vue";
import { IconSliders, IconPackage, IconImage, IconDownload, IconGlobe, IconPlay, IconRefresh, IconFile } from "../components/icons";

const settings = useSettingsStore();
const tab = ref<string | null>(null);

const groups = [
  { key: "general", label: $t("settings.general"), icon: IconSliders },
  { key: "appearance", label: $t("settings.appearance"), icon: IconImage },
  { key: "download", label: $t("downloads.download"), icon: IconDownload },
  { key: "plugins", label: $t("server-detail.plugins"), icon: IconPackage },
  { key: "content", label: $t("settings.content-services"), icon: IconGlobe },
  { key: "game", label: $t("settings.game"), icon: IconPlay },
  { key: "storage", label: $t("utils.categories.storage"), icon: IconRefresh },
  { key: "about", label: $t("settings.about"), icon: IconFile },
];
</script>

<template>
  <div class="sv">
    <van-cell-group v-if="!tab" inset class="grp">
      <van-cell v-for="g in groups" :key="g.key" :title="g.label" is-link center @click="tab = g.key" />
    </van-cell-group>
    <van-cell-group v-if="tab === 'general'" inset class="grp">
      <van-cell :title="$t('settings.show-news')" center>
        <template #right-icon>
          <app-switch :value="settings.settings!.show_news" @update:value="settings.patch({ show_news: $event })" />
        </template>
      </van-cell>
      <van-cell :title="$t('settings.auto-update')" center>
        <template #right-icon>
          <app-switch :value="settings.settings!.auto_update" @update:value="settings.patch({ auto_update: $event })" />
        </template>
      </van-cell>
    </van-cell-group>
    <van-cell-group v-if="tab === 'appearance'" inset class="grp">
      <van-cell :title="$t('settings.theme')" center>
        <template #right-icon>
          <div class="seg">
            <button :class="{ on: settings.settings!.theme !== 'light' }" @click="settings.patch({ theme: 'dark' })">{{ $t('settings.dark') }}</button>
            <button :class="{ on: settings.settings!.theme === 'light' }" @click="settings.patch({ theme: 'light' })">{{ $t('settings.light') }}</button>
          </div>
        </template>
      </van-cell>
    </van-cell-group>
    <div class="back"><van-button v-if="tab" block @click="tab = null">{{ $t('common.cancel') }}</van-button></div>
  </div>
</template>

<style scoped>
.sv {
  padding: 4px 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.seg {
  display: flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}
.seg button {
  min-height: 32px;
  padding: 6px 12px;
  border: none;
  background: transparent;
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
}
.seg button.on {
  background: var(--accent);
  color: #1a1208;
}
</style>
