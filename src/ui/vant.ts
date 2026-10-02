/**
 * Vant 的统一出口：弹层挂载点 + 命令式调用的封装。
 *
 * **为什么要自己包一层**：Vant 的弹层（Popup / Toast / Dialog）默认 Teleport 到 `<body>，
 * 而本项目用 `zoom` 实现界面缩放（`#app` 上），**传送到 body 就不在缩放子树里** ——
 * 于是 60% 缩放下弹层反而比页面大、150% 下反而变小（naive-ui 的 n-modal 是同一个坑，
 * 见 styles.css 的注释）。这里统一传送到 `#van-layer`（在 `.app` 内），缩放就能作用到弹层。
 *
 * 所有地方都用 `VANT_LAYER`，不直接写 "body"；将来要改挂载策略只改这一个常量。
 */
import {
  showToast,
  showSuccessToast,
  showFailToast,
  showLoadingToast,
  closeToast,
  showDialog,
  showConfirmDialog,
} from "vant";

/** 弹层挂载点（App.vue 里 `<div id="van-layer">`，位于 `#app` 的 zoom 子树内）。 */
export const VANT_LAYER = "#van-layer";

// 命令式 API 的第一参是 `string | Options | undefined`：剔掉 undefined（无参调用合法）
// 再剔掉 string（我们只允许对象形态，字符串由 normToast 统一包装）。
type ToastOpt = Exclude<NonNullable<Parameters<typeof showToast>[0]>, string>;
type DialogOpt = NonNullable<Parameters<typeof showDialog>[0]>;

const withLayer = <T extends object>(opt?: T) => ({ teleport: VANT_LAYER, ...(opt ?? {}) });

const normToast = (m: string | ToastOpt): ToastOpt =>
  typeof m === "string" ? ({ message: m } as ToastOpt) : m;

export function toast(m: string | ToastOpt) {
  return showToast(withLayer(normToast(m)));
}
export function toastOk(m: string | ToastOpt) {
  return showSuccessToast(withLayer(normToast(m)));
}
export function toastErr(m: string | ToastOpt) {
  return showFailToast(withLayer(normToast(m)));
}
export function toastLoading(m: string | ToastOpt) {
  return showLoadingToast(withLayer(normToast(m)));
}
export { closeToast };

export function dialog(opt: DialogOpt) {
  return showDialog(withLayer(opt));
}
export function confirmDialog(opt: DialogOpt) {
  return showConfirmDialog(withLayer(opt));
}

export {
  Popup,
  Tabbar,
  TabbarItem,
  Sidebar,
  SidebarItem,
  Cell,
  CellGroup,
  Switch,
  Field,
  Button,
  Picker,
  ActionSheet,
  Tag,
  Badge,
  Empty,
  Loading,
  NavBar,
  Search,
  List,
  PullRefresh,
  SwipeCell,
  Checkbox,
  Stepper,
  Slider,
  Progress,
  Collapse,
  CollapseItem,
} from "vant";
