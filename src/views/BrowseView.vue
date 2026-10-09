<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";

import { useMessage } from "../composables/message";
import { List as VanList, Empty as VanEmpty, Search as VanSearch } from "vant";
import { api } from "../api";
import { useInstancesStore } from "../stores/instances";
import { useSettingsStore } from "../stores/settings";

import type { ProjectHit } from "../types";
import ProjectCard from "../components/ProjectCard.vue";
import InstallDialog from "../components/InstallDialog.vue";
import AppSheet from "../ui/AppSheet.vue";
import AppSelect from "../ui/AppSelect.vue";
import { IconGlobe, IconSliders } from "../components/icons";


const message = useMessage();
const instances = useInstancesStore();
const settingsStore = useSettingsStore();

const loading = ref(false);
const query = ref("");
const hits = ref<ProjectHit[]>([]);
const page = ref(0);
const done = ref(false);
const type = ref("mod");
const provider = ref<"all" | "modrinth" | "curseforge">("all");
const version = ref("");
/** 游戏版本筛选的选项：官方 manifest 里的正式版（与创建实例页同一接口） */
const versionOptions = ref<{ label: string; value: string }[]>([]);
const showFilter = ref(false);
/** 加载器筛选（fabric/forge/…）：后端 browse 接口本来就支持 loader，之前一直没传 */
const loader = ref("");
/** 按实例筛选：只记实例 id，实际生效靠 setInstanceFilter 带出版本+加载器 */
const instanceFilter = ref("");

const LOADERS = [
  { value: "", label: $t("browse.all-loaders") },
  { value: "vanilla", label: "Vanilla" },
  { value: "fabric", label: "Fabric" },
  { value: "forge", label: "Forge" },
  { value: "neoforge", label: "NeoForge" },
  { value: "quilt", label: "Quilt" },
];

/** 筛选按钮的高亮：任何一项筛选生效就该亮（之前只看版本，用户以为没生效） */
const hasFilter = computed(() => !!(version.value || loader.value || instanceFilter.value || type.value !== "mod"));

/** 选实例 → 带出它的游戏版本与加载器（等价于「只看这个实例能用的内容」） */
function setInstanceFilter(id: string) {
  instanceFilter.value = id;
  const inst = id ? instances.get(id) : null;
  version.value = inst?.mc_version ?? "";
  loader.value = inst?.loader ?? "";
  void load(true);
}

/** 单独改加载器时，若它和所选实例冲突就清掉实例选择（避免两边打架看不出为什么没结果） */
function setLoader(next: string) {
  loader.value = next;
  if (instanceFilter.value) {
    const inst = instances.get(instanceFilter.value);
    if (inst && inst.loader !== next) instanceFilter.value = "";
  }
  void load(true);
}
const showInstall = ref(false);
const installTarget = ref<ProjectHit | null>(null);

// ---- 描述翻译：整页一起翻 ----
// 翻译服务有批量接口（一次最多 5 个、同一平台、按条数扣额度），所以这里按来源
// 分组后 5 条一批地翻，而不是一张张点。译文只放内存：换搜索词/刷新就没了，
// 免得和新一页的结果错位。
const showZh = ref(false); // 页面是否显示译文
const translatingPage = ref(false); // 整页翻译进行中
const pendingSlugs = ref<string[]>([]); // 本批还在翻的项目（卡片显示骨架）
const translatedDescs = ref<Record<string, string>>({});

function openBaiduByText(text: string) {
  const q = encodeURIComponent(text);
  openUrl(`https://fanyi.baidu.com/mtpe-individual/transText?query=${q}&lang=en2zh`).catch(() =>
    message.error($t("install-dialog.open-browser-failed"))
  );
}

