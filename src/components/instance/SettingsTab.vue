<script setup lang="ts">
/**
 * 实例详情 · 设置 tab。
 * 从 InstanceDetailView 拆出，自行负责：内存分配（全局/自动/自定义三模式 +
 * 内存仪表）、实例别名、JVM/游戏参数、账号覆盖、分辨率、实例图标，
 * 以及 edit 草稿的防抖自动保存。
 */
import { t as $t } from "../../i18n";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import AppButton from "../../ui/AppButton.vue";
import { useMessage } from "../../composables/message";
import { useInstancesStore } from "../../stores/instances";
import { useAccountsStore } from "../../stores/accounts";
import { api } from "../../api";
import { useSettingsStore } from "../../stores/settings";
import { useMemoryInfo } from "../../composables/useMemoryInfo";
import { fmtMem } from "../../utils/format";
import { autoRendererFor, rendererNames, rendererOptions } from "../../utils/renderer";
import AppIcon from "../AppIcon.vue";
import IconPickerDialog from "../IconPickerDialog.vue";
import AppInput from "../../ui/AppInput.vue";
import AppSlider from "../../ui/AppSlider.vue";
import AppSelect from "../../ui/AppSelect.vue";
import AppSeg from "../../ui/AppSeg.vue";
import AppSwitch from "../../ui/AppSwitch.vue";

const props = defineProps<{ instanceId: string }>();

/** 分段控件的选项（文案走 i18n，key 与原来一致，审计仍为 0） */
const memoryModeOptions = computed(() => [
  { value: "global", label: $t("instance-settings.from-global") },
  { value: "auto", label: $t("instance-settings.auto") },
  { value: "custom", label: $t("instance-settings.mcreator") },
]);
const rendererModeOptions = computed(() => [
  { value: "auto", label: $t("instance-settings.auto-recommended") },
  { value: "global", label: $t("instance-settings.follow-global") },
  { value: "custom", label: $t("instance-settings.explicit") },
]);

// ── 导出 / 分享（整合包）─────────────────────────────────────────────
// 面板数据来自后端体检（纯本地统计，点开即有）；三项开关默认「配置带上、
// 存档/截图不带」—— 后两者体积大且含个人内容，要让用户显式勾。
const exportPlan = ref<Awaited<ReturnType<typeof api.planModpackExport>> | null>(null);
const exportConfig = ref(true);
const exportSaves = ref(false);
const exportShots = ref(false);
const exporting = ref(false);

const exportBundledCount = computed(() =>
  (exportPlan.value?.bundled ?? []).reduce((n, b) => n + b.count, 0),
);

async function loadExportPlan() {
  try {
    exportPlan.value = await api.planModpackExport(props.instanceId);
  } catch {
    // 体检失败不该挡住宿主页面（列表/其它设置照常用），静默降级成「没有统计」
    exportPlan.value = null;
  }
}

async function doExport() {
  if (exporting.value) return;
  exporting.value = true;
  try {
    const r = await api.exportModpack(props.instanceId, {
      includeConfig: exportConfig.value,
      includeSaves: exportSaves.value,
      includeScreenshots: exportShots.value,
    });
    message.success($t("instance-settings.export-done", { p1: r.fileName }));
  } catch (e) {
    message.error($t("instance-settings.export-failed", { p1: String(e) }));
  } finally {
    exporting.value = false;
    loadExportPlan();
  }
}

onMounted(loadExportPlan);

const instances = useInstancesStore();
const accounts = useAccountsStore();
const settingsStore = useSettingsStore();
const message = useMessage();
const instance = computed(() => instances.get(props.instanceId));

const showIconPicker = ref(false);
const edit = ref({
  icon: "",
  max_memory_mb: 4096,
  memory_mode: "global" as "global" | "auto" | "custom",
  jvm_args: "",
  game_args: "",
  account_id: "",
  resolution_w: "",
  resolution_h: "",
  // 渲染器：auto（按版本自动）/ global（跟随全局设置）/ custom（指定下面这个）
  renderer_mode: "auto" as "auto" | "global" | "custom",
  renderer: "opengles2",
});

/** 全局渲染器（读自 Pojav 偏好），只在「跟随全局」时要显示。 */
const globalRenderer = ref<string>("opengles2");

