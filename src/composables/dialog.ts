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

function warn(o: NaiveDialogOpt) {
  confirmDialog({
    title: o.title,
    message: o.content ?? "",
    confirmButtonText: o.positiveText ?? fallbackOk,
    cancelButtonText: o.negativeText ?? fallbackCancel,
    confirmButtonColor: "#e5534b",
  })
    .then(() => o.onPositiveClick?.())
    .catch(() => o.onNegativeClick?.());
}

function info(o: NaiveDialogOpt) {
  showDialog({
    title: o.title,
    message: o.content ?? "",
    showCancelButton: !!o.negativeText,
    confirmButtonText: o.positiveText ?? fallbackOk,
    cancelButtonText: o.negativeText ?? fallbackCancel,
  })
    .then(() => o.onPositiveClick?.())
    .catch(() => o.onNegativeClick?.());
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
