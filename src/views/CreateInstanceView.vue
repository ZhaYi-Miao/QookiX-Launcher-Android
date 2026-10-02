<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useMessage } from "naive-ui";
import { Button as VanButton } from "vant";
import { api } from "../api";
import { useInstancesStore } from "../stores/instances";
import { pickFile } from "../composables/filePicker";
import AppInput from "../ui/AppInput.vue";
import AppSelect from "../ui/AppSelect.vue";

const router = useRouter();
const message = useMessage();
const instances = useInstancesStore();

const name = ref("");
const mcVersion = ref("");
const loader = ref("vanilla");
const versionOptions = ref<{ label: string; value: string }[]>([]);
const busy = ref(false);
const importing = ref(false);

const LOADERS = [
  { label: "Vanilla", value: "vanilla" },
  { label: "Fabric", value: "fabric" },
  { label: "Forge", value: "forge" },
  { label: "NeoForge", value: "neoforge" },
  { label: "Quilt", value: "quilt" },
];

const canSubmit = computed(() => !!name.value.trim() && !!mcVersion.value);

async function submit() {
  if (!canSubmit.value) return;
  busy.value = true;
  try {
    const inst = await instances.create(name.value.trim(), mcVersion.value, loader.value, null);
    message.success($t("create-instance.created-installing", { p1: inst.name }));
    instances.installGame(inst.id).catch(() => {});
    router.push(`/instance/${inst.id}`);
  } catch (e) {
    message.error($t("create-instance.install-failed", { p1: String(e) }));
  } finally {
    busy.value = false;
  }
}

async function importPack() {
  const f = await pickFile({ multiple: false, filters: [{ name: "modpack", extensions: ["zip", "mrpack"] }] });
  if (!f) return;
  importing.value = true;
  try {
    const inst = await api.importModpack(f as string);
    message.success($t("create-instance.imported-next", { p1: inst.name }));
    router.push(`/instance/${inst.id}`);
  } catch (e) {
    message.error(String(e));
  } finally {
    importing.value = false;
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
    <van-button block type="primary" :disabled="!canSubmit" :loading="busy" class="go" @click="submit">
      {{ $t("multiplayer.create") }}
    </van-button>
    <van-button block :loading="importing" @click="importPack">{{ $t("create-instance.import-modpack") }}</van-button>
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
.go {
  margin-top: 8px;
}
</style>
