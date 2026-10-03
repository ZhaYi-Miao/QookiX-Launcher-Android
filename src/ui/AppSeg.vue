<script setup lang="ts">
/**
 * 分段控件（手机上替代 naive 的 radio-button-group）。
 *
 * 为什么自己写而不继续用库：naive 的按钮组把高度写死成「单行高度」，
 * 我们一旦允许换行就得再补一条 `height: auto` 的补丁；而分段控件在手机上就两种
 * 形态——等宽铺满一行，或者折成两行——完全可以用 flex 表达清楚，
 * 不必背一个库的布局假设。项目里设置页已有的 `.seg` 也是同一套视觉。
 *
 * 用法：`<app-seg v-model:value="mode" :options="[{value:'a',label:'A'}]" />`
 */
withDefaults(
  defineProps<{
    value?: string | number | null;
    options: { value: string | number; label: string }[];
    disabled?: boolean;
    /** small 用于卡片内嵌（高度 34），默认 40 适合独立成行的选择 */
    size?: "small" | "medium";
  }>(),
  { size: "medium" }
);

const emit = defineEmits<{ (e: "update:value", v: string | number): void }>();
</script>

<template>
  <div class="seg" :class="`seg-${size}`">
    <button
      v-for="o in options"
      :key="String(o.value)"
      class="seg-btn"
      :class="{ on: o.value === value }"
      :disabled="disabled"
      @click="emit('update:value', o.value)"
    >
      {{ o.label }}
    </button>
  </div>
</template>

<style scoped>
.seg {
  display: flex;
  gap: 4px;
  padding: 3px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--panel);
  /* 窄屏放不下就折行（不用库就不存在「写死高度被第二行压住」的坑） */
  flex-wrap: wrap;
}
.seg-btn {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 34px;
  padding: 6px 12px;
  border: none;
  border-radius: 9px;
  background: transparent;
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
}
.seg-medium .seg-btn {
  min-height: 40px;
  font-size: 14px;
}
.seg-btn.on {
  background: var(--accent);
  color: #1a1208;
  font-weight: 600;
}
.seg-btn:disabled {
  opacity: 0.45;
}
</style>
