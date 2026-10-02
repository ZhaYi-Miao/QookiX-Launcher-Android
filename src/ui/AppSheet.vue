<script setup lang="ts">
/**
 * 统一弹层壳：手机的「底部弹层」形态（Vant Popup），替代原来 20 处 `n-modal preset="card"`。
 *
 * 为什么统一收口：
 * - 手机上居中卡片弹窗是典型的桌面交互（拇指够不到、遮住大半个屏、按钮小）；
 *   底部弹层落在拇指区、内容可滚、关闭键固定，是安卓/ios 的通用形态。
 * - 统一壳还顺带解决了三个老问题：弹层层叠上下文（见 AppPopup 的注释）、
 *   `n-modal` Teleport 到 body 脱离界面缩放、以及每个弹窗各写一套 max-height 补丁
 *   （styles.css 里那段 `.n-card.n-modal` 的手机限高规则可以退休了）。
 *
 * 用法：把 `<n-modal preset="card" :title="t" :show="s" @update:show="...">` 换成
 * `<app-sheet :title="t" :show="s" @update:show="...">`，内部内容原样保留
 * （内层控件后续逐页换成 Vant）。`closable=false` + `mask-closable=false` 用于
 * 「必须显式处理」的流程（如首启准备、服务器安装中）。
 */
import AppPopup from "./AppPopup.vue";
import { IconClose } from "../components/icons";
import { t as $t } from "../i18n";

withDefaults(
  defineProps<{
    show: boolean;
    title?: string;
    /** 右上角关闭键（默认有）。流程性弹窗传 false，只留「取消/知道了」按钮 */
    closable?: boolean;
    /** 点遮罩是否关闭（默认关）。安装中这类不可中断的传 false */
    maskClosable?: boolean;
  }>(),
  { title: "", closable: true, maskClosable: true }
);

const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();
</script>

<template>
  <app-popup
    :show="show"
    position="bottom"
    round
    :close-on-click-overlay="maskClosable"
    @update:show="(v: boolean) => emit('update:show', v)"
  >
    <div class="sheet">
      <header class="sheet-bar">
        <span class="sheet-title">{{ title }}</span>
        <button
          v-if="closable"
          class="sheet-x"
          :aria-label="$t('common.cancel')"
          @click="emit('update:show', false)"
        >
          <IconClose />
        </button>
      </header>
      <div class="sheet-body">
        <slot />
      </div>
      <footer v-if="$slots.footer" class="sheet-foot">
        <slot name="footer" />
      </footer>
    </div>
  </app-popup>
</template>

<style scoped>
.sheet {
  display: flex;
  flex-direction: column;
  /* 内容驱动的自适应高度：矮内容就只占需要的那点，高的最多 88vh 再内部滚动 */
  max-height: 88vh;
  padding-bottom: env(safe-area-inset-bottom, 0px);
}
.sheet-bar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 14px 8px 10px 18px;
  border-bottom: 1px solid var(--border);
}
.sheet-title {
  flex: 1;
  min-width: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--text-1);
  text-align: center;
  /* 标题居中时右侧要给关闭键留位，视觉上才不会偏 */
  margin-right: 34px;
}
.sheet-x {
  flex-shrink: 0;
  width: 34px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 50%;
  background: var(--panel);
  color: var(--text-2);
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
}
.sheet-x:active {
  background: var(--panel-hover);
}
.sheet-x svg {
  width: 16px;
  height: 16px;
}
.sheet-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
  padding: 16px 18px;
  /* 触屏上滚动条不占位（与全局规则一致） */
  scrollbar-width: none;
}
.sheet-body::-webkit-scrollbar {
  display: none;
}
.sheet-foot {
  flex-shrink: 0;
  padding: 10px 18px 14px;
  border-top: 1px solid var(--border);
}
/* 内容里自带操作行（各页面的 .dialog-foot / .modal-actions）时，把它**钉在底部**：
   手机上主操作（「创建」「保存」）不能要滚动才看见。用 sticky 而不是 flex 布局，
   这样长内容照样能滚过去。负 margin 是为了抵消 .sheet-body 的内边距、让条铺满。 */
.sheet-body :deep(.dialog-foot:last-child),
.sheet-body :deep(.modal-actions:last-child) {
  position: sticky;
  bottom: -16px;
  margin: 12px -18px -16px;
  padding: 10px 18px;
  background: color-mix(in srgb, var(--bg-2) 94%, transparent);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border-top: 1px solid var(--border);
}
</style>