async function translatePage() {
  if (translatingPage.value) return;
  const list = hits.value;
  if (!list.length) return;

  // 百度网页模式没有批量接口，只能一条条开浏览器
  const service = settingsStore.settings?.translate_provider ?? "default";
  if (service === "baidu_web") {
    const first = list[0];
    openBaiduByText(first.description || first.title);
    message.info($t("browse.baidu-page-hint"));
    return;
  }

  // 按来源分组：批量接口一批只能带一个平台
  //
  // 注意要用 **slug**：翻译服务（以及它的缓存）是以 slug 为键的，
  // 传 `p.id` 会「请求成功但译文查不到」—— 按钮变成「显示原文」、卡片却还是英文。
  const groups = new Map<string, string[]>();
  for (const p of list) {
    const key = p.slug || p.id;
    if (translatedDescs.value[key]) continue; // 已有译文的不用再翻
    const ids = groups.get(p.provider) ?? [];
    ids.push(key);
    groups.set(p.provider, ids);
  }
  showZh.value = true;
  if (!groups.size) return;

  translatingPage.value = true;
  pendingSlugs.value = [...groups.values()].flat();

  let rateLimited = false;
  let firstError = "";
  for (const [provider, ids] of groups) {
    if (rateLimited) break;
    for (let i = 0; i < ids.length; i += 5) {
      const chunk = ids.slice(i, i + 5);
      try {
        const r = await api.translateModDescriptions(provider, chunk);
        translatedDescs.value = { ...translatedDescs.value, ...r.translations };
        if (r.rateLimited) {
          rateLimited = true;
          firstError = firstError || $t("browse.translate-busy");
        } else if (r.error) {
          firstError = firstError || r.error;
        }
      } catch (e) {
        firstError = firstError || String(e);
      }
      pendingSlugs.value = pendingSlugs.value.filter((s) => !chunk.includes(s));
      // 限流是按 IP 算的，继续打只会更糟
      if (rateLimited) break;
    }
  }

  pendingSlugs.value = [];
  translatingPage.value = false;

  // 批量接口按条数扣额度（30 条/分），一页 20 条可能翻不完：
  // 已翻过的会被跳过，所以让用户再点一次「翻译本页」就能接着补。
  const total = list.length;
  const got = list.filter((p) => translatedDescs.value[p.slug || p.id]).length;
  if (got === 0) {
    showZh.value = false;
    message.warning(firstError || $t("browse.no-translations"));
    return;
  }
  if (got < total) {
    message.warning(
      $t("browse.translated-progress", {
        p1: got,
        p2: total,
        p3: rateLimited ? $t("browse.fu-wu-xian-liu") : "",
      })
    );
  }
}

function togglePageTranslate() {
  if (showZh.value) {
    showZh.value = false;
    return;
  }
  void translatePage();
}

// 开着译文时翻页/换搜索词，新一页自动跟上（仍是每页一次批量）
watch(hits, () => {
  if (showZh.value) void translatePage();
});

const TYPES = [
  { value: "mod", label: $t("browse.mods") },
  { value: "modpack", label: $t("instance-content.modpack") },
  { value: "resourcepack", label: $t("browse.group") },
  { value: "shader", label: $t("utils.categories.shader") },
];

async function load(reset = true) {
  if (loading.value) return;
  if (reset) {
    page.value = 0;
    done.value = false;
    hits.value = [];
  }
  loading.value = true;
  try {
    const r = await api.browse(provider.value, query.value, type.value, "", page.value, version.value || undefined, loader.value || undefined, undefined, 20);
    hits.value = reset ? r.hits : [...hits.value, ...r.hits];
    page.value += 1;
    if (!r.hits.length || hits.value.length >= r.total) done.value = true;
    if (r.cf_error) message.warning(String(r.cf_error));
  } catch (e) {
    message.error(String(e));
    done.value = true;
  } finally {
    loading.value = false;
  }
}

function onSearch() {
  void load(true);
}
function openInstall(p: any) {
  installTarget.value = p;
  showInstall.value = true;
}
onMounted(async () => {
  void load(true);
  try {
    const m = await api.getVersionManifest();
    versionOptions.value = m.versions
      .filter((v) => v.type === "release")
      .map((v) => ({ label: v.id, value: v.id }));
  } catch {
    // manifest 拉不到（离线）时筛选里就没有版本可选，占位符仍在，不影响其它筛选
  }
});
</script>

