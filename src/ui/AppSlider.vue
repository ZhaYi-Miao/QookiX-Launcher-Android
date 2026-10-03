<script setup lang="ts">
/**
 * 滑杆（Vant Slider 的 naive 兼容层）：保持 `v-model:value` / `:value` + `@update:value`。
 *
 * 处理了三个必须处理的差异：
 *
 * 1. **气泡文案**：naive 的 `format-tooltip` 能把值显示成「2048 MB」这类带单位的内容，
 *    Vant 的气泡只显示裸数值。这里用 `#button` 插槽自己画气泡，保留 naive 的 formatter 语义。
 *
 * 2. **「轨道上任意位置都能按住拖」**（用户 2026-10-03 反馈的三个滑杆都拖不动）：
 *    Vant 把 `touchstart` **只绑在圆钮上**（见 node_modules/vant/lib/slider/Slider.js），
 *    所以手指必须精确按在那个 ~12px 的圆点上才能拖 —— 手机上基本按不中；
 *    naive 的滑杆则是轨上任意位置按下即生效。这里补一层命中区：
 *    在包裹层上接管 touchstart/touchmove，按触点位置直接算值并派发，
 *    圆钮自身那套逻辑保持不动（两条路径都走同一个 emit，不会打架）。
 *
 * 3. **手势不能被页面滚动抢走**：安卓 WebView 会把横向拖动判成页面滚动并发
 *    pointercancel 掐断拖动。外层 `touch-action: none`，并且 touchmove 用
 *    非 passive 监听以便 `preventDefault()`（Vant 自己那层是 passive 的，不够）。
 */
import { Slider as VanSlider } from "vant";
import { ref } from "vue";

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

/**
 * 包裹层的 DOM 引用 —— 注意这里**必须量 DOM**，不能拿 van-slider 的组件实例 `$el`：
 * Vant 的 Slider 调了 `expose()`，模板 ref 拿到的是它暴露的对象，**没有 `$el`**，
 * 于是每次换算都在第一步就 return（表现为「拖了完全没反应」，排查了一圈）。
 */
const wrap = ref<HTMLElement | null>(null);
/** 手势状态：'' = 未开始 / 'pending' = 已按下但方向未定 / 'drag' = 正在拖 / 'scroll' = 判定为页面滚动 */
let phase: "" | "pending" | "drag" | "scroll" = "";
let startX = 0;
let startY = 0;

function emitFromClientX(clientX: number) {
  const rail = wrap.value?.querySelector(".van-slider") as HTMLElement | null;
  if (!rail || props.disabled) return;
  const rect = rail.getBoundingClientRect();
  if (!rect.width) return;
  // Vant 的轨道两端各留了半个圆钮的内边距，换算要按可用宽度而不是元素宽度
  const pad = 6;
  const usable = Math.max(1, rect.width - pad * 2);
  const ratio = Math.min(1, Math.max(0, (clientX - rect.left - pad) / usable));
  const raw = props.min + ratio * (props.max - props.min);
  // 对齐步长后夹进区间（步长可能是小数，先四舍五入到 1e-6 再对齐）
  const stepped = Math.round(raw / props.step) * props.step;
  const v = Math.min(props.max, Math.max(props.min, Number(stepped.toFixed(6))));
  if (v !== props.value) emit("update:value", v);
}

/**
 * 手指落在轨道上时**先不抢手势**：等第一次移动看清方向再决定。
 * 手机上用户经常是「在滑杆上方竖滑去滚页面」，如果一按下就吞掉所有手势，
 * 页面就滚不动了（滑杆常常横跨整行，躲不开）。
 */
function onTouchStart(e: TouchEvent) {
  if (props.disabled) return;
  const t = e.touches[0];
  if (!t) return;
  phase = "pending";
  startX = t.clientX;
  startY = t.clientY;
}

function onTouchMove(e: TouchEvent) {
  if (props.disabled || phase === "scroll" || phase === "") return;
  const t = e.touches[0];
  if (!t) return;
  const dx = Math.abs(t.clientX - startX);
  const dy = Math.abs(t.clientY - startY);

  if (phase === "pending") {
    // 移动还不够多就先等一等（手指刚碰上去时的抖动会误判方向）
    if (dx < 6 && dy < 6) return;
    if (dy > dx) {
      phase = "scroll"; // 竖滑 → 让页面滚，别抢
      return;
    }
    phase = "drag";
    emitFromClientX(t.clientX); // 判定为拖动后，立刻响应触点位置
  }

  // 非 passive，能真正阻止「横向拖动被判成页面滚动」
  e.preventDefault();
  emitFromClientX(t.clientX);
}

function onTouchEnd(e: TouchEvent) {
  // 「点一下轨道」也要生效（手机上很常用：不必拖，点哪到哪）。
  // 只有手势还停在 pending（说明这一下基本没移动）时才当作点击，
  // 否则竖滑被判成滚动的那次手指抬起会误改数值。
  if (phase === "pending") {
    const t = e.changedTouches[0];
    if (t) emitFromClientX(t.clientX);
  }
  phase = "";
}
</script>

<template>
  <div
    ref="wrap"
    class="app-slider-wrap"
    :class="{ disabled }"
    @touchstart="onTouchStart"
    @touchmove="onTouchMove"
    @touchend="onTouchEnd"
    @touchcancel="onTouchEnd"
  >
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
  </div>
</template>

<style scoped>
.app-slider-wrap {
  /* 宽度必须自己撑开：Vant 的 `.van-slider` 没有固定宽度，靠父级给。
     以前它是 `.scale-ctl` 等 flex 行的直接子元素，会被拉伸；现在多了一层包裹 div，
     默认 `flex: 0 1 auto` + 内容撑不开 → 整个滑杆被压成 **0 宽**（真机上就是「滑杆不见了/拖不动」）。
     所以这层必须显式占满：width 100% + flex 增长。 */
  width: 100%;
  flex: 1 1 auto;
  min-width: 0;
  /* pan-y：竖向留给浏览器（手指落在滑杆上也能滚页面），横向由我们接管拖拽。
     写成 touch-action: none 会把页面的竖向滚动也一起吞掉 —— 滑杆常常横跨整行，
     用户就那么恰好按在它上面往下滑，页面却纹丝不动。
     （Vant 自己的 CSS 没有声明 touch-action，所以这里是唯一来源，不用 !important） */
  touch-action: pan-y;
  /* 上下留白也是命中区：手机上别只给 8px 高的轨道 */
  padding: 10px 0;
}
.app-slider-wrap.disabled {
  opacity: 0.5;
}
.app-slider {
  touch-action: pan-y;
  /* 手机手感：默认轨道只有 2px、圆钮 12px，滑杆一多就显得又细又难点。
     加粗到 4px、圆钮 20px，和项目里其它控件的触控尺度一致。 */
  --van-slider-bar-height: 4px;
  --van-slider-button-width: 20px;
  --van-slider-button-height: 20px;
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
