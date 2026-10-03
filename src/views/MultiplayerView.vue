<script setup lang="ts">
import { t as $t } from "../i18n";
import { onMounted, ref } from "vue";
import { useMessage } from "../composables/message";
import { useServersStore } from "../stores/servers";
import { Button as VanButton, Empty as VanEmpty, Tabs as VanTabs, Tab as VanTab } from "vant";
import { IconPlus } from "../components/icons";
import AppPopup from "../ui/AppPopup.vue";
import AppInput from "../ui/AppInput.vue";

const servers = useServersStore();
const message = useMessage();
const tab = ref<"servers" | "online">("servers");
const showCreate = ref(false);
const form = ref({ name: "", core: "paper", mcVersion: "" });
const saving = ref(false);
const starting = ref("");

async function createServer() {
  if (!form.value.name.trim() || !form.value.mcVersion) return;
  saving.value = true;
  try {
    const s = await servers.create(form.value.name.trim(), form.value.core, form.value.mcVersion);
    showCreate.value = false;
    message.success($t("downloads.finished"));
    void servers.installCore(s.id);
  } catch (e) {
    message.error(String(e));
  } finally {
    saving.value = false;
  }
}

async function toggleRun(id: string) {
  starting.value = id;
  try {
    if (servers.isRunning(id)) await servers.stop(id);
    else await servers.start(id);
  } catch (e) {
    message.error(String(e));
  } finally {
    starting.value = "";
  }
}

onMounted(() => { void servers.load(); });
</script>

<template>
  <div class="mp">
    <van-tabs v-model:active="tab" class="tabs">
      <van-tab name="servers" :title="$t('home.server')" />
      <van-tab name="online" :title="$t('news.mc-news')" />
    </van-tabs>

    <template v-if="tab === 'servers'">
      <div v-if="servers.servers.length" class="list">
        <div v-for="s in servers.servers" :key="s.id" class="srv glass">
          <div class="srv-main">
            <div class="srv-name">{{ s.name }}</div>
            <div class="srv-meta">{{ s.core }} · {{ s.mc_version }} · :{{ s.port }}</div>
          </div>
          <van-button size="small" :type="servers.isRunning(s.id) ? 'default' : 'primary'" :loading="starting === s.id" @click="toggleRun(s.id)">
            {{ servers.isRunning(s.id) ? $t("multiplayer.stop") : $t("instance-card.launch") }}
          </van-button>
          <van-button size="small" @click="servers.remove(s.id)">{{ $t("common.delete") }}</van-button>
        </div>
      </div>
      <van-empty v-else :description="$t('instance-saves.no-servers')">
        <van-button type="primary" @click="showCreate = true"><IconPlus /> {{ $t("title-bar.new-server") }}</van-button>
      </van-empty>
    </template>

    <template v-else>
      <div class="online glass">
        <p class="hint">{{ $t("multiplayer.waiting") }}</p>
        <p class="hint">{{ $t("browse.baidu-page-hint") }}</p>
      </div>
    </template>

    <app-popup :show="showCreate" position="bottom" round @update:show="(v: boolean) => (showCreate = v)">
      <div class="sheet">
        <div class="sheet-title">{{ $t("title-bar.new-server") }}</div>
        <app-input v-model:value="form.name" :placeholder="$t('multiplayer.server-name-hint')" maxlength="40" />
        <app-input v-model:value="form.mcVersion" :placeholder="$t('create-instance.select-version')" maxlength="16" />
        <van-button block type="primary" :loading="saving" @click="createServer">{{ $t("multiplayer.create") }}</van-button>
      </div>
    </app-popup>
  </div>
</template>

<style scoped>
.mp {
  padding: 4px 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.srv {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
}
.srv-main {
  flex: 1;
  min-width: 0;
}
.srv-name {
  font-weight: 600;
  font-size: 14px;
}
.srv-meta {
  font-size: 12px;
  color: var(--text-3);
}
.hint {
  margin: 0 0 8px;
  font-size: 13px;
  color: var(--text-3);
}
.online {
  padding: 20px 16px;
}
.sheet {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px));
}
.sheet-title {
  font-size: 16px;
  font-weight: 600;
}
</style>