<template>
  <div class="bv">
    <div class="bar">
      <van-search v-model="query" :placeholder="$t('browse.search-placeholder')" class="q" @search="onSearch" />
      <button class="ftype" :class="{ on: hasFilter }" @click="showFilter = true">
        <IconSliders /> {{ $t("browse.filter") }}
      </button>
      <!-- 整页翻译：一次翻这一页所有卡片的描述（译文只放内存，换搜索词即失效） -->
      <button
        class="ftype"
        :class="{ on: showZh }"
        :disabled="translatingPage"
        @click="togglePageTranslate"
      >
        <IconGlobe />
        {{
          translatingPage
            ? $t("install-dialog.translating")
            : showZh
              ? $t("install-dialog.show-source")
              : $t("browse.translate-page")
        }}
      </button>
    </div>
    <van-list v-model:loading="loading" :finished="done" finished-text="" @update:loading="(v: boolean) => v && page > 0 && load(false)">
      <div class="grid">
        <ProjectCard
          v-for="p in hits"
          :key="p.id + p.provider"
          :project="p"
          :translating="pendingSlugs.includes(p.slug || p.id)"
          :translated-desc="showZh ? (translatedDescs[p.slug || p.id] ?? null) : null"
          @install="openInstall"
        />
      </div>
      <van-empty v-if="!loading && !hits.length" :description="$t('browse.no-results')" />
    </van-list>
    <app-sheet v-model:show="showFilter" :title="$t('browse.filter')">
      <div class="fgroup">
        <label>{{ $t("browse.filter") }}</label>
        <div class="chips">
          <button v-for="t in TYPES" :key="t.value" class="chip" :class="{ on: type === t.value }" @click="type = t.value; load(true)">
            {{ t.label }}
          </button>
        </div>
      </div>
      <div class="fgroup">
        <label>{{ $t("browse.instance-filter") }}</label>
        <!-- 按实例筛选：后端 browse 接口没有「实例」参数，这里用「选中实例 →
             自动带出它的游戏版本 + 加载器」来实现等价效果（内容中心给某个实例找模组
             本来就是这个意思）。选「全部实例」= 不按实例收窄。 -->
        <div class="chips">
          <button class="chip" :class="{ on: !instanceFilter }" @click="setInstanceFilter('')">
            {{ $t("browse.all-instances") }}
          </button>
          <button
            v-for="inst in instances.instances"
            :key="inst.id"
            class="chip"
            :class="{ on: instanceFilter === inst.id }"
            @click="setInstanceFilter(inst.id)"
          >
            {{ inst.name }}
          </button>
        </div>
      </div>
      <div class="fgroup">
        <label>{{ $t("browse.loader-filter") }}</label>
        <div class="chips">
          <button
            v-for="l in LOADERS"
            :key="l.value"
            class="chip"
            :class="{ on: loader === l.value }"
            @click="setLoader(l.value)"
          >
            {{ l.label }}
          </button>
        </div>
      </div>
      <div class="fgroup">
        <label>{{ $t("browse.game-version") }}</label>
        <!-- 版本列表来自官方 manifest（与创建实例页同一接口）；
             之前这里硬编码了空数组，弹层里永远只有占位符（什么都选不了）。 -->
        <app-select v-model:value="version" :options="versionOptions" :placeholder="$t('browse.all-versions')" size="small" />
      </div>
    </app-sheet>
    <install-dialog v-model:show="showInstall" :project="installTarget" :default-instance="instances.instances[0]?.id" @install-dep="openInstall" />
  </div>
</template>

<style scoped>
.bv {
  padding: 4px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.bar {
  display: flex;
  gap: 8px;
  align-items: center;
}
.q {
  flex: 1;
  min-width: 0;
  padding: 0;
  background: transparent;
}
.ftype {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 40px;
  padding: 8px 12px;
  white-space: nowrap;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
  font-size: 13px;
}
/* 有筛选生效时给个提示点，不然看不出「筛选」按钮里藏着条件 */
.ftype.on {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-08);
}
.ftype :deep(svg) {
  width: 16px;
  height: 16px;
}
/* 窄屏（工具栏里三个控件：搜索 + 筛选 + 翻译）把按钮收窄一点，别把搜索框挤没 */
@media (max-width: 400px) {
  .ftype {
    padding: 8px 9px;
    font-size: 12px;
    gap: 4px;
  }
}
.grid {
  display: grid;
  /* 手机：一行一张卡。双列时每张卡只有 ~160px，标题/作者/描述全被压扁，
     「安装」两个字还会被挤成竖排。
     ⚠️ 写 `minmax(0, 1fr)` 而不是 `1fr`：`1fr` = `minmax(auto, 1fr)`，轨道最小宽
     = 卡片 min-content；而卡片里的标题是 `white-space: nowrap`，min-content 就是
     **标题整行宽度** → 窄屏（360px 视口实测）上轨道被撑到比容器还宽 20px，
     卡片右侧直接溢出屏幕、顶到边缘。配合 ProjectCard 里的 `min-width: 0` 一起生效。 */
  grid-template-columns: minmax(0, 1fr);
  gap: 10px;
  grid-auto-rows: max-content;
}
.fgroup {
  margin-bottom: 14px;
}
.fgroup label {
  display: block;
  margin-bottom: 6px;
  font-size: 13px;
  color: var(--text-2);
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.chip {
  min-height: 34px;
  padding: 6px 12px;
  border-radius: 17px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
  white-space: nowrap;
}
.chip.on {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-08);
}

/* ── 手机横屏（矮窗口）：内容卡排 2~3 列 ─────────────────────────────────
 * 上面那条「一行一张卡」是给 360px 宽的竖屏写的（双列会把标题/作者压扁）；
 * 横屏内容区有 800px+，三列刚刚好。 */
@media (orientation: landscape) and (max-height: 560px) {
  .grid {
    grid-template-columns: repeat(auto-fill, minmax(min(240px, 100%), 1fr));
  }
}
</style>
