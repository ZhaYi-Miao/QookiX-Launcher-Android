<script setup lang="ts">
/**
 * 滑杆（Vant Slider 的 naive 兼容层）：保持 `v-model:value` / `:value` + `@update:value`。
 *
 * 两个必须处理的差异：
 * 1. **气泡文案**：naive 的 `format-tooltip` 能把值显示成「2048 MB」这类带单位的内容，
 *    Vant 的气泡只显示裸数值。toast/单位对设置页很重要，所以这里用 `#button` 插槽
 *    自己画气泡，保留 naive 的 formatter 语义（函数或 undefined）。
 * 2. **手势**：安卓 WebView 会把横向拖动判成页面滚动并发 pointercancel 掐断拖动
 *    （naive 的 n-slider 当年也踩过，全局样式里有 `touch-action: none` 补丁）。
 *    Vant Slider 自带 touch-action 处理，这里仍显式声明一次，避免被全局规则覆盖。
 */
import { Slider as VanSlider } from "vant";

const props = withDefaults(
  defineProps<{
    value?: any;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    /** naive 的 formatter：(value) => string */
    formatTooltip?: (v: number) => string;
    /** naive 的 marks 形如 { 25: '25%' }，Vant 无此能力，忽略但接收 */
    marks?: Record<string | number, string>;
  }>(),
  { value: 0, min: 0, max: 100, step: 1 }
);

const emit = defineEmits<{ (e: "update:value", v: any): void }>();

const text = (v: number) => (props.formatTooltip ? props.formatTooltip(v) : String(v));
</script>

<template>
  <van-slider
    class="app-slider"
    :model-value="value"
    :min="min"
    :max="max"
    :step="step"
    :disabled="disabled"
    @update:model-value="(v: number) => emit('update:value', v)"
  >
    <template #button>
      <div class="sl-thumb">
        <div v-if="formatTooltip" class="sl-bubble">{{ text(value ?? 0) }}</div>
      </div>
    </template>
  </van-slider>
</template>

<style scoped>
.app-slider {
  /* 拖拽必须自己接管触摸，否则横滑被页面滚动抢走（见上方说明） */
  touch-action: none;
  padding: 4px 0;
}
.sl-thumb {
  display: flex;
  flex-direction: column;
  align-items: center;
}
/* 气泡：手机上滑杆常常贴着卡片边缘，宽度自适应 + 不换行 */
.sl-bubble {
  position: absolute;
  bottom: 26px;
  padding: 2px 8px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.78);
  color: #fff;
  font-size: 11px;
  line-height: 16px;
  white-space: nowrap;
  pointer-events: none;
}
</style>
