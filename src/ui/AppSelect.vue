<script setup lang="ts">
/**
 * 选择器（naive `n-select` 的 Vant 形态）：**只读输入框 + 底部滚轮弹层**。
 *
 * 为什么不是「换皮」而是换交互：手机上点开一个下拉面板既难点又挡住页面，
 * 滚轮/列表弹层才是手机的选择方式（和 Picker、ActionSheet 同源，视觉与手速一致）。
 * 契约与 naive 对齐：`v-model:value` / `:options="[{label, value}]"` / `:placeholder`，
 * 所以调用点只换标签名。
 *
 * 弹层走 AppPopup → 自动挂到 #van-layer（跳出 .content 的层叠上下文、跟随 zoom）。
 */
import { computed, ref } from "vue";
import { Field as VanField, Picker as VanPicker } from "vant";
import AppPopup from "./AppPopup.vue";
import { t as $t } from "../i18n";

export interface AppSelectOption {
  label: string;
  value: any;
  disabled?: boolean;
}

const props = withDefaults(
  defineProps<{
    value?: any;
    options?: AppSelectOption[];
    placeholder?: string;
    disabled?: boolean;
    size?: "small" | "medium" | "large";
    /** naive 的顶部标题，缺省用 placeholder */
    title?: string;
  }>(),
  { options: () => [], size: "medium" }
);

const emit = defineEmits<{
  (e: "update:value", v: any): void;
  (e: "update:show", v: boolean): void;
}>();

const show = ref(false);

/** 允许按 label 传值（有些调用点存的是 label） */
const currentLabel = computed(() => {
  const hit = props.options.find((o) => o.value === props.value);
  return hit ? hit.label : "";
});

const columns = computed(() => [
  {
    values: props.options.map((o) => ({
      text: o.disabled ? `${o.label}（${$t("instance-content.disable")}）` : o.label,
      value: o.value,
      disabled: !!o.disabled,
    })),
  },
]);

const selectedValues = ref<(string | number)[]>([]);

function open() {
  if (props.disabled) return;
  selectedValues.value = props.value == null ? [] : [props.value];
  show.value = true;
}

function onConfirm({ selectedValues: picked }: { selectedValues: (string | number)[] }) {
  const v = picked?.[0];
  if (v !== undefined && v !== null) emit("update:value", v);
  show.value = false;
}
</script>

<template>
  <div class="app-select" :class="`app-select-${size}`">
    <van-field
      :model-value="currentLabel"
      :placeholder="placeholder"
      :disabled="disabled"
      is-link
      readonly
      :input-align="'left'"
      class="app-select-field"
      @click="open"
    />
    <app-popup :show="show" position="bottom" round @update:show="(v: boolean) => { show = v; emit('update:show', v); }">
      <div class="picker-box">
        <div class="picker-bar">
          <button class="picker-cancel" @click="show = false">{{ $t("common.cancel") }}</button>
          <span class="picker-title">{{ title || placeholder || "" }}</span>
          <button class="picker-ok" @click="onConfirm({ selectedValues })">{{ $t("file-manager.ok") }}</button>
        </div>
        <van-picker
          v-model="selectedValues"
          :columns="columns"
          :show-toolbar="false"
          @confirm="onConfirm"
          @cancel="show = false"
        />
      </div>
    </app-popup>
  </div>
</template>

<style scoped>
.app-select-field {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 8px 12px;
  cursor: pointer;
}
.app-select-field :deep(.van-field__control) {
  font-size: 14px;
  color: var(--text-1);
  cursor: pointer;
}
.app-select-small .app-select-field {
  padding: 5px 10px;
}
.app-select-small :deep(.van-field__control) {
  font-size: 13px;
}
.picker-box {
  display: flex;
  flex-direction: column;
  padding-bottom: env(safe-area-inset-bottom, 0px);
}
.picker-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
}
.picker-title {
  flex: 1;
  min-width: 0;
  text-align: center;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.picker-cancel,
.picker-ok {
  border: none;
  background: transparent;
  font-family: inherit;
  font-size: 14px;
  min-height: 36px;
  padding: 0 8px;
  cursor: pointer;
}
.picker-cancel {
  color: var(--text-3);
}
.picker-ok {
  color: var(--accent);
  font-weight: 600;
}
</style>
