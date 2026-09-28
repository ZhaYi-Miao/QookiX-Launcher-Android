/**
 * 渲染器的前端侧规则。
 *
 * **必须与后端保持一致**（`launch.rs::auto_renderer_for` /
 * `renderer_health::check`），否则会出现「界面说 GL4ES、实际跑 MG」这类错位。
 *
 * 版本不对称的由来：
 *  - 26.x 换了整套着色器体系（UBO + `core/*`），GL4ES 第一步就 `GL_INVALID_VALUE` → 必须 MobileGlues；
 *  - 1.x 反过来：MG 翻不动 `shaders/post/*`（`Invalid #version`），1.8.9 会崩在启动阶段 → 用 GL4ES。
 */

export type RendererKey = "opengles2" | "mobileglues" | "vulkan_zink";

/** 渲染器键 → 人话。 */
export const rendererNames: Record<string, string> = {
  opengles2: "GL4ES",
  mobileglues: "MobileGlues",
  vulkan_zink: "Zink + Turnip",
};

export const rendererOptions = [
  { label: "GL4ES（兼容性最好）", value: "opengles2" },
  { label: "MobileGlues（26.x 必需）", value: "mobileglues" },
  { label: "Zink + Turnip（实验）", value: "vulkan_zink" },
];

export function rendererLabel(key?: string | null): string {
  if (!key) return rendererNames.opengles2;
  return rendererNames[key] ?? key;
}

/** 按 MC 版本挑推荐渲染器（和后端 `auto_renderer_for` 同规则）。 */
export function autoRendererFor(mcVersion?: string | null): RendererKey {
  const major = Number.parseInt((mcVersion ?? "").trim().split(/[.\-_ ]/)[0] ?? "", 10);
  return Number.isFinite(major) && major >= 26 ? "mobileglues" : "opengles2";
}

/**
 * 实例设置 + 全局设置 → 本次实际会用的渲染器键。
 *
 * `global` 需要调用方把全局值传进来（存在安卓 SharedPreferences 里，只有后端读得到）；
 * `auto`（含缺省）直接按版本推断，因此**永远不会**和推荐值不一致。
 */
export function effectiveRendererKey(
  instanceRenderer: string | null | undefined,
  mcVersion: string | null | undefined,
  globalRenderer: string | null | undefined,
): RendererKey {
  const key = instanceRenderer ?? "auto";
  if (key === "auto" || key === "") return autoRendererFor(mcVersion);
  if (key === "global") {
    const g = globalRenderer ?? "opengles2";
    if (g === "mobileglues" || g === "vulkan_zink" || g === "opengles2") return g;
    return "opengles2";
  }
  if (key === "mobileglues" || key === "vulkan_zink" || key === "opengles2") return key;
  return autoRendererFor(mcVersion);
}
