<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { listen } from "@tauri-apps/api/event";

const visible = ref(false);
const step = ref("");
const progress = ref(0);
const done = ref(false);

let unlistenProgress: (() => void) | null = null;
let unlistenLog: (() => void) | null = null;
let unlistenExit: (() => void) | null = null;
let doneTimer: ReturnType<typeof setTimeout> | null = null;

onMounted(async () => {
  unlistenProgress = await listen<{ step: string; progress: number }>("launch://progress", (e) => {
    if (done.value) return;
    visible.value = true;
    step.value = e.payload.step;
    progress.value = e.payload.progress;
  });

  unlistenLog = await listen<{ line: string }>("launch://log", () => {
    if (!visible.value || done.value) return;
    done.value = true;
    step.value = "启动成功";
    progress.value = 100;
    doneTimer = setTimeout(() => {
      visible.value = false;
      done.value = false;
    }, 2000);
  });

  unlistenExit = await listen("launch://exit", () => {
    visible.value = false;
    done.value = false;
    if (doneTimer) { clearTimeout(doneTimer); doneTimer = null; }
  });
});

onBeforeUnmount(() => {
  unlistenProgress?.();
  unlistenLog?.();
  unlistenExit?.();
  if (doneTimer) clearTimeout(doneTimer);
});
</script>

<template>
  <Transition name="lp-slide">
    <div v-if="visible" class="launch-progress">
      <div class="lp-bar">
        <div class="lp-fill" :class="{ done }" :style="{ width: progress + '%' }"></div>
      </div>
      <div class="lp-info">
        <span class="lp-step">{{ step }}</span>
        <span class="lp-pct">{{ progress }}%</span>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.launch-progress {
  position: fixed;
  /* 底部导航是 fixed + z-index:100。这条提示原来 bottom:20px + z-index:9999
     且没有 pointer-events:none —— 于是它盖住底栏，把「首页/实例/下载」的点击全吞了
     （移动端断点现在横屏也命中，所以横屏同样中招）。
     后来导航栏支持摆到左侧（App.vue 的 --nav-inset-*）：
     · 底栏时 12px + 栏高 + 安全区 = 原来的 68px+安全区，数值不变；
     · 左栏时 left 加半格宽，让浮层仍在**内容区**里水平居中，而不是偏到导航栏底下。 */
  bottom: calc(12px + var(--nav-inset-bottom, 0px));
  pointer-events: none;
  left: calc(50% + var(--nav-inset-left, 0px) / 2);
  transform: translateX(-50%);
  width: 360px;
  max-width: 90vw;
  background: var(--panel, #1e2230);
  border: 1px solid var(--border, rgba(255,255,255,0.08));
  border-radius: 12px;
  padding: 14px 18px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  z-index: 9999;
}
.lp-bar {
  height: 4px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 2px;
  overflow: hidden;
  margin-bottom: 8px;
}
.lp-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--accent, #e89a4b), var(--accent-deep, #d97f33));
  border-radius: 2px;
  transition: width 0.3s ease;
}
.lp-fill.done {
  background: linear-gradient(90deg, #4ec9a0, #2b9a74);
}
.lp-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 13px;
}
.lp-step {
  color: var(--text-2, #c6c8d2);
}
.lp-pct {
  color: var(--text-3, #8b8e9c);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.lp-slide-enter-active,
.lp-slide-leave-active {
  transition: all 0.3s ease;
}
.lp-slide-enter-from,
.lp-slide-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(20px);
}
</style>
