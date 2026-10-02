import { inject, ref, type Ref } from "vue";

export interface PageAction {
  text: string;
  run: () => void;
}

/** 外壳顶栏右上角的动作。页面在 onMounted 里赋值即可（App.vue 会渲染成按钮）。 */
const fallback = ref<PageAction | null>(null);

export function usePageAction(): Ref<PageAction | null> {
  return inject("pageAction", fallback) as Ref<PageAction | null>;
}
