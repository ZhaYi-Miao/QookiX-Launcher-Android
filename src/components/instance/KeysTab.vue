<script setup lang="ts">
import { t as $t } from "../../i18n";
import { computed, onMounted, ref } from "vue";
import { NButton, NInput, useDialog, useMessage } from "naive-ui";
import { api } from "../../api";
import type { ControlLayoutInfo } from "../../types";
import AppSheet from "../../ui/AppSheet.vue";

// 手机端的「按键」= 屏幕上的触控控制层。布局文件放在 <files>/controlmap/，
// **全局共享、不区分实例**（所以这里不按实例过滤），当前生效的那份由 pojav 偏好
// defaultCtrl 指向。细调（大小/颜色/键位映射/组合键）在游戏内的「自定义控制布局」里做，
// 这个 tab 只做启动器侧能做的：选布局、复制、重命名、删除。
const props = defineProps<{ instanceId: string }>();
void props; // 布局是全局的，实例 id 只为了与其他 tab 组件签名一致

const message = useMessage();
const dialog = useDialog();

const layouts = ref<ControlLayoutInfo[]>([]);
const loading = ref(false);
const busy = ref("");

const currentLayout = computed(() => layouts.value.find((l) => l.current) ?? null);

async function loadLayouts() {
  loading.value = true;
  try {
    layouts.value = await api.listControlLayouts();
  } catch (e) {
    message.error(String(e));
  } finally {
    loading.value = false;
  }
}
onMounted(() => void loadLayouts());