/** 本实例最终会用的渲染器 + 一句「为什么」。 */
const effectiveRenderer = computed(() => {
  const mode = edit.value.renderer_mode;
  if (mode === "custom") {
    return { key: edit.value.renderer, why: $t("instance-settings.why") };
  }
  if (mode === "global") {
    return { key: globalRenderer.value, why: $t("instance-settings.gen-sui-quan-ju-she-zhi") };
  }
  const key = autoRendererFor(instance.value?.mc_version);
  return {
    key,
    why: $t("instance-settings.an-ban-ben-zi-dong", { p1: instance.value?.mc_version ?? $t("crash-analyzer.unknown") }),
  };
});
// 别名不进自动保存的 edit 对象：每敲一个字符触发一次 patch + 列表重载
// 会非常卡。改为独立草稿 + 显式保存按钮。
const aliasDraft = ref("");
const savingAlias = ref(false);

const { memTotal, memUsed, memAvailable, startPolling, stopPolling } = useMemoryInfo();

// The custom slider max is capped at the currently available (free) memory,
// so the game allocation can never exceed the remaining space (fallback 16 GB).
const sliderMax = computed(() => {
  if (!memAvailable.value) return 16384;
  return Math.max(1024, memAvailable.value);
});

const globalMemoryMode = computed(() => settingsStore.settings?.memory_mode ?? "custom");
const globalMemory = computed(() => {
  if (globalMemoryMode.value === "auto") return autoMemory.value;
  return settingsStore.settings?.max_memory_mb ?? 4096;
});

const autoMemory = computed(() => {
  const modCount = instance.value?.mods?.length ?? 0;
  // Base: 40% of available (min 2048 MB), +512 MB per 100 mods (cap +4 GB)
  let rec = Math.max(2048, Math.floor(memAvailable.value * 40 / 100)) + Math.min(4096, Math.floor(modCount * 512 / 100));
  // Cap at 75% of available memory, leave room for OS
  const cap = Math.max(512, Math.floor(memAvailable.value * 3 / 4));
  rec = Math.min(rec, cap, 8192);
  return Math.max(rec, 512);
});

const effectiveMemory = computed(() => {
  if (edit.value.memory_mode === "auto") return autoMemory.value;
  if (edit.value.memory_mode === "global") return globalMemory.value;
  return edit.value.max_memory_mb;
});

const usedPercent = computed(() => {
  if (!memTotal.value) return 0;
  return Math.min(100, Math.round((memUsed.value / memTotal.value) * 100));
});
const allocPercent = computed(() => {
  if (!memTotal.value) return 0;
  return Math.min(100, Math.round((effectiveMemory.value / memTotal.value) * 100));
});
// The allocated segment sits right after the used segment so both colors are always visible.
const allocStart = computed(() => usedPercent.value);
const allocWidth = computed(() =>
  Math.max(0, Math.min(allocPercent.value, 100 - usedPercent.value))
);

/**
 * 用 store 里的实例回填 edit 草稿。
 *
 * 两个守卫缺一不可（都是为「选了跟随全局却没落盘」这个 bug 加的）：
 *  1. `savingDepth > 0` 时**绝不回填** —— 否则会用盘上的旧值把用户刚选的值冲掉，
 *     随后自动保存又把旧值写回盘，表现就是「改了像没改」。
 *  2. 值没变就不赋值 —— 列表刷新会给出新的对象引用，无脑赋值会触发下面的自动保存
 *     watcher 反复写盘（并发时还会互相覆盖）。
 */
let syncingEdit = false;
let savingDepth = 0;

function draftOf(i: NonNullable<typeof instance.value>): typeof edit.value {
  return {
    icon: i.icon ?? "",
    max_memory_mb: i.max_memory_mb ?? 4096,
    memory_mode: (i.memory_mode as "global" | "auto" | "custom") ?? "global",
    jvm_args: i.jvm_args ?? "",
    game_args: i.game_args ?? "",
    account_id: i.account_id ?? "",
    resolution_w: i.resolution?.[0]?.toString() ?? "",
    resolution_h: i.resolution?.[1]?.toString() ?? "",
    // 缺省 / null / "auto" 都算「自动」；旧实例文件本来就没有这个字段
    renderer_mode:
      !i.renderer || i.renderer === "auto"
        ? "auto"
        : i.renderer === "global"
          ? "global"
          : "custom",
    renderer: i.renderer && i.renderer !== "auto" && i.renderer !== "global"
      ? i.renderer
      : "opengles2",
  };
}

