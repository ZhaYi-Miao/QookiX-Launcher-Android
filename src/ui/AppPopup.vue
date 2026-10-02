<script lang="ts">
/**
 * Vant Popup 的统一入口：默认 Teleport 到 `#van-layer`。
 *
 * 为什么必须包一层（两个都是真机踩出来的坑）：
 * 1. **层叠上下文**：页面根容器 `.content` 声明了 `container-type: inline-size`
 *    （界面缩放用 zoom，断点必须用容器查询），而 container-type 会创建新的
 *    stacking context —— 直接写在页面里的弹层（哪怕 z-index 2001）也会被
 *    整个压到 `.mobile-nav`（z-index 100）之下，底部被导航盖住。
 *    传送到 `.app` 直属的 `#van-layer`（z-index 1000）就跳出了这个上下文。
 * 2. **界面缩放**：`#app` 上挂 zoom，弹层在 `.app` 子树内才能跟页面同步缩放；
 *    Vant 默认 Teleport 到 body 会脱离缩放。
 */
export default { inheritAttrs: false };
</script>

<script setup lang="ts">
import { Popup as VanPopup } from "vant";
import { VANT_LAYER } from "./vant";

// $attrs 先展开、teleport 后写：调用方无法（也不应该）覆盖挂载点，
// 要改挂载策略只改 ui/vant.ts 里的 VANT_LAYER。
</script>

<template>
  <van-popup v-bind="$attrs" :teleport="VANT_LAYER">
    <slot />
  </van-popup>
</template>
