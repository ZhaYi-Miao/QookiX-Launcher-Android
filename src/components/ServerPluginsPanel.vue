<script setup lang="ts">
/**
 * 服务端插件面板（数据来自 PaperMC 的插件平台 Hangar）。
 *
 * 为什么不做成「内容中心 → 选服务器 → 安装」：插件的目标天然就是**这一个服务器**，
 * 放在服务器详情页里少一次选择；版本也不用用户挑 —— 拿服务器自己的 MC 版本去匹配，
 * 只有声明支持该版本的构建才会被装（装错版本的插件会让服务端起不来，而用户
 * 看着「装好了」完全不知道为什么）。
 */
import { t as $t } from "../i18n";
import { onMounted, ref } from "vue";
import { api } from "../api";
import { useMessage } from "../composables/message";
import { fmtSize } from "../utils/format";
import { handleExternalPlugin } from "../utils/pluginInstall";
import AppButton from "../ui/AppButton.vue";
import AppInput from "../ui/AppInput.vue";
import AppSheet from "../ui/AppSheet.vue";

const props = defineProps<{ serverId: string }>();
const message = useMessage();

type Hit = {
  slug: string;
  owner: string;
  name: string;
  description: string;
  downloads: number;
  versions: string[];
};
type Installed = { fileName: string; size: number };

const PAGE_SIZE = 20;
const query = ref("");
const hits = ref<Hit[]>([]);
const searching = ref(false);
const searchingMore = ref(false);
const page = ref(0);
const exhausted = ref(true);
const installing = ref<string | null>(null);
const installed = ref<Installed[]>([]);
const loadingInstalled = ref(false);

// 删除确认（沿用仓库里 app-sheet 的确认写法，不引新组件）
const confirmTarget = ref<Installed | null>(null);
const confirmLoading = ref(false);

async function doSearch(reset: boolean) {
  const q = query.value.trim();
  if (!q) return;
  if (reset) {
    page.value = 0;
    hits.value = [];
    exhausted.value = true;
    searching.value = true;
  } else {
    searchingMore.value = true;
  }
  try {
    const r = await api.hangarSearchPlugins(q, page.value, PAGE_SIZE);
    hits.value = reset ? r : [...hits.value, ...r];
    exhausted.value = r.length < PAGE_SIZE;
  } catch (e) {
    message.error(String(e));
  } finally {
    searching.value = false;
    searchingMore.value = false;
  }
}

async function loadMore() {
  if (exhausted.value || searchingMore.value) return;
  page.value += 1;
  await doSearch(false);
}

async function loadInstalled() {
  loadingInstalled.value = true;
  try {
    installed.value = await api.listServerPlugins(props.serverId);
  } catch {
    installed.value = [];
  } finally {
    loadingInstalled.value = false;
  }
}

async function install(h: Hit) {
  if (installing.value) return;
  installing.value = h.slug;
  try {
    const r = await api.installServerPlugin(props.serverId, h.slug, h.name);
    // 没托管在 Hangar 的插件：不算失败，直接开浏览器让用户自己下
    if (handleExternalPlugin(r, (m) => message.info(m))) return;
    message.success(
      $t("server-detail.plugins-installed-ok", { p1: r.fileName ?? "", p2: r.version ?? "" }),
    );
    // 接口没给平台版本信息时如实提醒：兼容性是「未知」而不是「已验证」
    if (!r.compatVerified) message.warning($t("server-detail.plugins-compat-unknown"));
    await loadInstalled();
  } catch (e) {
    message.error($t("server-detail.plugins-install-failed", { p1: String(e) }));
  } finally {
    installing.value = null;
  }
}

async function confirmDelete() {
  if (!confirmTarget.value) return;
  confirmLoading.value = true;
  try {
    await api.deleteServerPlugin(props.serverId, confirmTarget.value.fileName);
    message.success($t("server-detail.plugins-deleted"));
    confirmTarget.value = null;
    await loadInstalled();
  } catch (e) {
    message.error(String(e));
  } finally {
    confirmLoading.value = false;
  }
}