function sameDraft(a: typeof edit.value, b: typeof edit.value): boolean {
  return (
    a.icon === b.icon &&
    a.max_memory_mb === b.max_memory_mb &&
    a.memory_mode === b.memory_mode &&
    a.jvm_args === b.jvm_args &&
    a.game_args === b.game_args &&
    a.account_id === b.account_id &&
    a.resolution_w === b.resolution_w &&
    a.resolution_h === b.resolution_h &&
    a.renderer_mode === b.renderer_mode &&
    a.renderer === b.renderer
  );
}

watch(
  () => instance.value,
  (i) => {
    if (!i || savingDepth > 0) return;
    const next = draftOf(i);
    if (sameDraft(next, edit.value)) return;
    syncingEdit = true;
    edit.value = next;
    aliasDraft.value = i.alias ?? "";
    nextTick(() => {
      syncingEdit = false;
    });
  },
  { immediate: true }
);

async function saveSettings() {
  savingDepth += 1;
  try {
    const mem =
      edit.value.memory_mode === "custom"
        ? Math.min(edit.value.max_memory_mb, sliderMax.value)
        : 0;
    await instances.patch({
      id: props.instanceId,
      icon: edit.value.icon,
      max_memory_mb: mem,
      memory_mode: edit.value.memory_mode,
      jvm_args: edit.value.jvm_args,
      game_args: edit.value.game_args,
      account_id: edit.value.account_id,
      // auto / global 直接存字符串；custom 存具体渲染器键
      renderer:
        edit.value.renderer_mode === "custom"
          ? edit.value.renderer
          : edit.value.renderer_mode,
      resolution:
        edit.value.resolution_w && edit.value.resolution_h
          ? [Number(edit.value.resolution_w), Number(edit.value.resolution_h)]
          : null,
    });
  } catch (e) {
    message.error(String(e));
  } finally {
    savingDepth -= 1;
  }
}

/**
 * 渲染器改动**显式立刻保存**，不走防抖、也不依赖 watcher。
 *
 * 渲染器的典型用法是「选完马上点启动」—— 启动会把 WebView 切到后台，防抖定时器可能
 * 还没触发就没机会了；而 watcher 又被「首次跳过」这类标志吃过一次，两件事叠在一起
 * 就是「我明明选了跟随全局，启动还是按版本自动」。
 */
async function onRendererMode(raw: string | number) {
  // 分段控件回传的是 string | number，这里收窄到模式枚举
  const mode = String(raw) as "auto" | "global" | "custom";
  // 切到「指定」时把**当前实际生效**的那个带过去 —— 否则默认值会把渲染器悄悄换掉
  if (mode === "custom") {
    edit.value.renderer = effectiveRenderer.value.key;
  }
  edit.value.renderer_mode = mode;
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  await saveSettings();
}

/** 指定具体渲染器（下拉框）同样立刻保存。 */
async function onRendererKey(key: string) {
  edit.value.renderer = key;
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  await saveSettings();
}

/**
 * 启动前检查文件完整性。
 *
 * 不进 `edit` 草稿：它不是「输入完再存」的值，改动要立刻落盘（和渲染器同理——
 * 用户很可能改完马上点启动，防抖定时器还没跑就被切后台了）。
 * 缺省（旧实例没有这个键）视为**开启**。
 */
const checkFiles = computed(() => instance.value?.check_files_on_launch !== false);

async function onCheckFiles(v: boolean) {
  try {
    await instances.patch({ id: props.instanceId, check_files_on_launch: v });
  } catch (e) {
    message.error(String(e));
  }
}

