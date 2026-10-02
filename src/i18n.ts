/**
 * 极简 i18n：**语义化 key**（`nav.home` / `settings.plugins.recommended`）。
 *
 *   代码：`t("nav.home")`、`t("nav.downloading", { count: n })`
 *   词表：`zh-CN.json = { "nav.home": "首页" }`、`en-US.json = { "nav.home": "Home" }`
 *
 * 为什么不引 vue-i18n：启动器对体积敏感，而我们要的能力只有三件 ——
 * 查表、占位符替换、切语言。自家这几行就够，还不用跟着升级依赖。
 *
 * 用法：
 *   import { t } from "../i18n";
 *   {{ t("nav.home") }}                                   ← 模板（要用 `:title`，静态 `title` 不跟着切）
 *   message.success(t("settings.plugins.installed"))       ← 脚本里
 *   t("nav.downloading", { count: n })                     ← 占位符按 zh-CN 里的 `{名字}` 填
 *
 * 查不到就逐级回落：当前语言 → 中文 → key 本身（故意让 key 裸露出来，好一眼看出漏了哪条）。
 * 调试：构建期可强制语言，用来截图/验证 —— `VITE_LOCALE=en-US npm run build`
 */
import { ref } from "vue";
import zhCN from "./locales/zh-CN.json";
import enUS from "./locales/en-US.json";

export type LocaleCode = "zh-CN" | "en-US";

/** 语言清单：label 用该语言的自称，语言选择器直接显示 */
export const LOCALES: { code: LocaleCode; label: string }[] = [
  { code: "zh-CN", label: "简体中文" },
  { code: "en-US", label: "English" },
];

const FALLBACK: LocaleCode = "zh-CN";
const CATALOGS: Record<LocaleCode, Record<string, string>> = {
  "zh-CN": zhCN as Record<string, string>,
  "en-US": enUS as Record<string, string>,
};
const STORAGE_KEY = "qookix.locale";

function detect(): LocaleCode {
  const forced = import.meta.env.VITE_LOCALE as LocaleCode | undefined;
  if (forced && CATALOGS[forced]) return forced;
  try {
    const saved = localStorage.getItem(STORAGE_KEY) as LocaleCode | null;
    if (saved && CATALOGS[saved]) return saved;
  } catch {
    /* 无痕模式等：忽略 */
  }
  // 默认中文：翻译没铺开之前不去猜系统语言，免得界面半中半英
  return FALLBACK;
}

/** 当前语言。响应式 —— t() 会读它，切换时用到 t 的组件自动重渲染。 */
export const locale = ref<LocaleCode>(detect());

/** 某语言的已翻译条数（语言选择器可显示覆盖率，如「English 8/1265」） */
export function translatedCount(code: LocaleCode): number {
  return Object.keys(CATALOGS[code] ?? {}).length;
}

/**
 * 取译文。逐级回落：当前语言 → 中文 → 原文。
 * 所以没翻译、甚至没迁迁移的文案照旧显示中文，不会出现空白或 key。
 */
// 参数值放成 unknown：调用点常拿到 `number | null` / `unknown` 这类值，
// 而占位符**名字**的正确性由 `npm run i18n:audit` 校验（【6】调用点少传/占位符不一致），
// 这里再卡类型只会逼出一堆 as 断言。
export function t(text: string, params?: Record<string, unknown>): string {
  const dict = CATALOGS[locale.value] ?? {};
  let out = dict[text] ?? CATALOGS[FALLBACK][text] ?? text;
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      out = out.split(`{${name}}`).join(String(value));
    }
  }
  return out;
}

export function setLocale(code: LocaleCode): void {
  if (!CATALOGS[code]) return;
  locale.value = code;
  try {
    localStorage.setItem(STORAGE_KEY, code);
  } catch {
    /* 存不下也无所谓，本次会话仍然生效 */
  }
}