async function openPluginsDir() {
  // 复用既有命令（`server_files.rs::open_hosted_server_folder` 支持子目录）。
  // 它在 api.ts 里声明为 void（原生侧到底返回什么这里不做假设），所以只负责
  // 「把 plugins 目录交出去」；本机没有能认目录的应用时什么都不发生 ——
  // 用户仍可在「服务器文件」页里看/改，不必再弹一个看不懂的错。
  try {
    await api.openHostedServerFolder(props.serverId, "plugins");
  } catch (e) {
    message.error(String(e));
  }
}

onMounted(loadInstalled);
</script>

<template>
  <div class="plug">
    <p class="plug-hint">{{ $t("server-detail.plugins-hint") }}</p>

    <div class="plug-search">
      <app-input v-model:value="query" :placeholder="$t('server-detail.plugins-search-hint')" />
      <app-button :loading="searching" @click="doSearch(true)">
        {{ $t("server-detail.plugins-search") }}
      </app-button>
    </div>

    <div v-if="hits.length" class="plug-list">
      <button
        v-for="h in hits"
        :key="h.slug"
        class="plug-row"
        :disabled="!!installing"
        @click="install(h)"
      >
        <div class="plug-main">
          <div class="plug-name">
            {{ h.name }}
            <span class="plug-owner">{{ h.owner }}</span>
          </div>
          <div class="plug-desc">{{ h.description }}</div>
        </div>
        <span class="plug-act">
          {{ installing === h.slug ? $t("server-detail.plugins-installing") : $t("server-detail.plugins-install") }}
        </span>
      </button>
      <app-button v-if="!exhausted" :loading="searchingMore" @click="loadMore">
        {{ $t("server-detail.plugins-more") }}
      </app-button>
    </div>
    <p v-else-if="query && !searching" class="plug-hint">{{ $t("server-detail.plugins-empty") }}</p>

    <div class="plug-head">
      <h4>{{ $t("server-detail.plugins-installed") }}</h4>
      <button class="plug-link" @click="openPluginsDir">
        {{ $t("server-detail.plugins-open-dir") }}
      </button>
    </div>
    <p v-if="!installed.length && !loadingInstalled" class="plug-hint">
      {{ $t("server-detail.plugins-none") }}
    </p>
    <div v-for="p in installed" :key="p.fileName" class="plug-row static">
      <div class="plug-main">
        <div class="plug-name">{{ p.fileName }}</div>
        <div class="plug-desc">{{ fmtSize(p.size) }}</div>
      </div>
      <button class="plug-link danger" @click="confirmTarget = p">
        {{ $t("server-detail.plugins-delete") }}
      </button>
    </div>

    <app-sheet :show="!!confirmTarget" @mask-click="confirmTarget = null">
      <div v-if="confirmTarget" class="plug-confirm">
        <div class="plug-confirm-text">
          {{ $t("server-detail.plugins-delete-confirm", { p1: confirmTarget.fileName }) }}
        </div>
        <div class="plug-confirm-btns">
          <app-button @click="confirmTarget = null">{{ $t("common.cancel") }}</app-button>
          <app-button type="danger" :loading="confirmLoading" @click="confirmDelete">
            {{ $t("server-detail.plugins-delete") }}
          </app-button>
        </div>
      </div>
    </app-sheet>
  </div>
</template>

<style scoped>
.plug {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 4px 0 16px;
}
.plug-hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-3);
}
.plug-search {
  display: flex;
  gap: 8px;
  align-items: center;
}
.plug-search :deep(.app-input),
.plug-search :deep(input) {
  flex: 1;
  min-width: 0;
}
.plug-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.plug-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 6px;
}
.plug-head h4 {
  margin: 0;
  font-size: 14px;
}
.plug-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 56px;
  padding: 10px 12px;
  text-align: left;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
}
.plug-row.static {
  cursor: default;
}
.plug-main {
  flex: 1;
  min-width: 0;
}
.plug-name {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.plug-owner {
  font-size: 11px;
  font-weight: 400;
  color: var(--text-3);
}
.plug-desc {
  margin-top: 2px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-3);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.plug-act {
  flex-shrink: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--accent);
}
.plug-link {
  border: none;
  background: transparent;
  padding: 6px 2px;
  font-family: inherit;
  font-size: 13px;
  color: var(--accent);
}
.plug-link.danger {
  color: #e5534b;
}
.plug-confirm {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.plug-confirm-text {
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-2);
}
.plug-confirm-btns {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
</style>
