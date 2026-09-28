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
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NButton, NModal } from "naive-ui";
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

/** 确认「切换为推荐渲染器并启动」 */
async function switchAndLaunch() {
  const g = guard.value;
  if (!g || switching.value) return;
  switching.value = true;
  try {
    await instances.patch({ id: g.instanceId, renderer: g.recommended });
    instances.pendingRendererGuard = null;
    notifySuccess(`已把该实例切换为 ${g.recommendedName}，正在启动`);
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
    notifySuccess(`已把该实例切换为 ${it.recommended_name}，下次启动生效`);
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
  <!-- 启动前确认：秒级判断，不用等游戏跑起来 -->
  <NModal
    :show="!!guard"
    preset="card"
    :style="{ width: 'min(560px, 92vw)' }"
    :closable="true"
    @update:show="(v: boolean) => (v ? null : launchAnyway())"
  >
    <template #header>
      <div class="rh-header">
        <span class="rh-badge">渲染器提醒</span>
        <span class="rh-title">这个渲染器在该版本上大概率不行</span>
      </div>
    </template>

    <div v-if="guard" class="rh-body">
      <div class="rh-switch">
        <span class="rh-chip rh-chip-used">{{ guard.usedName }}（当前会用）</span>
        <span class="rh-arrow">→</span>
        <span class="rh-chip rh-chip-rec">{{ guard.recommendedName }}（推荐）</span>
      </div>

      <p class="rh-advice">
        本实例是 MC <b>{{ guard.mcVersion }}</b
        >。26.x 与 1.x 要的渲染器是反的：1.x 用 MobileGlues 会在加载着色器时崩在启动阶段
        （画面全白）；26.x 用 GL4ES 则渲染不出来。
      </p>

      <p class="rh-advice">
        要不要先把这个实例切成 <b>{{ guard.recommendedName }}</b> 再启动？（只影响这个实例）
      </p>

      <p class="rh-advice rh-advice-dim">
        注意：点「切换…并启动」会把该实例的渲染器**固定**为
        {{ guard.recommendedName }}，也就是不再跟随全局设置；想恢复跟随，
        去实例设置里改回「跟随全局」即可。
      </p>
    </div>

    <template #footer>
      <div class="rh-footer">
        <NButton size="small" quaternary @click="launchAnyway">
          仍用 {{ guard?.usedName }} 启动
        </NButton>
        <NButton size="small" type="primary" :loading="switching" @click="switchAndLaunch">
          切换为 {{ guard?.recommendedName }} 并启动
        </NButton>
      </div>
    </template>
  </NModal>

  <!-- 启动后：日志里发现了渲染器失败证据 -->
  <NModal
    :show="show"
    preset="card"
    :style="{ width: 'min(560px, 92vw)' }"
    :closable="true"
    @update:show="(v: boolean) => (v ? (show = v) : dismiss())"
  >
    <template #header>
      <div class="rh-header">
        <span class="rh-badge">渲染器建议</span>
        <span class="rh-title">这个实例换个渲染器会更稳</span>
      </div>
    </template>

    <div v-if="issue" class="rh-body">
      <p class="rh-reason">{{ issue.reason }}</p>

      <div class="rh-switch">
        <span class="rh-chip rh-chip-used">{{ issue.used_name }}（当前）</span>
        <span class="rh-arrow">→</span>
        <span class="rh-chip rh-chip-rec">{{ issue.recommended_name }}（推荐）</span>
      </div>

      <p class="rh-advice">
        本实例是 MC <b>{{ issue.mc_version }}</b
        >，推荐的渲染器是 <b>{{ issue.recommended_name }}</b>。点击下面的按钮只会把
        <b>这个实例</b>切过去（其它实例不受影响），下次启动生效。
      </p>

      <details v-if="issue.evidence.length" class="rh-evidence">
        <summary>查看日志证据（{{ issue.evidence.length }} 条）</summary>
        <pre>{{ issue.evidence.join("\n") }}</pre>
      </details>
    </div>

    <template #footer>
      <div class="rh-footer">
        <NButton size="small" quaternary @click="dismiss">暂不切换</NButton>
        <NButton size="small" type="primary" :loading="switching" @click="doSwitch">
          切换为 {{ issue?.recommended_name }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.rh-header {
  display: flex;
  align-items: center;
  gap: 10px;
}
.rh-badge {
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(255, 176, 32, 0.16);
  color: #ffb020;
  border: 1px solid rgba(255, 176, 32, 0.35);
  white-space: nowrap;
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
</style>
