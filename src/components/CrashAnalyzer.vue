<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, ref, watch } from "vue";
import AppButton from "../ui/AppButton.vue";
import { Loading as VanLoading } from "vant";
import { useMessage } from "../composables/message";
import { useDialog } from "../composables/dialog";
import { api } from "../api";
import { fmtDateLocale as fmtTime, fmtSize } from "../utils/format";
import { error as devError } from "../utils/logger";
import type { CrashDiagnosis } from "../types";
import {
  IconAlertCircle,
  IconBug,
  IconChevronDown,
  IconChevronRight,
  IconCopy,
  IconFile,
  IconRefresh,
  IconTrash,
} from "./icons";

const props = defineProps<{ instanceId: string }>();
const message = useMessage();
const dialog = useDialog();

const loading = ref(false);
const analyzing = ref(false);
const logs = ref<{ filename: string; modified: number; size: number; kind: string }[]>([]);
const selected = ref<string>("");
const diagnosis = ref<CrashDiagnosis | null>(null);
const rawContent = ref("");
const showRaw = ref(false);

// Tauri 的 invoke 报错可能是字符串，也可能是带 message 的对象，
// 统一转成可读文本，避免界面上只弹出一个 [object Object] 或啥都没有。
function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

// —— 分析结果缓存 ——
// 崩溃报告内容基本不会变，每次切走再回来都重新分析既慢又费（要读文件 + 跑诊断）。
// 按 实例 + 文件名 缓存诊断，TTL 1 小时。重新分析会强制刷新。
const DIAG_TTL = 7 * 24 * 60 * 60 * 1000;
interface DiagCache {
  d: CrashDiagnosis;
  ts: number;
}
function diagKey(instanceId: string, filename: string) {
  return `qookix:crash_diag:${instanceId}:${filename}`;
}
function readDiagCache(instanceId: string, filename: string): CrashDiagnosis | null {
  try {
    const raw = localStorage.getItem(diagKey(instanceId, filename));
    if (!raw) return null;
    const c = JSON.parse(raw) as DiagCache;
    if (Date.now() - c.ts > DIAG_TTL) return null;
    return normalize(c.d);
  } catch {
    return null;
  }
}
function writeDiagCache(instanceId: string, filename: string, d: CrashDiagnosis) {
  try {
    localStorage.setItem(diagKey(instanceId, filename), JSON.stringify({ d, ts: Date.now() }));
  } catch {
    /* 容量溢出等忽略，不影响主流程 */
  }
}

async function loadLogs() {
  loading.value = true;
  try {
    const r = await api.crashAnalysis(props.instanceId);
    logs.value = r;
    if (logs.value.length && !selected.value) {
      // 自动选中第一个文件时必须同时恢复其缓存诊断，
      // 否则切走再回来 diagnosis 为空，看起来像"缓存没生效"。
      selected.value = logs.value[0].filename;
      diagnosis.value = readDiagCache(props.instanceId, selected.value);
    }
  } catch (e) {
    devError("[CrashAnalyzer] loadLogs failed:", e);
    message.error($t("crash-analyzer.load-failed") + errText(e));
  } finally {
    loading.value = false;
  }
}

async function analyze(force = false) {
  if (!selected.value) {
    // 没选文件时给明确提示，而不是静默 return（否则看起来像"点了没反应"）
    if (!logs.value.length) {
      message.warning($t("crash-analyzer.instance-no-reports"));
    } else {
      message.warning($t("crash-analyzer.select-first"));
    }
    return;
  }
  // 命中有效缓存且非强制刷新：直接展示，不发请求
  if (!force) {
    const cached = readDiagCache(props.instanceId, selected.value);
    if (cached) {
      diagnosis.value = cached;
      return;
    }
  }
  analyzing.value = true;
  diagnosis.value = null;
  rawContent.value = "";
  try {
    const d = normalize(await api.analyzeCrash(props.instanceId, selected.value));
    diagnosis.value = d;
    writeDiagCache(props.instanceId, selected.value, d);
  } catch (e) {
    devError("[CrashAnalyzer] analyze failed:", e);
    message.error($t("crash-analyzer.analyze-failed") + errText(e));
  } finally {
    analyzing.value = false;
  }
}