/** 显式保存实例别名（点按钮触发，不走自动保存） */
async function saveAlias() {
  if (savingAlias.value) return;
  savingAlias.value = true;
  try {
    await instances.patch({ id: props.instanceId, alias: aliasDraft.value });
    message.success($t("instance-settings.alias-saved"));
  } catch (e) {
    message.error(String(e));
  } finally {
    savingAlias.value = false;
  }
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;
// 只有**用户改动**才进防抖保存：回填（syncingEdit）不算，保存自身不改 edit 所以不会自激。
// 渲染器不走这里 —— 它由 onRendererMode / onRendererKey 显式立刻保存（见那里注释）。
//
// 注意**不能**因为「正在保存」就 return：那会把保存期间发生的改动整段吞掉
// （滑杆连拖的最后一次就丢了）。照常重新计时，保存本身是幂等的。
watch(
  edit,
  () => {
    if (syncingEdit) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(saveSettings, 500);
  },
  { deep: true }
);

onMounted(() => {
  startPolling();
  // 全局渲染器存在安卓 SharedPreferences 里，「跟随全局」时要把它显示出来
  api
    .getPojavPrefs()
    .then((p) => {
      const r = p?.renderer;
      if (typeof r === "string" && r) globalRenderer.value = r;
    })
    .catch(() => {
      /* 读不到就用默认 GL4ES 显示 */
    });
});
onBeforeUnmount(() => {
  stopPolling();
  if (saveTimer) clearTimeout(saveTimer);
});
</script>

<template>
  <div class="settings-grid">
    <div class="set-card glass">
      <h4>{{ $t("instance-settings.memory") }}</h4>
      <app-seg v-model:value="edit.memory_mode" class="mem-modes" :options="memoryModeOptions" />

      <template v-if="edit.memory_mode === 'custom'">
        <app-slider
          v-model:value="edit.max_memory_mb"
          :min="1024"
          :max="sliderMax"
          :step="256" />
        <div class="range-labels"><span>1 GB</span><span>{{ fmtMem(sliderMax) }}</span></div>
        <div class="mem-current">{{ edit.max_memory_mb }} MB</div>
      </template>

      <div v-else class="mem-current">
        {{ effectiveMemory }} MB
        <span v-if="edit.memory_mode === 'global' && globalMemoryMode === 'auto'" class="mem-mode-note">{{ $t("instance-settings.global-auto-note") }}</span>
        <span v-else-if="edit.memory_mode === 'global'" class="mem-mode-note">{{ $t("instance-settings.global-manual-note") }}</span>
        <span v-else-if="edit.memory_mode === 'auto'" class="mem-mode-note">{{ $t("instance-settings.auto-note") }}</span>
      </div>

      <div class="mem-gauge">
        <div class="mem-gauge-track">
          <div class="mem-gauge-used" :style="{ width: usedPercent + '%' }"></div>
          <div
            class="mem-gauge-alloc"
            :style="{ left: allocStart + '%', width: allocWidth + '%' }"
          ></div>
        </div>
        <div class="mem-gauge-labels">
          <span><i class="dot used"></i>{{ $t("instance-settings.mem-used", { p1: fmtMem(memUsed), p2: usedPercent }) }}</span>
          <span><i class="dot alloc"></i>{{ $t("instance-settings.mem-assigned", { p1: fmtMem(effectiveMemory), p2: allocPercent }) }}</span>
          <span><i class="dot total"></i>{{ $t("instance-settings.mem-total", { p1: fmtMem(memTotal), p2: fmtMem(memAvailable) }) }}</span>
        </div>
      </div>
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.renderer") }}</h4>
      <app-seg
        :value="edit.renderer_mode"
        class="mem-modes"
        :options="rendererModeOptions"
        @update:value="onRendererMode"
      />

      <template v-if="edit.renderer_mode === 'custom'">
        <app-select
          :value="edit.renderer"
          :options="rendererOptions"
          size="small"
          style="margin-top: 10px"
          @update:value="onRendererKey" />
      </template>

      <p class="hint">{{ $t("instance-settings.effective") }}<b>{{ rendererNames[effectiveRenderer.key] ?? effectiveRenderer.key }}</b>
        （{{ effectiveRenderer.why }}）
      </p>
      <p class="hint">{{ $t("instance-settings.renderer-auto-rule") }}</p>
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.check-files") }}</h4>
      <div class="switch-row">
        <p class="hint switch-hint">{{ $t("instance-settings.check-files-hint") }}</p>
        <app-switch :value="checkFiles" @update:value="onCheckFiles" />
      </div>
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.alias") }}</h4>
      <div class="alias-row">
        <app-input
          v-model:value="aliasDraft"
          :placeholder="$t('instance-settings.alias-hint')"
          @keydown.enter="saveAlias" />
        <app-button
          size="small"
          :disabled="savingAlias || aliasDraft === (instance?.alias ?? '')"
          @click="saveAlias"
        >
          {{ savingAlias ? $t('file-manager.saving') : $t('common.save') }}
        </app-button>
      </div>
      <p class="hint">{{ $t("instance-settings.alias-usage") }}<code>qookix://launch/{{ aliasDraft || $t('instance-settings.alias-label') }}</code>{{ $t("instance-settings.alias-benefit") }}</p>
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.extra-jvm-args") }}</h4>
      <app-input
        v-model:value="edit.jvm_args"
        type="textarea"
        :rows="3"
        :placeholder="$t('instance-settings.jvm-args-hint')" />
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.extra-game-args") }}</h4>
      <app-input v-model:value="edit.game_args" :placeholder="$t('instance-settings.game-args-hint')" />
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.account") }}</h4>
      <app-select
        v-model:value="edit.account_id"
        :options="[
          { label: $t('instance-settings.follow-global-account', { p1: accounts.current?.username ?? $t('instance-settings.none-selected') }), value: '' },
          ...accounts.accounts.map((a) => ({
            label: `${a.username}（${a.type === 'microsoft' ? $t('account-chip.premium') : $t('account-chip.offline')}）`,
            value: a.uuid,
          })),
        ]" />
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.resolution") }}</h4>
      <div class="res-row">
        <app-input v-model:value="edit.resolution_w" :placeholder="$t('instance-settings.width-hint')" />
        <span>×</span>
        <app-input v-model:value="edit.resolution_h" :placeholder="$t('instance-settings.height-hint')" />
      </div>
    </div>

    <div class="set-card glass">
      <h4>{{ $t("instance-settings.icon") }}</h4>
      <div class="icon-pick">
        <div class="icon-preview">
          <AppIcon :name="edit.icon" />
        </div>
        <button class="btn" @click="showIconPicker = true">{{ $t("instance-settings.pick-icon") }}</button>
      </div>
    </div>

    <!-- 导出 / 分享：低频操作，所以放设置页而不是页头 —— 页头只有「启动」一个按钮，
         再塞图标进去会把最常用的那个挤没。 -->
    <div class="set-card glass">
      <h4>{{ $t("instance-settings.export-title") }}</h4>
      <p class="set-desc">{{ $t("instance-settings.export-desc") }}</p>
      <div v-if="exportPlan" class="export-lines">
        <div>{{ $t("instance-settings.export-linked", { p1: exportPlan.linked }) }}</div>
        <div>{{ $t("instance-settings.export-bundled", { p1: exportBundledCount, p2: fmtMem(exportPlan.bundledBytes) }) }}</div>
      </div>
      <label class="set-row">
        <span>{{ $t("instance-settings.export-config") }}</span>
        <app-switch v-model:value="exportConfig" />
      </label>
      <label v-if="exportPlan?.hasSaves" class="set-row">
        <span>{{ $t("instance-settings.export-saves") }}</span>
        <app-switch v-model:value="exportSaves" />
      </label>
      <label v-if="exportPlan?.hasScreenshots" class="set-row">
        <span>{{ $t("instance-settings.export-screenshots") }}</span>
        <app-switch v-model:value="exportShots" />
      </label>
      <button class="btn" :disabled="exporting" @click="doExport">
        {{ exporting ? $t("instance-settings.exporting") : $t("instance-settings.export-do") }}
      </button>
    </div>

    <IconPickerDialog
      v-model:show="showIconPicker"
      :value="edit.icon"
      :instance-id="instanceId"
      @save="edit.icon = $event"
    />
  </div>
