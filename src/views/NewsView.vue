<script setup lang="ts">
import { t as $t } from "../i18n";
import { onMounted } from "vue";
import { useMessage } from "../composables/message";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useNewsStore } from "../stores/news";
import { fmtDateShort as fmtDate } from "../utils/format";
import { Button as VanButton, Empty as VanEmpty } from "vant";
import { IconRefresh } from "../components/icons";

const newsStore = useNewsStore();
const message = useMessage();

function refresh() {
  newsStore.load(true).catch((e) => message.error(String(e)));
}
function open(u?: string) {
  if (u) void openUrl(u);
}
onMounted(() => {
  if (!newsStore.news.length) void newsStore.load();
});
</script>

<template>
  <div class="nv">
    <div class="bar">
      <van-button size="small" :loading="newsStore.loading" @click="refresh">
        <IconRefresh /> {{ $t("common.refresh") }}
      </van-button>
    </div>
    <p v-if="newsStore.loading && !newsStore.news.length" class="state">{{ $t("file-manager.loading") }}</p>
    <van-empty v-else-if="!newsStore.news.length" :description="$t('news.no-news')" />
    <article v-for="n in newsStore.news" :key="n.url" class="card glass" @click="open(n.url)">
      <img v-if="n.image" :src="n.image" class="img" alt="" loading="lazy" />
      <div class="txt">
        <h2 class="ttl">{{ n.title }}</h2>
        <p v-if="n.description" class="desc">{{ n.description }}</p>
        <p class="meta">{{ fmtDate(n.time) }}<span v-if="n.author"> · {{ n.author }}</span></p>
      </div>
    </article>
  </div>
</template>

<style scoped>
.nv {
  padding: 4px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.bar {
  display: flex;
  justify-content: flex-end;
}
.state {
  text-align: center;
  color: var(--text-3);
  padding: 48px 0;
}
.card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  cursor: pointer;
}
.img {
  width: 100%;
  border-radius: 10px;
}
.ttl {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
}
.desc {
  margin: 0;
  font-size: 13px;
  color: var(--text-2);
}
.meta {
  margin: 0;
  font-size: 11px;
  color: var(--text-3);
}
</style>
