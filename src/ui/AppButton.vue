<script setup lang="ts">
/**
 * 按钮（手机上替代 naive 的 `n-button`，底层是 Vant Button）。
 *
 * 为什么还包一层：全仓有 20+ 处 `n-button`，它们用了 naive 的几种形态
 * （`type="primary|error"` / `quaternary` / `ghost` / `size="small|tiny"` / `:loading`）。
 * 直接在模板里手改每一项容易漏，而且项目以后调按钮规格（圆角/高度/主色）会散落各处。
 * 这里把 naive 的那套写法**照单收下并翻译**成 Vant，调用点几乎不用改。
 *
 * 与 naive 的差异：
 * - `type="error"` → 映到 Vant 的 `danger`（红色）；
 * - `quaternary` / `ghost` → 映到 Vant 的 `plain`（描边/无底色），语义最接近；
 * - 默认高度按手机触控给（40px），`small` 是 34px，`tiny` 也是 34px（不再有 20 几像素的按钮）。
 * 配色不写死：`src/vant.css` 已把 `--van-button-*` 映射到项目 token，主色跟随主题。
 */
import { Button as VanButton } from "vant";
import { computed } from "vue";

type NaiveType = "default" | "primary" | "info" | "success" | "warning" | "error";
/** 同时收 naive 的 `error` 和 Vant 的 `danger`（两边都表示红色，迁移期两种写法都会出现） */
type BtnType = NaiveType | "danger";

const props = withDefaults(
  defineProps<{
    type?: BtnType;
    /** naive 的 size；这里只区分「常规」和「小」 */
    size?: "tiny" | "small" | "medium" | "large";
    /** naive 的无底色按钮 */
    quaternary?: boolean;
    ghost?: boolean;
    block?: boolean;
    loading?: boolean;
    disabled?: boolean;
    /** 通底纯文字（如弹层里的次要按钮） */
    text?: boolean;
  }>(),
  { type: "default", size: "medium" }
);

/**
 * 类型映射（naive 有 6 种，Vant 只有 5 种且没有 `info`）：
 * - error → danger（红色，两边同义）
 * - info → default（Vant 无对应色，退成中性键，语义上「信息性操作」用中性更稳）
 */
const vanType = computed<"default" | "primary" | "success" | "warning" | "danger">(() => {
  if (props.type === "error") return "danger";
  if (props.type === "info") return "default";
  return props.type;
});
const plain = computed(() => !!(props.quaternary || props.ghost));
const vanSize = computed(() => (props.size === "large" ? "large" : props.size === "medium" ? "normal" : "small"));
</script>

<template>
  <van-button
    :type="vanType"
    :size="vanSize"
    :plain="plain"
    :block="block"
    :loading="loading"
    :disabled="disabled"
    class="app-btn"
    :class="{ 'app-btn-text': text }"
  >
    <slot />
  </van-button>
</template>

<style scoped>
.app-btn {
  font-family: inherit;
  font-weight: 500;
}
/* 手机底线：常规按钮 40px 高、小号 34px（Vant 默认更矮，手指不好点） */
.app-btn:not(.van-button--small) {
  min-height: 40px;
}
.app-btn.van-button--small {
  min-height: 34px;
}
/* 纯文字按钮：去掉边框和内边距感，用于弹层里的次要动作 */
.app-btn-text {
  border: none;
  background: transparent;
  color: var(--text-2);
}
</style>
