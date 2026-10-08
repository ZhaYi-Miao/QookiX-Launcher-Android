<script setup lang="ts">
import { t as $t } from "../../i18n";
import { computed, onMounted, onUnmounted, ref } from "vue";
import AppButton from "../../ui/AppButton.vue";
import { useDialog } from "../../composables/dialog";
import { useMessage } from "../../composables/message";
import { api } from "../../api";
import type { ControlImportResult, ControlLayoutInfo } from "../../types";
import AppSheet from "../../ui/AppSheet.vue";
import AppInput from "../../ui/AppInput.vue";
import AppPopup from "../../ui/AppPopup.vue";
import { IconCopy, IconDownload, IconEdit, IconMoreVertical, IconTrash } from "../icons";

// 手机端的「按键」= 屏幕上的触控控制层。布局文件放在 <files>/controlmap/，
// **全局共享、不区分实例**（所以这里不按实例过滤），当前生效的那份由 pojav 偏好
// defaultCtrl 指向。
//
// 现在启动器侧也能改布局了：**进原生编辑器**（就是游戏里那个横屏界面，所见即所得），
// 以及导入/导出。细调（大小/颜色/键位映射/组合键）在那个编辑器里做，和游戏内完全一致。
const props = defineProps<{ instanceId: string }>();
void props; // 布局是全局的，实例 id 只为了与其他 tab 组件签名一致

const message = useMessage();
const dialog = useDialog();

const layouts = ref<ControlLayoutInfo[]>([]);
const loading = ref(false);
const busy = ref("");
/** 布局操作面板的目标（手机：行内只留主操作，其余进底部清单） */
const kTarget = ref<string | null>(null);

// ── 打开原生编辑器 / 导入 / 导出 ────────────────────────────────────────
/** 从系统设置页（原生编辑器）返回后要刷新列表：布局可能已被改名/新增 */
function refreshOnFocus() {
  const on = () => {
    if (document.visibilityState === "visible") void loadLayouts();
  };
  document.addEventListener("visibilitychange", on);
  onUnmounted(() => document.removeEventListener("visibilitychange", on));
}

