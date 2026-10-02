<script setup lang="ts">
/**
 * 开关（Vant Switch 的 naive 兼容层）：保持 `:value` + `@update:value` 契约。
 *
 * naive 的 Switch 默认很小（实测约 20px 高），手指上不好点；Vant 24px 起步，
 * 且自带按压反馈与禁用态。size 档位做了映射（small 20 / medium 24 / large 28）。
 */
import { Switch as VanSwitch } from "vant";

const props = withDefaults(
  defineProps<{
    value?: boolean;
    disabled?: boolean;
    size?: "small" | "medium" | "large";
    loading?: boolean;
  }>(),
  { value: false, size: "medium" }
);

const emit = defineEmits<{ (e: "update:value", v: boolean): void }>();

/** naive 的三档尺寸 → Vant 的像素宽度 */
const px = () => ({ small: "20px", medium: "24px", large: "28px" })[props.size] ?? "24px";
</script>

<template>
  <van-switch
    :model-value="!!value"
    :disabled="disabled"
    :loading="loading"
    :size="px()"
    @update:model-value="(v: boolean) => emit('update:value', v)"
  />
</template>