</template>

<style scoped>
/* 导出卡片：说明文字 + 两行统计 + 三个开关 + 一个动作按钮 */
.set-desc {
  margin: 0 0 10px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-3);
}
.export-lines {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 10px;
  font-size: 12px;
  color: var(--text-2);
}
.set-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 40px;
  font-size: 13px;
  color: var(--text-2);
}
.settings-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(320px, 100%), 1fr));
  gap: 14px;
  /* 7 张设置卡在手机上约 900px 高：必须让这一层自己滚，
     否则整页滚动会把左侧 9 项导航带着一起滚出屏幕。 */
  height: 100%;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  align-content: start;
}
.set-card {
  padding: 16px;
}
.set-card h4 {
  margin: 0 0 12px;
  font-size: 14px;
}
.alias-row {
  display: flex;
  gap: 8px;
  align-items: center;
}
.alias-row .text-input {
  flex: 1;
  min-width: 0;
}
.alias-row .mini-btn {
  flex-shrink: 0;
  white-space: nowrap;
}
.text-input {
  flex: 1;
  min-width: 0;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid var(--border);
  border-radius: 9px;
  color: var(--text-1);
  padding: 8px 12px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  transition: border-color 0.12s;
}
.text-input:focus {
  border-color: var(--accent-05);
}
textarea.text-input {
  resize: vertical;
  width: 100%;
}
.range {
  width: 100%;
  accent-color: var(--accent);
}
.range-labels {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--text-3);
}
.mem-modes {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
  flex-wrap: wrap;
  /* naive-ui 的按钮组把高度写死成单个按钮高（默认假定只有一行）。我们允许换行，
     就必须把高度放开，否则第二行会**压在下面内容上**（实测：窄卡片/高缩放时
     「自定义」折行叠在 2048 MB 那行字上）。 */
  height: auto;
  /* 手机：三个档位等宽铺满一行，且按钮高度给到触控标准
     （原来按内容宽度排、只有 ~28px 高，手指点不准） */
  width: 100%;
}
.mem-modes :deep(.n-radio-button) {
  flex: 1;
  min-height: 38px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.mem-mode {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.04);
  font-size: 13px;
  cursor: pointer;
  color: var(--text-2);
  transition: all 0.12s;
}
.mem-mode:hover {
  background: rgba(255, 255, 255, 0.08);
}
.mem-mode.active {
  border-color: var(--accent);
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.mem-mode input {
  accent-color: var(--accent);
}
.mem-current {
  font-size: 14px;
  font-weight: 600;
  color: var(--accent);
  margin-top: 6px;
}
.mem-mode-note {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-3);
}
.mem-gauge {
  margin-top: 14px;
}
.mem-gauge-track {
  position: relative;
  height: 10px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}