/** 编辑某一份布局（不传 = 当前默认那份），进的是游戏里那个横屏编辑器。 */
async function editLayout(name?: string) {
  if (busy.value) return;
  busy.value = name ?? "*";
  try {
    await api.openControlLayoutEditor(name, false, undefined);
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

/** 导出（系统分享）某一份布局。 */
async function exportLayout(name: string) {
  if (busy.value) return;
  busy.value = name;
  try {
    await api.exportControlLayout(name);
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

/** 导入：选文件 → 校验 → 问名字 → 进原生编辑器只读预览 → 确认后才落盘。 */
async function importLayout() {
  if (busy.value) return;
  busy.value = "import";
  try {
    await api.pickControlLayout();
    const res = await waitForImportResult();
    await afterImport(res);
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

/** SAF 是异步的：原生侧把结果写进临时文件，这里轮询取 */
async function waitForImportResult(): Promise<ControlImportResult | null> {
  for (let i = 0; i < 60; i++) {
    await new Promise((r) => setTimeout(r, 500));
    const res = await api.takeControlImport();
    if (res) return res;
  }
  return null;
}

/** 校验结果处理 + 问名字 + 进预览 */
async function afterImport(res: ControlImportResult | null) {
  if (!res) {
    message.warning($t("instance-keys.import-cancelled"));
    return;
  }
  if (!res.ok) {
    message.error(res.error || $t("instance-keys.import-invalid"));
    return;
  }
  // 先让用户起个名字，再进预览确认 —— 名字在启动器里问，比在原生对话框里打字舒服
  const suggested = freeName(res.name || "imported");
  const name = await askName(suggested);
  if (!name) return;
  await api.openControlLayoutEditor(res.name, true, name);
  // 预览确认后原生才会 copy 成正式布局，回到启动器再刷一次列表
  setTimeout(() => void loadLayouts(), 1500);
}

/**
 * 按**文件路径**导入 —— 给「系统选择器打不开某些目录」准备的第二条路。
 *
 * 系统只认 SAF，我们没法让 MT 代替我们弹选择器（MT 没这个接口）；
 * 但 MT 能拿到那些目录里文件的绝对路径，用户复制过来我们直接读就行
 * （配合「所有文件访问」权限）。
 */
async function importLayoutByPath() {
  if (busy.value) return;
  const path = await askText($t("instance-keys.import-path-title"), "");
  if (!path) return;
  busy.value = "import";
  try {
    const res = await api.importControlLayoutByPath(path);
    await afterImport(res);
  } catch (e) {
    message.error(String(e));
  } finally {
    busy.value = "";
  }
}

/** 通用文本输入弹层（复用命名那个弹层）。 */
function askText(title: string, initial: string): Promise<string | null> {
  return new Promise((resolve) => {
    renameFrom.value = "";
    renameTo.value = initial;
    textResolve = resolve;
    renameShow.value = true;
    renameTitle.value = title;
  });
}
let textResolve: ((v: string | null) => void) | null = null;

/** 复用重命名那套弹层问个名字；取消返回 null。 */
function askName(initial: string): Promise<string | null> {
  return new Promise((resolve) => {
    renameFrom.value = "";
    renameTo.value = initial;
    renameResolve = resolve;
    renameShow.value = true;
  });
}
let renameResolve: ((v: string | null) => void) | null = null;

/**
 * 面板动作统一包装：先收起面板再执行。
 *
 * **目标名字必须作为参数传给 fn**。原来模板里写的是 `runK(() => openRename(kTarget!))`：
 * 闭包里的 `kTarget` 是模板 ref，**调用时才解包** —— 而 runK 在调用 fn 之前已经把它置成
 * null，于是每个动作实际拿到 null。重命名一提交就在 `renameTo.value.trim()` 上抛
 * TypeError（用户实测「重命名会报错」就是这个）；复制/删除则是拿着 null 去发请求、
 * 后端报「布局不存在」。SavesTab 的 worldTarget 是正确写法（先捕获再置空），照它改。
 */
function runK(fn: (name: string) => unknown) {
  const name = kTarget.value;
  kTarget.value = null;
  if (!name) return;
  void fn(name);
}

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
onMounted(() => {
  void loadLayouts();
  refreshOnFocus();
});

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
/** 弹层标题可变：命名 / 输入文件路径共用这一个弹层 */
const renameTitle = ref("");

function openRename(name: string) {
  renameFrom.value = name;
  renameTo.value = name;
  renameTitle.value = "";
  renameShow.value = true;
}

async function submitRename() {
  const to = renameTo.value.trim();
  // 文本输入（导入路径）也复用这个弹层
  if (textResolve) {
    const done = textResolve;
    textResolve = null;
    renameShow.value = false;
    if (!to) {
      message.warning($t("instance-keys.path-required"));
      done(null);
      return;
    }
    done(to);
    return;
  }
  // 导入流程也复用这个弹层：此时 renameResolve 有值，只需把名字回给它
  if (renameResolve) {
    const done = renameResolve;
    renameResolve = null;
    renameShow.value = false;
    if (!to) {
      message.warning($t("instance-keys.name-required"));
      done(null);
      return;
    }
    done(to);
    return;
  }
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

function cancelRename() {
  renameShow.value = false;
  if (textResolve) {
    const done = textResolve;
    textResolve = null;
    done(null);
  }
  if (renameResolve) {
    const done = renameResolve;
    renameResolve = null;
    done(null);
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
    <!-- 原来这里有一张「手机端的「按键」是…所有实例共用…」的说明卡片（.keys-note）。
         用户要求整张卡片不要：下面「当前使用 default 16 个按键」那块已经把该说的说清楚了，
         说明文字既占地方又跟下面重复。相关的 instance-keys.keys-intro-* / keys-howto-*
         / keys-shared 文案已从 zh-CN.json 一并删除。 -->
    <div v-if="currentLayout" class="keys-cur glass">
      <div class="cur-main">
        <div class="cur-label">{{ $t("instance-keys.in-use") }}</div>
        <div class="cur-name">{{ currentLayout.name }}</div>
      </div>
      <div class="cur-meta">
        {{ $t("instance-keys.stats", { p1: currentLayout.buttons, p2: currentLayout.joysticks, p3: currentLayout.drawers }) }}</div>
    </div>

    <!-- 编辑 / 导入 / 导出：编辑器是原生那个横屏界面（和游戏里所见即所得） -->
    <div class="keys-tools glass">
      <button class="k-tool" :disabled="!!busy" @click="editLayout(currentLayout?.name)">
        {{ $t("instance-keys.edit-layout") }}
      </button>
      <button class="k-tool" :disabled="!!busy" @click="importLayout">
        {{ $t("instance-keys.import") }}
      </button>
      <button class="k-tool" :disabled="!!busy" @click="importLayoutByPath">
        {{ $t("instance-keys.import-by-path") }}
      </button>
      <button
        class="k-tool"
        :disabled="!!busy || !currentLayout"
        @click="currentLayout && exportLayout(currentLayout.name)"
      >
        {{ $t("instance-keys.export") }}
      </button>
      <!-- SAF 打不开 Android/data 等目录，所以明确告诉用户还有一条路：
           在 MT 之类有权限的文件管理器里「分享 → QookiX」也能导入。 -->
      <p class="k-tools-hint">{{ $t("instance-keys.import-hint") }}</p>
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
        <!-- 手机：行内只留「设为当前」（主操作），复制/重命名/删除收进更多面板。
             原来 4 个 tiny 按钮并排，在竖屏里每个只剩 ~30px 宽，手指根本点不准。 -->
        <div class="k-actions">
          <button class="k-set" :disabled="!!busy" @click.stop="editLayout(l.name)">
            {{ $t("instance-keys.edit-layout") }}
          </button>
          <button v-if="!l.current" class="k-set" :disabled="!!busy" @click.stop="setCurrent(l.name)">
            {{ $t("instance-keys.set-current") }}
          </button>
          <button class="k-more" :aria-label="$t('common.rename')" @click.stop="kTarget = l.name">
            <IconMoreVertical />
          </button>
        </div>
      </div>
    </div>

    <app-sheet
      v-model:show="renameShow"
      :title="renameTitle || $t('instance-keys.rename')"
      class="keys-rename-card"
    >
      <app-input
        v-model:value="renameTo"
        :placeholder="renameTitle ? $t('instance-keys.import-path-placeholder') : $t('instance-keys.new-name')"
        @keyup.enter="submitRename"
      />
      <p class="rename-hint">{{ renameTitle || $t("instance-keys.name-hint") }}</p>
      <div class="rename-actions">
        <app-button @click="cancelRename">{{ $t("common.cancel") }}</app-button>
        <app-button type="primary" @click="submitRename">{{ $t("file-manager.ok") }}</app-button>
      </div>
    </app-sheet>

    <!-- 布局操作面板（手机形态） -->
    <app-popup
      :show="kTarget !== null"
      position="bottom"
      round
      @update:show="(v: boolean) => { if (!v) kTarget = null; }"
    >
      <div v-if="kTarget" class="k-panel">
        <div class="k-panel-title text-ellipsis">{{ kTarget }}</div>
        <button class="k-act" :disabled="!!busy" @click="runK(editLayout)">
          <IconEdit />{{ $t("instance-keys.edit-layout") }}
        </button>
        <button class="k-act" :disabled="!!busy" @click="runK(exportLayout)">
          <IconDownload />{{ $t("instance-keys.export") }}
        </button>
        <button class="k-act" :disabled="!!busy" @click="runK(duplicate)">
          <IconCopy />{{ $t("common.copy") }}
        </button>
        <button class="k-act" :disabled="!!busy" @click="runK(openRename)">
          <IconEdit />{{ $t("common.rename") }}
        </button>
        <button
          class="k-act danger"
          :disabled="!!busy || kTarget === 'default'"
          @click="runK(remove)"
        >
          <IconTrash />{{ $t("common.delete") }}
        </button>
      </div>
    </app-popup>
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
.keys-cur,
.keys-list,
.keys-tools,
.keys-empty {
  border-radius: 12px;
  padding: 12px 14px;
}
/* 编辑 / 导入 / 导出：三个等宽按钮一行（窄屏允许换行） */
.keys-tools {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.k-tool {
  flex: 1 1 96px;
  min-height: 44px;
  border-radius: 12px;
  border: 1px solid var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
  font-family: inherit;
  font-size: 14px;
  font-weight: 600;
}
.k-tool:disabled {
  opacity: 0.5;
}
/* 按钮下面那行小字：提示「也可以从MT 分享导入」 */
.k-tools-hint {
  flex: 1 0 100%;
  margin: 2px 0 0;
  font-size: 12px;
  line-height: 1.5;
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
/* ── 手机形态：行内主操作 + 更多面板 ──────────────────────────── */
.k-row {
  min-height: 64px;
}
.k-set {
  flex-shrink: 0;
  min-height: 38px;
  padding: 0 14px;
  border-radius: 10px;
  border: 1px solid var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
  font-family: inherit;
  font-size: 13px;
  font-weight: 600;
}
.k-more {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-3);
}
.k-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px));
}
.k-panel-title {
  font-size: 15px;
  font-weight: 700;
  margin-bottom: 2px;
}
.k-act {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 48px;
  padding: 0 14px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
  font-size: 15px;
  text-align: left;
}
.k-act.danger {
  color: #e5534b;
  border-color: rgba(229, 83, 75, 0.4);
}
.k-act:disabled {
  opacity: 0.45;
}
/* 重命名弹层的两个键：通栏平分 */
.rename-actions :deep(.n-button) {
  flex: 1;
  min-height: 44px;
}
</style>
