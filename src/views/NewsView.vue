<script setup lang="ts">
import { onMounted } from "vue";
import { NButton, NEmpty, NSpin, useMessage } from "naive-ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import { IconRefresh, IconExternal } from "../components/icons";
import { useNewsStore } from "../stores/news";
import { fmtDateShort as fmtDate } from "../utils/format";

const message = useMessage();
const newsStore = useNewsStore();

async function refresh() {
  try {
    await newsStore.load(true);
  } catch (e) {
    message.error(String(e));
  }
}

onMounted(async () => {
  try {
    await newsStore.load();
  } catch (e) {
    message.error(String(e));
  }
});
</script>

<template>
  <div class="news-view">
    <div class="news-header">
      <h1>Minecraft 新闻</h1>
      <n-button size="small" secondary :loading="newsStore.loading" @click="refresh">
        <template #icon><IconRefresh /></template>
        刷新
      </n-button>
    </div>

    <div v-if="newsStore.loading && !newsStore.news.length" class="news-state">
      <n-spin size="small" />
      <span>加载中…</span>
    </div>
    <n-empty v-else-if="!newsStore.news.length" class="news-state" description="暂无新闻" />

    <div v-else class="news-list">
      <article
        v-for="n in newsStore.news"
        :key="n.url"
        class="news-card glass"
        @click="n.url && openUrl(n.url).catch(() => {})"
      >
        <div v-if="n.image" class="news-image">
          <img :src="n.image" :alt="n.image_alt" loading="lazy" />
        </div>
        <div class="news-body">
          <h3 class="news-title">{{ n.title }}</h3>
          <p v-if="n.description" class="news-desc">{{ n.description }}</p>
          <div class="news-meta">
            <span v-if="n.author" class="meta-author">{{ n.author }}</span>
            <span class="meta-date">{{ fmtDate(n.time) }}</span>
            <IconExternal class="meta-go" />
          </div>
        </div>
      </article>
    </div>
  </div>
</template>

<style scoped>
.news-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.news-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.news-header h1 {
  font-size: 20px;
  font-weight: 700;
  margin: 0;
}
.news-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  min-height: 160px;
  color: var(--text-3);
}

/* ── 列表：单列长条 → 多列紧凑卡 ────────────────────────────────────────
   这里踩过一个很隐蔽的坑：`.news-list` 同时是**滚动容器**（flex:1 + overflow-y:auto）
   和 **grid**，grid 的 auto 行在有确定高度的滚动容器里会被压扁 ——
   实测 24 张卡片被排成 36 行、每行只剩 5.48px，卡片互相叠在一起（用户看到的就是这个）。
   显式给 `grid-auto-rows: max-content` + `align-content: start` 就不会再塌。 */
.news-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(340px, 100%), 1fr));
  grid-auto-rows: max-content;
  align-content: start;
  gap: 12px;
}
/* 滚动容器里的子项禁止收缩（沿列向 flex 时也适用） */
.news-list > * {
  flex-shrink: 0;
}
.news-card {
  display: flex;
  border-radius: 14px;
  overflow: hidden;
  cursor: pointer;
  transition: transform 0.15s ease, border-color 0.15s ease;
}
.news-card:active {
  transform: scale(0.99);
}
.news-image {
  flex-shrink: 0;
  width: 132px;
  height: 92px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.03);
}
.news-image img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.news-body {
  flex: 1;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.news-title {
  font-size: 14px;
  font-weight: 700;
  margin: 0;
  color: var(--text-1);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.news-desc {
  font-size: 12px;
  color: var(--text-3);
  margin: 0;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.45;
}
.news-meta {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 11px;
  color: var(--text-3);
  margin-top: auto;
}
.meta-author {
  color: var(--accent);
  font-weight: 600;
}
.meta-go {
  margin-left: auto;
  opacity: 0.6;
}

/* ── 手机：横向紧凑卡（缩略图在左、文字在右），两列 ──────────────────────
   原来手机上改成「缩略图占满整行 + 文字在下面」的竖卡，一屏只放得下一条半；
   横卡信息密度合适、缩略图也足够看清，且不用滚动就能扫完一屏。 */
@media (max-width: 1100px), (pointer: coarse) {
  .news-list {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .news-image {
    width: 104px;
    height: 100%;
    min-height: 78px;
  }
  .news-title {
    -webkit-line-clamp: 2;
  }
  .news-desc {
    display: none;
  }
}
@media (max-width: 620px) {
  .news-list {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
