<script setup lang="ts">
/**
 * 渲染器建议弹窗 —— 两个时机共用一套「实例 + 渲染器」记忆，不会重复打扰：
 *
 * 1. **启动前**（`instances.pendingRendererGuard`，秒级）：会用的渲染器和版本推荐不一致时，
 *    先问一句「切换为推荐渲染器并启动 / 仍用当前渲染器启动」。这是最有价值的一条 ——
 *    不用等游戏跑到一半才发现不行（1.8.9 + MG 会白屏卡住，等系统收掉 Activity 要几十秒）。
 * 2. **启动后**（`launch://state` 的 exited 或回到启动器）：后端 `check_renderer_health`
 *    在启动日志里发现渲染器失败特征**且**当前渲染器不是该版本推荐时，提示一键切换。
 *    兜底那些「启动前看不出来」的情况。
 *
 * 确认后只改**这一个实例**的 renderer 字段。点「暂不/仍用」会在本地记住
 * 「这个实例 + 这个渲染器已经问过」，不再重复问；用户换了渲染器再出问题会重新提示。
 */
import { t as $t } from "../i18n";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Button as VanButton } from "vant";
import AppSheet from "../ui/AppSheet.vue";
import { api } from "../api";
import type { RendererIssue } from "../types";
import { useTasksStore } from "../stores/tasks";
import {
  hasRendererWarned,
  markRendererGuardOk,
  markRendererWarned,
  useInstancesStore,
} from "../stores/instances";
import { notifyError, notifySuccess } from "../composables/notify";

const show = ref(false);
const issue = ref<RendererIssue | null>(null);
const switching = ref(false);

const tasks = useTasksStore();
const instances = useInstancesStore();

/** 启动前那份待确认（由 store.launch 挂上来的） */
const guard = computed(() => instances.pendingRendererGuard);

// 两个弹层别叠在一起：用户点了启动 → 启动前提醒出现时，先把「启动后建议」收掉
watch(guard, (g) => {
  if (g) {
    show.value = false;
    issue.value = null;
  }
});

/** 确认「切换为推荐渲染器并启动」 */
async function switchAndLaunch() {
  const g = guard.value;
  if (!g || switching.value) return;
  switching.value = true;
  try {
    await instances.patch({ id: g.instanceId, renderer: g.recommended });
    instances.pendingRendererGuard = null;
    notifySuccess($t("renderer-hint-dialog.notify-success", { p1: g.recommendedName }));
    await instances.launch(g.instanceId, g.world, g.server, { force: true });
  } catch (e) {
    notifyError(String(e));
  } finally {
    switching.value = false;
  }
}

/**
 * 「仍用当前渲染器启动」：只记「启动前别再问」，**不**记事后的 warned。
 *
 * 事后提示是**有日志证据**的（这次真的炸了），这时还应该再问一次；
 * 用户那时点「暂不切换」才会把这条也一起封掉。
 */
async function launchAnyway() {
  const g = guard.value;
  if (!g) return;
  markRendererGuardOk(g.instanceId, g.used);
  instances.pendingRendererGuard = null;
  try {
    await instances.launch(g.instanceId, g.world, g.server, { force: true });
  } catch (e) {
    notifyError(String(e));
  }
}

/** 已经处理过的「实例:退出码」组合，避免重复检查 */
let handledExit = "";
let timer: ReturnType<typeof setTimeout> | null = null;
/** 「回到启动器」这条路径的节流时间戳 */
let lastResumeCheck = 0;

function present(found: RendererIssue | null) {
  if (!found) return;
  if (hasRendererWarned(found.instance_id, found.used)) return;
  if (show.value || guard.value) return;
  issue.value = found;
  show.value = true;
}

async function check(instanceId: string) {
  try {
    present(await api.checkRendererHealth(instanceId));
  } catch {
    /* 检查失败不打扰用户 */
  }
}