async function run(name: string, action: () => Promise<ControlLayoutInfo[]>, okText: string) {
  if (busy.value) return;
  busy.value = name;
  try {
    layouts.value = await action();
    message.success(okText);
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

function setCurrent(name: string) {
  void run(name, () => api.setCurrentControlLayout(name), $t("instance-keys.run", { p1: name }));
}

/// 手机打字麻烦：复制出来的名字自动生成（「原名 副本」「原名 副本2」…）
function freeName(base: string): string {
  const taken = new Set(layouts.value.map((l) => l.name));
  let candidate = $t("instance-keys.candidate", { p1: base });
  let i = 2;
  while (taken.has(candidate)) {
    candidate = $t("instance-keys.copy-suffix", { p1: base, p2: i });
    i += 1;
  }
  return candidate;
}

function duplicate(name: string) {
  const newName = freeName(name);
  void run(name, () => api.duplicateControlLayout(name, newName), $t("instance-keys.yi-fu-zhi-wei", { p1: newName }));
}

const renameShow = ref(false);
const renameFrom = ref("");
const renameTo = ref("");

function openRename(name: string) {
  renameFrom.value = name;
  renameTo.value = name;
  renameShow.value = true;
}

async function submitRename() {
  const to = renameTo.value.trim();
  if (!to) {
    message.warning($t("instance-keys.name-required"));
    return;
  }
  if (to === renameFrom.value) {
    renameShow.value = false;
    return;
  }
  try {
    layouts.value = await api.renameControlLayout(renameFrom.value, to);
    message.success($t("instance-keys.renamed", { p1: to }));
    renameShow.value = false;
  } catch (e) {
    message.error(String(e));
  }
}

function remove(name: string) {
  const target = layouts.value.find((l) => l.name === name);
  dialog.warning({
    title: $t("instance-keys.delete"),
    content: target?.current
      ? $t("instance-keys.delete-active-confirm", { p1: name })
      : $t("instance-keys.delete-confirm", { p1: name }),
    positiveText: $t("common.delete"),
    negativeText: $t("common.cancel"),
    onPositiveClick: async () => {
      try {
        layouts.value = await api.deleteControlLayout(name);
        message.success($t("crash-analyzer.on-positive-click"));
      } catch (e) {
        message.error(String(e));
      }
    },
  });
}

function fmtSize(bytes: number): string {
  if (!bytes) return "—";
  return bytes >= 1024 ? `${(bytes / 1024).toFixed(0)} KB` : `${bytes} B`;
}

function fmtTime(sec: number): string {
  if (!sec) return "—";
  const d = new Date(sec * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getMonth() + 1}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
</script>

<template>
  <div class="tab-root">
    <div class="keys-note glass">
      <p class="note-line">{{ $t("instance-keys.keys-intro-a") }}<b>{{ $t("instance-keys.keys-shared") }}</b>{{ $t("instance-keys.keys-intro-b") }}<b>{{ $t("instance-keys.keys-howto-a") }}</b>{{ $t("instance-keys.keys-intro-c") }}</p>
      <p class="note-line dim">{{ $t("instance-keys.keys-howto-b") }}<b>{{ $t("instance-keys.keys-howto-d") }}</b>{{ $t("instance-keys.keys-howto-c") }}</p>
    </div>

    <div v-if="currentLayout" class="keys-cur glass">
      <div class="cur-main">
        <div class="cur-label">{{ $t("instance-keys.in-use") }}</div>
        <div class="cur-name">{{ currentLayout.name }}</div>
      </div>
      <div class="cur-meta">
        {{ $t("instance-keys.stats", { p1: currentLayout.buttons, p2: currentLayout.joysticks, p3: currentLayout.drawers }) }}</div>
    </div>

    <div v-if="loading" class="keys-empty">{{ $t("instance-keys.loading") }}</div>

    <div v-else-if="!layouts.length" class="keys-empty glass">
      <p>{{ $t("instance-keys.no-layout") }}</p>
    </div>

    <div v-else class="keys-list glass">
      <div
        v-for="l in layouts"
        :key="l.name"
        class="k-row"
        :class="{ active: l.current, busy: busy === l.name }"
      >
        <div class="k-info">
          <div class="k-name">
            {{ l.name }}
            <span v-if="l.current" class="k-badge">{{ $t("instance-keys.active") }}</span>
          </div>
          <div class="k-meta">
            {{ $t("instance-keys.stats-b", { p1: l.buttons, p2: l.joysticks, p3: l.drawers, p4: fmtSize(l.size), p5: fmtTime(l.modified) }) }}
          </div>
        </div>
        <div class="k-actions">
          <n-button v-if="!l.current" size="tiny" :disabled="!!busy" @click="setCurrent(l.name)">{{ $t("instance-keys.set-current") }}</n-button>
          <n-button size="tiny" :disabled="!!busy" @click="duplicate(l.name)">{{ $t("common.copy") }}</n-button>
          <n-button size="tiny" :disabled="!!busy" @click="openRename(l.name)">{{ $t("common.rename") }}</n-button>
          <n-button
            size="tiny"
            type="error"
            ghost
            :disabled="!!busy || l.name === 'default'"
            @click="remove(l.name)"
          >{{ $t("common.delete") }}</n-button>
        </div>
      </div>
    </div>

    <app-sheet
      v-model:show="renameShow"
      :title="$t('instance-keys.rename')"
      class="keys-rename-card"
    >
      <n-input v-model:value="renameTo" :placeholder="$t('instance-keys.new-name')" @keyup.enter="submitRename" />
      <p class="rename-hint">{{ $t("instance-keys.name-hint") }}</p>
      <div class="rename-actions">
        <n-button @click="renameShow = false">{{ $t("common.cancel") }}</n-button>
        <n-button type="primary" @click="submitRename">{{ $t("file-manager.ok") }}</n-button>
      </div>
    </app-sheet>
  </div>
</template>

<style scoped>
/* 与其他 tab 一致：外层撑满、内部自己滚 */
.tab-root {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding-right: 2px;
}
.keys-note,
.keys-cur,
.keys-list,
.keys-empty {
  border-radius: 12px;
  padding: 12px 14px;
}
.note-line {
  margin: 0;
  font-size: 12px;
  line-height: 1.65;
  color: var(--text-2);
}
.note-line + .note-line {
  margin-top: 6px;
}
.note-line.dim {
  color: var(--text-3);
}
.keys-cur {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.cur-label {
  font-size: 11px;
  color: var(--text-3);
}
.cur-name {
  font-size: 15px;
  font-weight: 700;
  margin-top: 2px;
}
.cur-meta {
  font-size: 12px;
  color: var(--text-3);
}
.keys-empty {
  font-size: 12px;
  color: var(--text-3);
  line-height: 1.7;
}
.keys-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
/* 行内允许换行：窄屏/高缩放下按钮不许压到名字上 */
.k-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
  padding: 8px 10px;
  border: 1px solid transparent;
  border-radius: 10px;
}
.k-row + .k-row {
  border-top: 1px solid var(--border);
}
.k-row.active {
  border-color: var(--accent-45);
  background: var(--accent-soft);
}
.k-row.busy {
  opacity: 0.6;
}
.k-info {
  min-width: 0;
  flex: 1 1 200px;
}
.k-name {
  font-size: 13px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 6px;
  word-break: break-all;
}
.k-badge {
  font-size: 10px;
  font-weight: 600;
  padding: 1px 6px;
  border-radius: 6px;
  background: var(--accent-soft);
  color: var(--accent);
  flex-shrink: 0;
}
.k-meta {
  font-size: 11px;
  color: var(--text-3);
  margin-top: 2px;
}
.k-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  flex-shrink: 0;
}
.rename-hint {
  margin: 8px 0 0;
  font-size: 11px;
  color: var(--text-3);
}
.rename-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 14px;
}
</style>
