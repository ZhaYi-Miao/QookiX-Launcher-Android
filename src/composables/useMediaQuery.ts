import { onScopeDispose, ref } from "vue";

/**
 * 响应式的媒体查询。用于「同一时刻只渲染一个」的布局判断
 * （例如账号面板：侧边栏与底部导航各有一个 AccountChip，
 * 而 naive-ui 的 n-popover 默认 Teleport 到 body ——
 * 即使父容器是 display:none，弹层照样会显示，
 * 于是「切换账号」会同时弹出两个选择用户的窗口）。
 *
 * 仅靠 CSS 隐藏不够，必须用 v-if 真正只保留一个实例。
 */
export function useMediaQuery(query: string) {
  const mql = window.matchMedia(query);
  const matches = ref(mql.matches);
  const handler = (e: MediaQueryListEvent) => {
    matches.value = e.matches;
  };
  mql.addEventListener("change", handler);
  onScopeDispose(() => mql.removeEventListener("change", handler));
  return matches;
}

/** 与各处 CSS 里的移动端断点保持一致 */
export const MOBILE_QUERY = "(max-width: 1100px), (pointer: coarse)";

export function useIsMobile() {
  return useMediaQuery(MOBILE_QUERY);
}