/**
 * 玩家回到启动器时按「最近一次启动的实例」查一次。
 *
 * 为什么需要这条路径：有些渲染器故障会让游戏线程死掉但 JVM 还挂着（1.8.9 + MG 实测就是
 * 白屏卡住），`launch://state` 的 exited 永远不发 —— 只能等用户手动关掉游戏回来。
 */
async function checkLatest() {
  const now = Date.now();
  if (now - lastResumeCheck < 10_000) return;
  lastResumeCheck = now;
  try {
    present(await api.checkRendererHealthLatest());
  } catch {
    /* 忽略 */
  }
}

function onVisibility() {
  if (document.visibilityState === "visible") void checkLatest();
}

watch(
  () => tasks.lastExit,
  (exit) => {
    if (!exit) return;
    const key = `${exit.instanceId}:${exit.code}`;
    if (key === handledExit) return;
    handledExit = key;
    // 非正常退出时崩溃弹窗会先出现，这里晚一点再问，避免两个弹窗叠在一起
    const delay = exit.code && exit.code !== 0 ? 2200 : 600;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void check(exit.instanceId), delay);
  },
  { immediate: true },
);

async function doSwitch() {
  const it = issue.value;
  if (!it || switching.value) return;
  switching.value = true;
  try {
    await instances.patch({ id: it.instance_id, renderer: it.recommended });
    // 记一笔「这个实例 + 这个渲染器已经处理过」：切换后日志里那批旧报错还在，
    // 不记账的话每次打开启动器都会拿旧账再弹一次。
    markRendererWarned(it.instance_id, it.used);
    notifySuccess($t("renderer-hint-dialog.yi-ba-gai-shi-li-qie", { p1: it.recommended_name }));
    show.value = false;
    issue.value = null;
  } catch (e) {
    notifyError(String(e));
  } finally {
    switching.value = false;
  }
}

function dismiss() {
  const it = issue.value;
  if (it) markRendererWarned(it.instance_id, it.used);
  show.value = false;
  issue.value = null;
}

onMounted(() => {
  // 启动器刚起来时也查一次：上一次可能是在「游戏挂住 → 用户强退 → 重启启动器」之后
  void checkLatest();
  document.addEventListener("visibilitychange", onVisibility);
  window.addEventListener("focus", onVisibility);
});

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
  document.removeEventListener("visibilitychange", onVisibility);
  window.removeEventListener("focus", onVisibility);
});
</script>

