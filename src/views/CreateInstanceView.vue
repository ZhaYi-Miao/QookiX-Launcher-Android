<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useMessage } from "../composables/message";
import { Button as VanButton } from "vant";
import { api } from "../api";
import { useInstancesStore } from "../stores/instances";
import AppInput from "../ui/AppInput.vue";
import AppSelect from "../ui/AppSelect.vue";

const router = useRouter();
const message = useMessage();
const instances = useInstancesStore();

const name = ref("");
const mcVersion = ref("");
const loader = ref("vanilla");
const loaderVersion = ref("");
const loaderVersionOptions = ref<{ label: string; value: string }[]>([]);
const loaderVersionsLoading = ref(false);
const loaderVersionsError = ref("");
const versionOptions = ref<{ label: string; value: string }[]>([]);
const busy = ref(false);

const LOADERS = [
  { label: "Vanilla", value: "vanilla" },
  { label: "Fabric", value: "fabric" },
  { label: "Forge", value: "forge" },
  { label: "NeoForge", value: "neoforge" },
  { label: "Quilt", value: "quilt" },
];

const loaderLabel = computed(() => LOADERS.find((l) => l.value === loader.value)?.label ?? loader.value);

// 选了加载器就必须带上版本号：`loader_version` 空着创建出来的实例，
// 加载器装不上、依赖也解析不出来，连「补全游戏文件」都没有可下手的版本。
// 名称不参与校验：留空由后端兜底成游戏版本号（输入框提示就是这么写的）。
const canSubmit = computed(
  () => !!mcVersion.value && (loader.value === "vanilla" || !!loaderVersion.value),
);

/** 换加载器或换游戏版本都要重取一遍版本列表，默认选最新 */
async function loadLoaderVersions() {
  loaderVersion.value = "";
  loaderVersionOptions.value = [];
  loaderVersionsError.value = "";
  if (loader.value === "vanilla" || !mcVersion.value) return;

  loaderVersionsLoading.value = true;
  try {
    const list = await api.getLoaderVersions(loader.value, mcVersion.value);
    loaderVersionOptions.value = list.map((v) => ({ label: v, value: v }));
    loaderVersion.value = list[0] ?? "";
    if (!list.length) {
      // 参数写在同一行：文案审计是逐行扫 `$t(key, { … })`，换行会误报「少传占位符」
      const p1 = loaderLabel.value;
      const p2 = mcVersion.value;
      loaderVersionsError.value = $t("create-instance.loader-version-none", { p1, p2 });
    }
  } catch (e) {
    loaderVersionsError.value = $t("create-instance.loader-version-failed", { p1: String(e) });
  } finally {
    loaderVersionsLoading.value = false;
  }
}

watch([loader, mcVersion], loadLoaderVersions);

async function submit() {
  if (!canSubmit.value) return;
  busy.value = true;
  try {
    const inst = await instances.create(
      name.value.trim(),
      mcVersion.value,
      loader.value,
      loader.value === "vanilla" ? null : loaderVersion.value,
    );
    message.success($t("create-instance.created-installing", { p1: inst.name }));
    instances.installGame(inst.id).catch(() => {});
    router.push(`/instance/${inst.id}`);
  } catch (e) {
    message.error($t("create-instance.install-failed", { p1: String(e) }));
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  try {
    const m = await api.getVersionManifest();
    versionOptions.value = m.versions
      .filter((v) => v.type === "release")
      .map((v) => ({ label: v.id, value: v.id }));
    mcVersion.value = m.latest.release;
  } catch (e) {
    message.error(String(e));
  }
});
</script>

<template>
  <div class="cv">
    <div class="field">
      <label>{{ $t("instances.name") }}</label>
      <app-input v-model:value="name" :placeholder="$t('create-instance.name-hint')" maxlength="40" />
    </div>
    <div class="field">
      <label>{{ $t("create-instance.select-version") }}</label>
      <app-select v-model:value="mcVersion" :options="versionOptions" :placeholder="$t('create-instance.select-version')" />
    </div>
    <div class="field">
      <label>{{ $t("browse.loader") }}</label>
      <div class="chips">
        <button v-for="l in LOADERS" :key="l.value" class="chip" :class="{ on: loader === l.value }" @click="loader = l.value">
          {{ l.label }}
        </button>
      </div>
    </div>
    <div v-if="loader !== 'vanilla'" class="field">
      <label>{{ $t("create-instance.loader-version") }}</label>
      <app-select
        v-if="loaderVersionOptions.length"
        v-model:value="loaderVersion"
        :options="loaderVersionOptions"
        :placeholder="$t('create-instance.select-loader-version')"
      />
      <div v-else class="hint">
        {{ loaderVersionsLoading ? $t("create-instance.loader-version-loading") : loaderVersionsError }}
      </div>
    </div>
    <van-button block type="primary" :disabled="!canSubmit" :loading="busy" class="go" @click="submit">
      {{ $t("multiplayer.create") }}
    </van-button>
  </div>
</template>

<style scoped>
.cv {
  padding: 8px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.field label {
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
  min-height: 38px;
  padding: 8px 14px;
  border-radius: 19px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
}
.chip.on {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-08);
}
.hint {
  font-size: 13px;
  color: var(--text-2);
  line-height: 1.5;
}
.go {
  margin-top: 8px;
}
</style>
