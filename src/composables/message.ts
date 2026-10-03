/**
 * 全局消息提示（naive-ui 的 `useMessage()` 替身，后端换成 Vant Toast）。
 *
 * 为什么做「替身」而不是逐个改调用点：`message.success(...)` 这类调用全仓有 150+ 处，
 * 分布在 26 个文件里。这里保持**完全相同的对象 API**（success/error/warning/info/loading），
 * 于是迁移成本从「改 150 处」降到「改 26 行 import」。
 *
 * 与 naive 的行为差异（都是有意的）：
 * - 提示统一出现在**底部**（手机上拇指区，视线也不用上抬去找顶部通知）；
 * - 同时只显示一条（Vant 默认行为）——手机上叠一屏提示很糟；
 * - `loading` 默认不自动关闭（与 naive 一致，由调用方自己收尾：再弹一条成功/失败即可）。
 */
import { closeToast, toast, toastErr, toastLoading, toastOk } from "../ui/vant";

export interface MessageApi {
  success: (content: string) => void;
  error: (content: string) => void;
  warning: (content: string) => void;
  info: (content: string) => void;
  loading: (content: string) => void;
  /** 收尾：关掉当前提示（Vant 的 toast 是一次性对象，这里只是转发 closeToast） */
  destroyAll: () => void;
}

/** 错误信息通常带技术细节，手机上给足停留时间（默认 2s 读不完） */
const ERROR_DURATION = 4000;

const api: MessageApi = {
  success(content: string) {
    toastOk({ message: content, duration: 2200 });
  },
  error(content: string) {
    toastErr({ message: content, duration: ERROR_DURATION });
  },
  warning(content: string) {
    toast({ message: content, duration: 3000 });
  },
  info(content: string) {
    toast({ message: content, duration: 2500 });
  },
  loading(content: string) {
    toastLoading({ message: content, duration: 0, forbidClick: true });
  },
  destroyAll() {
    closeToast();
  },
};

/**
 * 与 naive 同名同形：组件 setup 里 `const message = useMessage()` 照旧写。
 * 这里不依赖任何 provider 上下文，所以**非组件代码也能直接调**（比 naive 更宽松）。
 */
export function useMessage(): MessageApi {
  return api;
}
