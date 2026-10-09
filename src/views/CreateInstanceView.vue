<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useMessage } from "../composables/message";
import { Button as VanButton } from "vant";
import { api } from "../api";
import { pickFile } from "../composables/filePicker";
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

// ── 导入整合包 ────────────────────────────────────────────────────────
// 与「全新创建」并列的模式：整合包自带 MC 版本与加载器，用户只要选文件，
// 不必再手填一遍（填错反而更糟）。解析 / 建实例 / 解 overrides / 下载内容
// 全在后端一条流水线里跑，进度进「下载中心」。
const mode = ref<"fresh" | "import">("fresh");
const importing = ref(false);

async function pickAndImportModpack() {
  if (importing.value) return;
  let path: string | null = null;
  try {
    path = await pickFile({
      multiple: false,
      // .mrpack（Modrinth）与 .zip（CurseForge / 离线包）都收
      filters: [{ name: $t("create-instance.modpack-file"), extensions: ["mrpack", "zip"] }],
    });
  } catch (e) {
    message.error(String(e));
    return;
  }
  if (!path) return;
  importing.value = true;
  try {
    const r = await api.importModpack(path);
    // 「少下了一部分」必须当场告诉用户：只写日志的后果是他进游戏才崩，
    // 而那时错误信息指不到真凶（资源文件那次就是这么踩的）。
    const bad = (r.failed ?? 0) + (r.skippedNoUrl ?? 0);
    if (bad > 0) {
      message.warning($t("create-instance.import-partial", { p1: bad }));
    } else {
      message.success($t("create-instance.import-done", { p1: r.instance.name }));
    }
    await instances.load(true);
    router.push(`/instance/${r.instance.id}`);
  } catch (e) {
    message.error($t("create-instance.import-failed", { p1: String(e) }));
  } finally {
    importing.value = false;
  }
}

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
    <!-- 两种入口并列。整合包那条不需要用户再填版本/加载器（包自带），
         让他手填一遍只会填错，所以两条路的表单完全不同。 -->
    <div class="modes">
      <button class="chip" :class="{ on: mode === 'fresh' }" @click="mode = 'fresh'">
        {{ $t("create-instance.mode-fresh") }}
      </button>
      <button class="chip" :class="{ on: mode === 'import' }" @click="mode = 'import'">
        {{ $t("create-instance.mode-import") }}
      </button>
    </div>

    <template v-if="mode === 'fresh'">
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
    </template>

    <template v-else>
      <div class="hint import-hint">{{ $t("create-instance.import-hint") }}</div>
      <van-button block type="primary" :loading="importing" class="go" @click="pickAndImportModpack">
        {{ $t("create-instance.pick-modpack") }}
      </van-button>
    </template>
  </div>
</template>

<style scoped>
.cv {
  padding: 8px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.modes {
  display: flex;
  gap: 8px;
}
.modes .chip {
  flex: 1;
}
.import-hint {
  line-height: 1.6;
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