<template>
  <!-- 启动前确认：秒级判断，不用等游戏跑起来。
       手机形态：底部弹层（这是最后一个 naive 居中卡片弹窗）。 -->
  <app-sheet
    :show="!!guard"
    :title="$t('renderer-hint-dialog.renderer-hint')"
    :mask-closable="true"
    @update:show="(v: boolean) => (v ? null : launchAnyway())"
  >
    <div v-if="guard" class="rh-body">
      <div class="rh-header">
        <span class="rh-title">{{ $t("renderer-hint-dialog.incompatible-title") }}</span>
      </div>
      <div class="rh-switch">
        <span class="rh-chip rh-chip-used">{{ $t("renderer-hint-dialog.current-option", { p1: guard.usedName }) }}</span>
        <span class="rh-arrow">→</span>
        <span class="rh-chip rh-chip-rec">{{ $t("renderer-hint-dialog.recommended-option", { p1: guard.recommendedName }) }}</span>
      </div>

      <p class="rh-advice">
        {{ $t("renderer-hint-dialog.instance-version-a") }}
        <b>{{ guard.mcVersion }}</b>，{{ $t("renderer-hint-dialog.renderer-conflict") }}
      </p>

      <p class="rh-advice">
        {{ $t("renderer-hint-dialog.switch-question") }}
        <b>{{ guard.recommendedName }}</b> {{ $t("renderer-hint-dialog.relaunch-confirm") }}
      </p>

      <p class="rh-advice rh-advice-dim">{{ $t("renderer-hint-dialog.pin-warning", { p1: guard.recommendedName }) }}</p>
    </div>

    <template #footer>
      <div class="rh-footer">
        <van-button block @click="launchAnyway">{{ $t("renderer-hint-dialog.keep-and-launch", { p1: guard?.usedName }) }}</van-button>
        <van-button block type="primary" :loading="switching" @click="switchAndLaunch">{{ $t("renderer-hint-dialog.switch-and-launch", { p1: guard?.recommendedName }) }}</van-button>
      </div>
    </template>
  </app-sheet>

  <!-- 启动后：日志里发现了渲染器失败证据 -->
  <app-sheet
    :show="show"
    :title="$t('renderer-hint-dialog.renderer-suggestion')"
    :mask-closable="true"
    @update:show="(v: boolean) => (v ? (show = v) : dismiss())"
  >
    <div v-if="issue" class="rh-body">
      <div class="rh-header">
        <span class="rh-title">{{ $t("renderer-hint-dialog.suggestion-text") }}</span>
      </div>
      <p class="rh-reason">{{ issue.reason }}</p>

      <div class="rh-switch">
        <span class="rh-chip rh-chip-used">{{ $t("renderer-hint-dialog.current-badge", { p1: issue.used_name }) }}</span>
        <span class="rh-arrow">→</span>
        <span class="rh-chip rh-chip-rec">{{ $t("renderer-hint-dialog.recommended-option", { p1: issue.recommended_name }) }}</span>
      </div>

      <p class="rh-advice">
        {{ $t("renderer-hint-dialog.instance-version-a") }}
        <b>{{ issue.mc_version }}</b>{{ $t("renderer-hint-dialog.recommended-is") }}
        <b>{{ issue.recommended_name }}</b>。{{ $t("renderer-hint-dialog.switch-hint-a") }}
        <b>{{ $t("renderer-hint-dialog.this-instance") }}</b>{{ $t("renderer-hint-dialog.switch-hint-b") }}
      </p>

      <details v-if="issue.evidence.length" class="rh-evidence">
        <summary>{{ $t("renderer-hint-dialog.view-log-evidence", { p1: issue.evidence.length }) }}</summary>
        <pre>{{ issue.evidence.join("\n") }}</pre>
      </details>
    </div>

    <template #footer>
      <div class="rh-footer">
        <van-button block @click="dismiss">{{ $t("renderer-hint-dialog.keep-current") }}</van-button>
        <van-button block type="primary" :loading="switching" @click="doSwitch">
          {{ $t("renderer-hint-dialog.switch-to", { p1: issue?.recommended_name }) }}
        </van-button>
      </div>
    </template>
  </app-sheet>
</template>

<style scoped>
.rh-header {
  display: flex;
  align-items: center;
  gap: 10px;
}
.rh-title {
  font-weight: 600;
}
.rh-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.rh-reason {
  margin: 0;
  line-height: 1.6;
}
.rh-switch {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.rh-chip {
  font-size: 12px;
  padding: 3px 10px;
  border-radius: 999px;
  border: 1px solid transparent;
}
.rh-chip-used {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.14);
}
.rh-chip-rec {
  background: rgba(64, 200, 120, 0.14);
  border-color: rgba(64, 200, 120, 0.4);
  color: #40c878;
}
.rh-arrow {
  opacity: 0.6;
}
.rh-advice {
  margin: 0;
  font-size: 13px;
  opacity: 0.85;
  line-height: 1.6;
}
.rh-advice-dim {
  opacity: 0.7;
  font-size: 12px;
}
.rh-evidence {
  font-size: 12px;
  opacity: 0.8;
}
.rh-evidence pre {
  max-height: 160px;
  overflow: auto;
  margin: 8px 0 0;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.28);
  white-space: pre-wrap;
  word-break: break-all;
  font-size: 11px;
  line-height: 1.5;
}
.rh-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
/* 手机：两个操作键通栏平分（原来是右上角并排的小按钮） */
.rh-footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.rh-footer :deep(.van-button) {
  min-height: 46px;
}
</style>
