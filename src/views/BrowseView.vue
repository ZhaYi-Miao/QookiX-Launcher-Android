<script setup lang="ts">
import { t as $t } from "../i18n";
import { onMounted, ref } from "vue";

import { useMessage } from "naive-ui";
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
const showFilter = ref(false);
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
    const r = await api.browse(provider.value, query.value, type.value, "", page.value, version.value || undefined, undefined, undefined, 20);
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
onMounted(() => void load(true));
</script>

<template>
  <div class="bv">
    <div class="bar">
      <van-search v-model="query" :placeholder="$t('browse.search-placeholder')" class="q" @search="onSearch" />
      <button class="ftype" @click="showFilter = true">
        <IconSliders /> {{ TYPES.find((t) => t.value === type)?.label }}
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
        <label>{{ $t("browse.game-version") }}</label>
        <app-select v-model:value="version" :options="[]" :placeholder="$t('browse.all-versions')" size="small" />
      </div>
    </app-sheet>
    <install-dialog v-model:show="showInstall" :project="installTarget" :default-instance="instances.instances[0]?.id" @install-dep="openInstall" />
  </div>
</template>

<style scoped>
.bv {
  padding: 4px 12px 16px;
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
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(160px, 100%), 1fr));
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
