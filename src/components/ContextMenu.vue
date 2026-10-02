<script setup lang="ts">
/**
 * 操作菜单（手机形态）：**底部面板**。
 *
 * 这里曾经是跟随手指/鼠标坐标弹出的浮层菜单（桌面右键菜单），在手机上问题很实在：
 * 手指按下的位置就是菜单弹出的位置，前两三项正好被手指和系统手势条压着；
 * 菜单又从锚点向外展开，边缘还要算翻折与视口夹取。
 * 现在统一成从底部升起的面板 —— 条目是整行大按钮，拇指顺手、永远不会被手挡住。
 *
 * 调用方 API 不变（show / x / y / items / @close），x、y 仅作兼容保留。
 */
import { Popup as VanPopup } from "vant";
import type { ContextMenuItem } from "../types";
import { IconCheck } from "./icons";

// x / y 保留在 props 里是为了不改调用点（旧坐标在手机形态下没有意义）
const props = defineProps<{
  show: boolean;
  x?: number;
  y?: number;
  items: ContextMenuItem[];
}>();
void props;

const emit = defineEmits<{ (e: "close"): void }>();

function pick(it: ContextMenuItem) {
  if (it.disabled || it.sep) return;
  emit("close");
  it.action?.();
}
</script>

<template>
  <van-popup
    :show="show"
    position="bottom"
    round
    teleport="#van-layer"
    @update:show="(v: boolean) => { if (!v) emit('close'); }"
  >
    <div class="ctx">
      <template v-for="it in items" :key="it.key">
        <div v-if="it.sep" class="ctx-sep"></div>
        <button
          v-else
          class="ctx-item"
          :class="{ danger: it.danger }"
          :disabled="it.disabled"
          @click="pick(it)"
        >
          <span class="ctx-icon">
            <component :is="it.icon" v-if="it.icon" />
            <IconCheck v-else />
          </span>
          <span class="ctx-label">{{ it.label }}</span>
        </button>
      </template>
    </div>
  </van-popup>
</template>

<style scoped>
.ctx {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px));
  max-height: 72vh;
  overflow-y: auto;
}
.ctx-item {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  /* 触控底线：面板里的每一项都是手指目标 */
  min-height: 50px;
  padding: 0 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--panel);
  color: var(--text-1);
  font-size: 15px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
}
.ctx-item:disabled {
  opacity: 0.45;
}
.ctx-item.danger {
  color: #e5534b;
  border-color: rgba(229, 83, 75, 0.4);
}
.ctx-icon {
  width: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--text-3);
}
.ctx-icon svg {
  width: 18px;
  height: 18px;
}
.ctx-item.danger .ctx-icon {
  color: #e5534b;
}
.ctx-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ctx-sep {
  height: 1px;
  margin: 2px 4px;
  background: var(--border);
}
</style>