async function loadRaw() {
  if (!selected.value) {
    message.warning($t("crash-analyzer.select-first"));
    return;
  }
  try {
    rawContent.value = await api.getCrashReportContent(props.instanceId, selected.value);
  } catch (e) {
    devError("[CrashAnalyzer] loadRaw failed:", e);
    message.error($t("crash-analyzer.read-failed") + errText(e));
  }
}

// 点「查看原始报告」时：若还没加载过原始内容就顺手拉取，不必再单独点一次
async function ensureRaw() {
  if (rawContent.value) return;
  await loadRaw();
}

// 展开/收起原始报告：展开时若未加载则自动拉取
async function toggleRaw() {
  showRaw.value = !showRaw.value;
  if (showRaw.value) await ensureRaw();
}

function deleteLog(filename: string) {
  // 原来这里用的是 WebView 原生的 `window.confirm()`：与全项目自绘的对话框风格不一致
  // （Android 上是系统 AlertDialog 的样式），而且它的返回非常依赖宿主 WebChromeClient。
  // 换成与「删除服务器」等处一致的 `dialog.warning({ ... onPositiveClick })`。
  dialog.warning({
    content: $t("crash-analyzer.confirm", { p1: filename }),
    positiveText: $t("common.delete"),
    negativeText: $t("common.cancel"),
    onPositiveClick: async () => {
      try {
        await api.deleteInstancePath(props.instanceId, `crash-reports/${filename}`);
        // 顺手清掉对应诊断缓存，避免留下孤儿数据
        localStorage.removeItem(diagKey(props.instanceId, filename));
        message.success($t("crash-analyzer.on-positive-click"));
        await loadLogs();
        diagnosis.value = null;
        rawContent.value = "";
      } catch (e) {
        message.error(String(e));
      }
    },
  });
}

/**
 * 复制文本。
 *
 * 注意必须 `await`：之前写成「同步 try/catch 包了一个返回 Promise 的 writeText」，
 * rejection 永远进不了 catch，于是**不管成功失败都提示「已复制」** ——
 * 用户以为复制好了，粘出来却是旧内容。剪贴板 API 需要用户手势 + 安全上下文，
 * 在安卓 WebView 上是会失败的，必须让失败真的报出来。
 */
async function copyText(text: string) {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    message.success($t("crash-analyzer.copied"));
  } catch {
    message.error($t("crash-analyzer.copy-failed"));
  }
}

function copyExcerpt() {
  void copyText(diagnosis.value?.excerpt ?? "");
}

function copyRaw() {
  void copyText(rawContent.value);
}

function copyStack() {
  void copyText(diagnosis.value?.stacktrace.join("\n") ?? "");
}
/** 主因已在上方单独展示，这里列出其余命中原因 */
const otherCauses = computed(() => (diagnosis.value?.causes ?? []).slice(1));

/**
 * 兼容旧缓存：7 天 TTL 内可能存着没有 causes/stacktrace/details 的旧结构，
 * 直接读字段会在模板里炸，这里统一补齐默认值。
 */
function normalize(d: CrashDiagnosis): CrashDiagnosis {
  return {
    ...d,
    causes: d.causes ?? [],
    stacktrace: d.stacktrace ?? [],
    details: d.details ?? [],
    affected_mods: d.affected_mods ?? [],
    confidence: typeof d.confidence === "number" ? d.confidence : 0,
  };
}

// 严重度 → 中文名 / 前景色 / 背景色。主因与分因共用同一套配色。
function severityLabel(s: string): string {
  switch (s) {
    case "oom":
      return $t("crash-analyzer.sev-oom");
    case "jvm":
      return $t("crash-analyzer.sev-jvm");
    case "gl":
      return $t("crash-analyzer.sev-gl");
    case "mod":
      return $t("crash-analyzer.mod-issue");
    case "lwjgl":
      return $t("crash-analyzer.sev-lwjgl");
    case "java_ver":
      return $t("crash-analyzer.sev-java");
    default:
      return $t("crash-analyzer.unknown");
  }
}

