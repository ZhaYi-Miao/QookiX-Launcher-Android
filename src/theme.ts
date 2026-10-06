
/** Warm amber accent derived from the app icon (RGB ~224,160,80). */
export const DEFAULT_ACCENT = "#e89a4b";

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export function hexToRgb(hex: string): Rgb {
  let h = (hex || "").trim().replace("#", "");
  if (h.length === 3) {
    h = h
      .split("")
      .map((c) => c + c)
      .join("");
  }
  const num = parseInt(h, 16);
  if (Number.isNaN(num) || h.length !== 6) {
    return { r: 232, g: 154, b: 75 };
  }
  return { r: (num >> 16) & 255, g: (num >> 8) & 255, b: num & 255 };
}

/** Darken a hex color by the given ratio (0-1). */
export function darken(hex: string, amount = 0.14): string {
  const { r, g, b } = hexToRgb(hex);
  const f = (v: number) => Math.round(v * (1 - amount));
  return rgbToHex(f(r), f(g), f(b));
}

/** Lighten a hex color toward white by the given ratio (0-1). */
export function lighten(hex: string, amount = 0.12): string {
  const { r, g, b } = hexToRgb(hex);
  const f = (v: number) => Math.round(v + (255 - v) * amount);
  return rgbToHex(f(r), f(g), f(b));
}

export function rgbToHex(r: number, g: number, b: number): string {
  const f = (v: number) => Math.max(0, Math.min(255, v)).toString(16).padStart(2, "0");
  return `#${f(r)}${f(g)}${f(b)}`;
}

export function rgba(hex: string, alpha: number): string {
  const { r, g, b } = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** hex → HSL（h: 0–360，s/l: 0–100）。取色面板三个滑杆都按 HSL 算。 */
export function hexToHsl(hex: string): { h: number; s: number; l: number } {
  const { r, g, b } = hexToRgb(hex);
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const d = max - min;
  const l = (max + min) / 2;
  let h = 0;
  let s = 0;
  if (d !== 0) {
    s = d / (1 - Math.abs(2 * l - 1));
    if (max === rn) h = ((gn - bn) / d) % 6;
    else if (max === gn) h = (bn - rn) / d + 2;
    else h = (rn - gn) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: s * 100, l: l * 100 };
}

/** HSL → hex。h 允许任意值（会取模），s/l 会夹进 0–100。 */
export function hslToHex(h: number, s: number, l: number): string {
  const hn = ((h % 360) + 360) % 360;
  const sn = Math.min(100, Math.max(0, s)) / 100;
  const ln = Math.min(100, Math.max(0, l)) / 100;
  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const x = c * (1 - Math.abs(((hn / 60) % 2) - 1));
  const m = ln - c / 2;
  let r = 0;
  let g = 0;
  let b = 0;
  if (hn < 60) { r = c; g = x; }
  else if (hn < 120) { r = x; g = c; }
  else if (hn < 180) { g = c; b = x; }
  else if (hn < 240) { g = x; b = c; }
  else if (hn < 300) { r = x; b = c; }
  else { r = c; b = x; }
  return rgbToHex(
    Math.round((r + m) * 255),
    Math.round((g + m) * 255),
    Math.round((b + m) * 255)
  );
}

export const ACCENT = DEFAULT_ACCENT;
export const ACCENT_DEEP = darken(DEFAULT_ACCENT);
export const ACCENT_SOFT = rgba(DEFAULT_ACCENT, 0.14);

/** Accent alpha levels used across the stylesheets, exposed as `--accent-01` ... `--accent-60`. */
export const ACCENT_ALPHAS = [
  0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.08, 0.1, 0.12, 0.14, 0.16, 0.2, 0.22,
  0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.6,
];

export function accentAlphaVar(alpha: number): string {
  const digits = String(Math.round(alpha * 100)).padStart(2, "0");
  return `--accent-${digits}`;
}
