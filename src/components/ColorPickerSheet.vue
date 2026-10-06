<script setup lang="ts">
/**
 * 主题色取色面板（底部弹层）。
 *
 * 为什么不用 `<input type="color">`：安卓 WebView 弹的是系统「选择颜色」对话框 ——
 * 红/黄/蓝/绿/品红/青/黑/白八个原色，「自定义」还得再点进去一层，
 * 跟主题色这场景完全对不上（黑/白当主题色没有意义），样式也没法跟深浅色主题统一。
 * 这里自己用 HSL 三个滑杆做：轨道本身就是色相环 / 饱和度 / 明度渐变，拖到哪就是哪。
 *
 * 只改面板内部的颜色，点「保存」才写回设置 —— 拖动过程中不落盘。
 */
import { computed, ref, watch } from "vue";
import { t as $t } from "../i18n";
import { hexToHsl, hslToHex } from "../theme";
import AppSheet from "../ui/AppSheet.vue";
import AppSlider from "../ui/AppSlider.vue";
import AppButton from "../ui/AppButton.vue";

const props = defineProps<{ show: boolean; value: string }>();
const emit = defineEmits<{
  (e: "update:show", v: boolean): void;
  (e: "confirm", hex: string): void;
}>();

const h = ref(30);
const s = ref(72);
const l = ref(60);
/** 打开时的原始取值：没动滑杆就原样返回旧颜色，免得 HSL 往返取整把 #e89a4b 磨成 #e8994a */
const opened = ref({ h: 30, s: 72, l: 60 });

// 每次打开都对齐到当前颜色（上次拖到一半取消掉的不该残留到下次）
watch(
  () => props.show,
  (open) => {
    if (!open) return;
    const hsl = hexToHsl(props.value);
    h.value = Math.round(hsl.h);
    s.value = Math.round(hsl.s);
    l.value = Math.round(hsl.l);
    opened.value = { h: h.value, s: s.value, l: l.value };
  },
  { immediate: true }
);

const touched = computed(
  () => h.value !== opened.value.h || s.value !== opened.value.s || l.value !== opened.value.l
);
const hex = computed(() => (touched.value ? hslToHex(h.value, s.value, l.value) : props.value));

/* 三条轨道的底色就是滑块取值本身的渐变，所见即所得 */
const HUE_RAIL = "linear-gradient(90deg,#ff0000,#ffff00,#00ff00,#00ffff,#0000ff,#ff00ff,#ff0000)";
const satRail = computed(
  () => `linear-gradient(90deg,hsl(${h.value},0%,${l.value}%),hsl(${h.value},100%,${l.value}%))`
);
const lightRail = computed(
  () =>
    `linear-gradient(90deg,hsl(${h.value},${s.value}%,0%),hsl(${h.value},${s.value}%,50%),hsl(${h.value},${s.value}%,100%))`
);
</script>

<template>
  <app-sheet
    :show="show"
    :title="$t('settings.custom-color')"
    @update:show="(v: boolean) => emit('update:show', v)"
  >
    <div class="cp-preview">
      <span class="cp-chip" :style="{ background: hex }"></span>
      <span class="cp-hex">{{ hex.toUpperCase() }}</span>
    </div>

    <div class="cp-row">
      <span class="cp-label">{{ $t("settings.hue") }}</span>
      <app-slider
        class="cp-rail"
        :value="h"
        :min="0"
        :max="360"
        :style="{ '--rail': HUE_RAIL }"
        @update:value="(v: number) => (h = v)"
      />
    </div>
    <div class="cp-row">
      <span class="cp-label">{{ $t("settings.saturation") }}</span>
      <app-slider
        class="cp-rail"
        :value="s"
        :min="0"
        :max="100"
        :style="{ '--rail': satRail }"
        @update:value="(v: number) => (s = v)"
      />
    </div>
    <div class="cp-row">
      <span class="cp-label">{{ $t("settings.lightness") }}</span>
      <app-slider
        class="cp-rail"
        :value="l"
        :min="0"
        :max="100"
        :style="{ '--rail': lightRail }"
        @update:value="(v: number) => (l = v)"
      />
    </div>

    <template #footer>
      <div class="cp-acts">
        <app-button block @click="emit('update:show', false)">{{ $t("common.cancel") }}</app-button>
        <app-button block type="primary" @click="emit('confirm', hex)">{{ $t("common.save") }}</app-button>
      </div>
    </template>
  </app-sheet>
</template>

<style scoped>
.cp-preview {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 6px;
}
.cp-chip {
  width: 46px;
  height: 46px;
  border-radius: 14px;
  flex-shrink: 0;
  box-shadow: inset 0 0 0 1px var(--border);
}
.cp-hex {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 16px;
  color: var(--text-1);
}
.cp-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 6px;
}
.cp-label {
  width: 42px;
  flex-shrink: 0;
  font-size: 13px;
  color: var(--text-2);
}
/* 轨道整条铺成渐变、已选部分透明，滑块位置才看得出落在哪个颜色上 */
.cp-rail :deep(.van-slider) {
  background: var(--rail);
}
.cp-rail :deep(.van-slider__bar) {
  background: transparent;
}
/* 圆钮：app-slider 用了 Vant 的 `#button` 插槽，于是 Vant 自带的圆钮不再渲染，
   而它的 `.sl-thumb` 里只有气泡、没有本体（没传 formatTooltip 时就是 0×0）——
   轨道上完全看不出当前值在哪。取色面板最依赖这个位置感，这里先把圆形本体补回来。
   （注意：这其实是 app-slider 全局都有的问题，见本次改动说明。） */
.cp-rail :deep(.sl-thumb) {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.16), 0 1px 4px rgba(0, 0, 0, 0.35);
}
.cp-acts {
  display: flex;
  gap: 10px;
}
</style>
