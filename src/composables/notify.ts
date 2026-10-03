/**
 * 全局消息 API（给**非组件代码**用）。
 *
 * 历史：naive 的 `useMessage()` 只能在组件 setup 里调，异步回调/全局事件里调会抛异常，
 * 所以以前要向 MessageBridge 组件「借」一个实例存到模块级变量。
 * 现在后端换成了不依赖任何 provider 的 Vant Toast（见 ./message.ts），
 * 这层间接就不需要了 —— 这里保留的只是**语义化名字**（notifySuccess/notifyError），
 * 让调用点读起来是「通知」而不是「组件里拿到的那个 message 对象」。
 */
import { log as devLog, error as devError, warn as devWarn } from "../utils/logger";
import { useMessage } from "./message";

const message = useMessage();

export function notifySuccess(content: string) {
  try {
    message.success(content);
  } catch {
    devLog("[notify]", content);
  }
}

export function notifyError(content: string) {
  try {
    message.error(content);
  } catch {
    devError("[notify]", content);
  }
}

export function notifyWarning(content: string) {
  try {
    message.warning(content);
  } catch {
    devWarn("[notify]", content);
  }
}