function severityColor(s: string): string {
  switch (s) {
    case "oom":
      return "#e5534b";
    case "jvm":
      return "#e0a030";
    case "gl":
      return "#5aa2f0";
    case "mod":
      return "#7ad08a";
    case "lwjgl":
      return "#c78aff";
    case "java_ver":
      return "#ff7a90";
    default:
      return "#8b8e9c";
  }
}

function severityBg(s: string): string {
  switch (s) {
    case "oom":
      return "rgba(229,83,75,0.12)";
    case "jvm":
      return "rgba(224,160,48,0.12)";
    case "gl":
      return "rgba(90,162,240,0.12)";
    case "mod":
      return "rgba(122,208,138,0.12)";
    case "lwjgl":
      return "rgba(199,138,255,0.12)";
    case "java_ver":
      return "rgba(255,122,144,0.12)";
    default:
      return "rgba(139,142,156,0.12)";
  }
}

/** 置信度 → 文案 */
function confidenceLabel(c: number): string {
  if (c >= 85) return $t("crash-analyzer.very-likely");
  if (c >= 60) return $t("crash-analyzer.likely");
  if (c > 0) return $t("crash-analyzer.possible");
  return $t("crash-analyzer.not-located");
}

watch(
  () => props.instanceId,
  () => {
    logs.value = [];
    selected.value = "";
    diagnosis.value = null;
    rawContent.value = "";
    loadLogs();
  },
  { immediate: true }
);

watch(selected, () => {
  diagnosis.value = null;
  rawContent.value = "";
});

function handleSelect(filename: string) {
  selected.value = filename;
  // 切换文件：清掉上一次的原始内容；诊断优先走缓存（命中即直接展示）
  rawContent.value = "";
  showRaw.value = false;
  const cached = readDiagCache(props.instanceId, filename);
  diagnosis.value = cached;
}
</script>

