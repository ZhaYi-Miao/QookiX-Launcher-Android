<script setup lang="ts">
/**
 * 文本输入（Vant Field 的 naive 兼容层）。
 *
 * 存在的意义：让迁移只是「换个标签名」——调用点继续写 `v-model:value`、`:placeholder`、
 * `maxlength`、`@update:value`，不用逐处改成 Vant 的 `v-model`。
 *
 * 三个必须处理的差异（都是迁移时 vue-tsc 报出来的）：
 * 1. `maxlength`/`rows`：模板里写的是静态属性（`maxlength="40"`），Vue 会传**字符串**，
 *    而 Vant 的 prop 类型是 number → 组件内做一次 Number() 转换；
 * 2. `update:value` 的载荷故意标成 any：naive 的 value 是宽松的联合类型，页面里
 *    已有大量 `(v: string) => …` 之类更窄的处理器，声明太严会一次性报几十个错；
 * 3. `focus()` / `select()`：页面（FileManager）拿模板 ref 调这两个方法，
 *    Vant Field 只有 focus()，select() 需要自己转发给内部 input/textarea。
 */
import { ref } from "vue";
import { Field as VanField } from "vant";

withDefaults(
  defineProps<{
    value?: string | number | null;
    type?: "text" | "password" | "textarea";
    placeholder?: string;
    /** 模板里是静态属性（字符串），这里两种都收 */
    maxlength?: string | number;
    rows?: string | number;
    clearable?: boolean;
    disabled?: boolean;
    /** naive 的 show-count：Vant 对应 show-word-limit（仅 textarea 有意义） */
    showCount?: boolean;
    size?: "small" | "medium" | "large";
  }>(),
  { value: "", type: "text", size: "medium" }
);

const emit = defineEmits<{ (e: "update:value", v: any): void }>();

const inner = ref<any>(null);

/** 数字化：静态属性给过来的是字符串 */
const num = (v?: string | number) => (v == null || v === "" ? undefined : Number(v));

defineExpose({
  focus: () => inner.value?.focus?.(),
  select: () => {
    const el = inner.value?.$el?.querySelector?.("input,textarea");
    el?.select?.();
  },
});
</script>

<template>
  <van-field
    ref="inner"
    class="app-input"
    :class="`app-input-${size}`"
    :model-value="value == null ? '' : String(value)"
    :type="type === 'textarea' ? 'textarea' : 'text'"
    :rows="num(rows) ?? 3"
    :maxlength="num(maxlength)"
    :show-word-limit="showCount && type === 'textarea' ? true : undefined"
    :clearable="clearable"
    :disabled="disabled"
    :placeholder="placeholder"
    :input-align="type === 'textarea' ? 'top' : 'left'"
    @update:model-value="(v: string) => emit('update:value', v)"
  />
</template>

<style scoped>
/* 表单里的输入区要看得见边界：Vant Field 默认透明（我们把 cell 背景映射成透明了） */
.app-input {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 8px 12px;
}
.app-input :deep(.van-field__control) {
  font-size: 14px;
  color: var(--text-1);
}
.app-input :deep(.van-field__control::placeholder) {
  color: var(--text-3);
}
/* 尺寸档位：small 用于弹层里的紧凑表单，默认行高给足手指 */
.app-input-small {
  padding: 5px 10px;
}
.app-input-small :deep(.van-field__control) {
  font-size: 13px;
}
.app-input-large :deep(.van-field__control) {
  font-size: 15px;
}
</style>
