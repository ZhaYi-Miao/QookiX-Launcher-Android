<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref } from "vue";

import { useMessage } from "../composables/message";
import { List as VanList, Empty as VanEmpty, Search as VanSearch } from "vant";
import { api } from "../api";
import { useInstancesStore } from "../stores/instances";

import type { ProjectHit } from "../types";
import ProjectCard from "../components/ProjectCard.vue";
import InstallDialog from "../components/InstallDialog.vue";
import AppSheet from "../ui/AppSheet.vue";
import AppSelect from "../ui/AppSelect.vue";
import { IconSliders } from "../components/icons";


const message = useMessage();
const instances = useInstancesStore();

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
    </div>
    <van-list v-model:loading="loading" :finished="done" finished-text="" @update:loading="(v: boolean) => v && page > 0 && load(false)">
      <div class="grid">
        <ProjectCard v-for="p in hits" :key="p.id + p.provider" :project="p" @install="openInstall" />
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
.grid {
  display: grid;
  /* 手机：一行一张卡。双列时每张卡只有 ~160px，标题/作者/描述全被压扁，
     「安装」两个字还会被挤成竖排。 */
  grid-template-columns: 1fr;
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
</style>