<template>
  <div class="crash-analyzer">
    <div v-if="loading" class="crash-loading">
      <van-loading size="20" />
      <span>{{ $t("crash-analyzer.scanning") }}</span>
    </div>

    <div v-else-if="logs.length === 0" class="crash-empty">
      <IconBug />
      <p>{{ $t("crash-analyzer.no-reports") }}</p>
      <span class="crash-empty-hint">{{ $t("crash-analyzer.normal-exit-hint") }}</span>
    </div>

    <div v-else class="crash-body">
      <!-- 日志列表 -->
      <div class="crash-log-list">
        <div class="crash-log-header">
          <span class="crash-log-title">{{ $t("crash-analyzer.crash-report") }}</span>
          <span class="crash-log-count">{{ $t("crash-analyzer.count-suffix", { p1: logs.length }) }}</span>
        </div>
        <div class="crash-log-items">
          <div
            v-for="l in logs"
            :key="l.filename"
            class="crash-log-item"
            :class="{ active: selected === l.filename }"
          >
            <button class="crash-log-select" @click="handleSelect(l.filename)">
              <div class="crash-log-icon">
                <IconFile v-if="l.kind === 'crash'" />
                <IconAlertCircle v-else />
              </div>
              <div class="crash-log-info">
                <div class="crash-log-name text-ellipsis">{{ l.filename }}</div>
                <div class="crash-log-meta">
                  <span>{{ l.kind === "crash" ? $t('crash-analyzer.crash-report') : $t('crash-analyzer.jvm-log') }}</span>
                  <span>·</span>
                  <span>{{ fmtSize(l.size) }}</span>
                  <span>·</span>
                  <span>{{ fmtTime(l.modified) }}</span>
                </div>
              </div>
            </button>
            <button
              class="crash-log-del"
              :title="$t('common.delete')" :aria-label="$t('common.delete')"
              @click="deleteLog(l.filename)"
            >
              <IconTrash />
            </button>
          </div>
        </div>
      </div>

      <!-- 分析结果 -->
      <div class="crash-result">
        <div v-if="analyzing" class="crash-analyzing">
          <van-loading size="20" />
          <span>{{ $t("crash-analyzer.analyzing") }}</span>
        </div>

        <!-- 已出诊断：优先于「分析」按钮展示，否则点完按钮结果永远不显示 -->
        <div v-else-if="diagnosis" class="crash-diagnosis">
          <!-- 严重度 + 置信度标签 -->
          <div class="crash-severity-row">
            <div
              class="crash-severity"
              :style="{ background: severityBg(diagnosis.severity), color: severityColor(diagnosis.severity) }"
            >
              <span class="crash-severity-dot" :style="{ background: severityColor(diagnosis.severity) }" />
              <span class="crash-severity-text">{{ severityLabel(diagnosis.severity) }}</span>
            </div>
            <div class="crash-confidence">
              {{ confidenceLabel(diagnosis.confidence) }}（{{ diagnosis.confidence }}%）
            </div>
          </div>

          <!-- 标题 -->
          <h3 class="crash-title">{{ diagnosis.title }}</h3>

          <!-- 原因 -->
          <div class="crash-section">
            <div class="crash-section-label">{{ $t("crash-analyzer.cause") }}</div>
            <p class="crash-reason">{{ diagnosis.reason }}</p>
          </div>

          <!-- 摘录 -->
          <div v-if="diagnosis.excerpt" class="crash-section">
            <div class="crash-section-label">{{ $t("crash-analyzer.key-info") }}<app-button plain size="tiny" @click="copyExcerpt">
                <IconCopy />
              </app-button>
            </div>
            <p class="crash-excerpt">{{ diagnosis.excerpt }}</p>
          </div>

          <!-- 受影响模组 -->
          <div v-if="diagnosis.affected_mods.length" class="crash-section">
            <div class="crash-section-label">{{ $t("crash-analyzer.affected-mods") }}</div>
            <div class="crash-mods">
              <span v-for="m in diagnosis.affected_mods" :key="m" class="crash-mod">{{ m }}</span>
            </div>
          </div>

          <!-- 建议 -->
          <div class="crash-section">
            <div class="crash-section-label">{{ $t("crash-analyzer.fix-suggestion") }}</div>
            <p class="crash-advice">{{ diagnosis.advice }}</p>
          </div>

          <!-- 其他可能原因（引擎会收集全部命中的规则，不只是主因） -->
          <div v-if="otherCauses.length" class="crash-section">
            <div class="crash-section-label">{{ $t("crash-analyzer.other-causes", { p1: otherCauses.length }) }}</div>
            <div class="crash-other-causes">
              <div v-for="c in otherCauses" :key="c.id" class="crash-other-cause">
                <div class="crash-other-head">
                  <span
                    class="crash-other-tag"
                    :style="{ background: severityBg(c.severity), color: severityColor(c.severity) }"
                  >
                    {{ severityLabel(c.severity) }}
                  </span>
                  <span class="crash-other-title">{{ c.title }}</span>
                  <span class="crash-other-conf">{{ c.confidence }}%</span>
                </div>
                <p class="crash-other-advice">{{ c.advice }}</p>
              </div>
            </div>
          </div>

          <!-- 环境信息（Minecraft / Java / 内存 / 显卡…） -->
          <div v-if="diagnosis.details.length" class="crash-section">
            <div class="crash-section-label">{{ $t("crash-analyzer.environment") }}</div>
            <div class="crash-details">
              <div v-for="d in diagnosis.details" :key="d.key" class="crash-detail">
                <span class="crash-detail-key">{{ d.key }}</span>
                <span class="crash-detail-value text-ellipsis">{{ d.value }}</span>
              </div>
            </div>
          </div>

          <!-- 关键堆栈 -->
          <div v-if="diagnosis.stacktrace.length" class="crash-section">
            <div class="crash-section-label">{{ $t("crash-analyzer.key-stack") }}<app-button plain size="tiny" @click="copyStack">
                <IconCopy />
              </app-button>
            </div>
            <div class="crash-stack">
              <div v-for="(f, i) in diagnosis.stacktrace" :key="i" class="crash-stack-line">
                {{ f }}
              </div>
            </div>
          </div>

          <!-- 操作 -->
          <div class="crash-actions">
            <app-button plain size="small" @click="analyze(true)">
              <IconRefresh />{{ $t("crash-analyzer.reanalyze") }}</app-button>
            <app-button plain size="small" @click="toggleRaw">
              <IconChevronRight v-if="!showRaw" />
              <IconChevronDown v-else />{{ $t("crash-analyzer.view-raw") }}</app-button>
          </div>

          <!-- 原始内容：点「查看原始报告」即自动加载，无需再单独点一次 -->
          <div v-if="showRaw" class="crash-raw">
            <van-loading v-if="!rawContent" size="16" />
            <template v-else>
              <pre>{{ rawContent }}</pre>
              <app-button plain size="small" @click="copyRaw">
                <IconCopy />{{ $t("crash-analyzer.copy-all") }}</app-button>
            </template>
          </div>
        </div>

        <!-- 尚未分析：显示「分析此崩溃报告」按钮（selected 为真但还没出结果） -->
        <div v-else class="crash-prompt">
          <app-button type="primary" size="small" :disabled="analyzing || !selected" @click="analyze()">{{ $t("crash-analyzer.analyze-this") }}</app-button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.crash-analyzer {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 16px;
}