.mem-gauge-used,
.mem-gauge-alloc {
  position: absolute;
  top: 0;
  bottom: 0;
  height: 100%;
  /* 两端圆角由外层 track 的 overflow:hidden 统一裁剪，
     两段之间保持直角无缝衔接，铺满整个轨道 */
  transition: width 0.2s, left 0.2s;
}
.mem-gauge-used {
  left: 0;
  background: linear-gradient(90deg, #5a8ef0, #8ab4ff);
}
.mem-gauge-alloc {
  background: linear-gradient(90deg, #e89a4b, #f2c079);
}
.mem-gauge-labels {
  display: flex;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 4px;
  font-size: 11px;
  color: var(--text-3);
  margin-top: 6px;
}
.mem-gauge-labels span {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}
.dot.used {
  background: #8ec4ff;
}
.dot.alloc {
  background: #e89a4b;
}
.dot.total {
  background: #9aa4b2;
}
.res-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.icon-pick {
  display: flex;
  align-items: center;
  gap: 10px;
}
.icon-preview {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  overflow: hidden;
  background: transparent;
  position: relative;
  flex-shrink: 0;
  box-sizing: border-box;
}
.icon-preview :deep(.app-icon) {
  position: absolute;
  inset: 0;
  font-size: 18px;
}
.mini-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-2);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.12s;
}
.mini-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-1);
}
.btn {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  border: none;
  border-radius: 10px;
  padding: 9px 16px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.14s;
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-1);
  border: 1px solid var(--border);
}
.btn:hover {
  background: rgba(255, 255, 255, 0.1);
}
.hint {
  font-size: 12px;
  color: var(--text-3);
  margin-top: 4px;
}
.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
/* 这一行里的说明文字和开关同一行，去掉段落的顶部间距，否则与开关不居中 */
.switch-row .switch-hint {
  margin-top: 0;
  flex: 1;
  min-width: 0;
}
/* 手机底线：这张卡里的所有 naive 按钮都抬到可点高度（别名增删、重命名等小按钮） */
.set-card :deep(.n-button) {
  min-height: 38px;
}
</style>
