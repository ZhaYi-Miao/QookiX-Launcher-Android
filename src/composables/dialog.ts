/**
 * 命令式确认框（naive-ui 的 `useDialog()` 替身，后端换成 Vant Dialog）。
 *
 * 同 `message.ts` 的思路：保持调用方那份 naive 形状的入参
 * （`{ title, content, positiveText, negativeText, onPositiveClick }`），
 * 内部翻译成 Vant 的 `confirmDialog`。这样调用点不用改，
 * 而呈现已经是手机形态（居中卡片 → 底部面板式对话框，按钮通栏）。
 */
import { confirmDialog, dialog as showDialog } from "../ui/vant";

interface NaiveDialogOpt {
  title?: string;
  content?: string;
  positiveText?: string;
  negativeText?: string;
  onPositiveClick?: () => void | Promise<void>;
  onNegativeClick?: () => void | Promise<void>;
}

const fallbackOk = "OK";
const fallbackCancel = "取消";

/**
 * 执行「确认」回调，并**单独**兜住它自己的异常。
 *
 * 原来是 `.then(() => onPositiveClick?.()).catch(() => onNegativeClick?.())` —— 同一个 catch
 * 同时承担两件事：「用户点了取消」（Vant 用 reject 表示）和「确认回调自身抛错/reject」。
 * 于是确认回调一旦失败，就会去执行**取消分支**（比如删除失败却按取消处理、该刷新的不刷新），
 * 而且异常被静默吞掉，日志里也看不到任何痕迹。
 */
function runPositive(o: NaiveDialogOpt) {
  try {
    void Promise.resolve(o.onPositiveClick?.()).catch((e) =>
      console.error("[dialog] 确认回调执行失败", e)
    );
  } catch (e) {
    console.error("[dialog] 确认回调执行失败", e);
  }
}

function warn(o: NaiveDialogOpt) {
  confirmDialog({
    title: o.title,
    message: o.content ?? "",
    confirmButtonText: o.positiveText ?? fallbackOk,
    cancelButtonText: o.negativeText ?? fallbackCancel,
    confirmButtonColor: "#e5534b",
  })
    .then(() => runPositive(o))
    // 走到这里只可能是「用户取消 / 关掉了对话框」
    .catch(() => void o.onNegativeClick?.());
}

function info(o: NaiveDialogOpt) {
  showDialog({
    title: o.title,
    message: o.content ?? "",
    showCancelButton: !!o.negativeText,
    confirmButtonText: o.positiveText ?? fallbackOk,
    cancelButtonText: o.negativeText ?? fallbackCancel,
  })
    .then(() => runPositive(o))
    .catch(() => void o.onNegativeClick?.());
}

export interface DialogApi {
  warning: (o: NaiveDialogOpt) => void;
  error: (o: NaiveDialogOpt) => void;
  info: (o: NaiveDialogOpt) => void;
  success: (o: NaiveDialogOpt) => void;
}

const api: DialogApi = { warning: warn, error: warn, info, success: info };

/** 与 naive 同名同形；不需要 provider 上下文。 */
export function useDialog(): DialogApi {
  return api;
}