/* 磨砂玻璃适配：卡片与卡片式按钮统一加上背景模糊，跟随主题 --glass-blur */
.crash-result,
.crash-log-item,
.crash-excerpt,
.crash-raw,
.crash-advice {
  backdrop-filter: blur(var(--glass-blur, 8px));
  -webkit-backdrop-filter: blur(var(--glass-blur, 8px));
}

.crash-loading,
.crash-analyzing,
.crash-prompt {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px 0;
  color: var(--text-3);
  font-size: 14px;
}

.crash-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 60px 0;
  color: var(--text-3);
  font-size: 14px;
}
.crash-empty p {
  margin: 0;
  font-size: 15px;
  color: var(--text-2);
}
.crash-empty-hint {
  font-size: 12px;
  color: var(--text-3);
}

.crash-body {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 16px;
  height: 100%;
  min-height: 0;
}

/* 日志列表 */
.crash-log-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
}
.crash-log-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.crash-log-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-1);
}
.crash-log-count {
  font-size: 11px;
  color: var(--text-3);
  background: var(--panel);
  padding: 2px 8px;
  border-radius: 20px;
}
.crash-log-items {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 360px;
  overflow-y: auto;
}
.crash-log-item {
  display: flex;
  align-items: center;
  gap: 10px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--panel);
  transition: all 0.12s;
}
/* 选择区：撑满条目并承载 padding，使条目内除删除键外的区域都可点击 */
.crash-log-select {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border: none;
  background: transparent;
  color: inherit;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
}
.crash-log-item:hover {
  background: var(--panel-hover);
}
.crash-log-item.active {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.crash-log-icon {
  flex-shrink: 0;
  color: var(--text-3);
  font-size: 16px;
}
.crash-log-item.active .crash-log-icon {
  color: var(--accent);
}
.crash-log-info {
  flex: 1;
  min-width: 0;
}
.crash-log-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
}
.crash-log-meta {
  font-size: 11px;
  color: var(--text-3);
  display: flex;
  gap: 4px;
  margin-top: 2px;
}
.crash-log-del {
  flex-shrink: 0;
  margin-right: 12px;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--text-3);
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  font-size: 13px;
  opacity: 0;
  transition: all 0.12s;
}
.crash-log-item:hover .crash-log-del {
  opacity: 1;
}
.crash-log-del:hover {
  color: #e5534b;
  background: rgba(229, 83, 75, 0.1);
}

/* 分析结果 */
/* 分析结果卡片 */
.crash-result {
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow-y: auto;
  min-width: 0;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 16px;
}

.crash-severity-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.crash-severity {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 20px;
  font-size: 12px;
  font-weight: 600;
  width: fit-content;
}
.crash-confidence {
  font-size: 11px;
  color: var(--text-3);
}

/* 其他可能原因 */
.crash-other-causes {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.crash-other-cause {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 12px;
  background: var(--panel);
}
.crash-other-head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.crash-other-tag {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 20px;
  flex-shrink: 0;
}
.crash-other-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
  flex: 1;
  min-width: 0;
}
.crash-other-conf {
  font-size: 11px;
  color: var(--text-3);
  flex-shrink: 0;
}
.crash-other-advice {
  margin: 6px 0 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-2);
}

/* 运行环境 */
.crash-details {
  display: flex;
  flex-direction: column;
  gap: 2px;
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: hidden;
  background: var(--panel);
}
.crash-detail {
  display: flex;
  gap: 10px;
  padding: 6px 12px;
  font-size: 12px;
}
.crash-detail:nth-child(odd) {
  background: rgba(128, 128, 128, 0.04);
}
.crash-detail-key {
  flex-shrink: 0;
  width: 140px;
  color: var(--text-3);
}
.crash-detail-value {
  color: var(--text-1);
  min-width: 0;
}

/* 关键堆栈 */
.crash-stack {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 12px;
  background: var(--panel);
  max-height: 220px;
  overflow: auto;
  font-family: "Cascadia Code", Consolas, monospace;
}
.crash-stack-line {
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-2);
  white-space: pre-wrap;
  word-break: break-all;
}
.crash-stack-line:first-child {
  color: var(--text-1);
  font-weight: 600;
}
.crash-severity-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.crash-title {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-1);
  margin: 0;
  line-height: 1.3;
}

.crash-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.crash-section-label {
  font-size: 11px;
  font-weight: 700;
  color: var(--text-3);
  letter-spacing: 0.5px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.crash-reason {
  margin: 0;
  font-size: 14px;
  color: var(--text-2);
  line-height: 1.6;
}
.crash-excerpt {
  margin: 0;
  font-size: 13px;
  color: var(--text-1);
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px 12px;
  font-family: "Cascadia Code", Consolas, monospace;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
.crash-mods {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.crash-mod {
  font-size: 12px;
  padding: 3px 8px;
  border-radius: 6px;
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}
.crash-advice {
  margin: 0;
  font-size: 14px;
  color: var(--text-1);
  line-height: 1.7;
  background: rgba(122, 208, 138, 0.08);
  border: 1px solid rgba(122, 208, 138, 0.18);
  border-radius: 10px;
  padding: 12px 14px;
}

.crash-actions {
  display: flex;
  gap: 8px;
  padding-top: 4px;
}

.crash-raw {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px;
  overflow: auto;
  max-height: 300px;
}
.crash-raw pre {
  margin: 0;
  font-size: 12px;
  font-family: "Cascadia Code", Consolas, monospace;
  line-height: 1.5;
  color: var(--text-2);
  white-space: pre-wrap;
  word-break: break-word;
}

/*
 * 手机（窄容器）单栏。
 *
 * 桌面是「280px 报告列表 + 结果」两栏，而这一页容器只有 384px：
 * 结果列被压到 70px 上下，标题、正文全折成一列一两个字。
 * 竖屏改成上下堆叠——列表限高在上，结果占满下方。
 * 触发条件用容器查询：界面缩放走 zoom，媒体查询的宽度不可靠。
 */
@container page (max-width: 720px) {
  .crash-body {
    grid-template-columns: 1fr;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 12px;
  }
  .crash-log-items {
    max-height: 168px;
  }
  /* 删除键原来是 hover 才显形，触屏没有 hover → 常显，并放大到手指点得到 */
  .crash-log-del {
    opacity: 1;
    width: 40px;
    height: 40px;
    margin-right: 4px;
  }
  .crash-result {
    padding: 12px;
    gap: 12px;
  }
  .crash-title {
    font-size: 17px;
  }
  /* 环境信息：键值并排时值只剩几十像素，改成上下两行并允许折行 */
  .crash-detail {
    flex-direction: column;
    gap: 2px;
  }
  .crash-detail-key {
    width: auto;
  }
  .crash-detail-value {
    white-space: normal;
    overflow: visible;
    text-overflow: clip;
    word-break: break-all;
  }
  .crash-stack {
    max-height: 180px;
  }
  .crash-actions {
    flex-wrap: wrap;
  }
}
</style>
