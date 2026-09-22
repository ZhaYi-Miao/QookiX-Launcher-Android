<script setup lang="ts">
/**
 * 关于页顶部展示卡片画布。
 *
 * 实现：
 * - 背景：红色格子「桌布」，随时间做斜向（x/y 同步）循环移动。
 * - 中央：QookiX 曲奇 logo，循环播放「完整 → 被咬 → 复原 → 保持……」。
 * - 关键：咬是「一个事件」，不是先后分开的阶段。咬下的那一帧就**同时**：
 *   1) 咬口弹簧把缺口「弹开」；
 *   2) 给旋转弹簧注入一个角速度冲量，曲奇立刻抖动；
 *   3) 从缺口喷出碎屑粒子，并在咬开过程中持续掉落。
 *   因此咬、抖、粒子三者同时发生，观感连贯顺滑。
 * - 咬口：用 destination-out + globalAlpha 按咬口弹簧值(0..1)从右上边缘抹掉一小块，
 *   并带几颗「牙印」。复原阶段咬口弹簧弹回 0，整块曲奇愈合。
 * - 2D Procrustes 对齐：咬掉后质心偏移，用两圆相交面积比做反向平移，
 *   把质心拉回画布中心。
 * - 物理：全部弹簧用「固定小步长累积积分」推进，避免可变帧长导致回弹不一致，
 *   保证动画在不同帧率下都顺滑。
 */
import { onMounted, onUnmounted, ref } from "vue";
import logoUrl from "../assets/logo.png";
import creeperFaceUrl from "../assets/creeper-face.png";
import appIconAndroid from "../assets/app-icon-android.png";

const wrapRef = ref<HTMLDivElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);

/* ---- 阻尼弹簧振荡器（半隐式欧拉积分，target 可随时间变化） ---- */
interface Spring {
  value: number;
  velocity: number;
  stiffness: number;
  damping: number;
  target: number;
}

function springStep(s: Spring, dt: number) {
  const force = (s.target - s.value) * s.stiffness - s.velocity * s.damping;
  s.velocity += force * dt;
  s.value += s.velocity * dt;
}

/* ---- 动画时间线（单位：秒） ---- */
// 周期：静止(settle) → [咬下: 张开(open) + 愈合(heal)] → 尾段静止
const SETTLE = 1.15;
const OPEN = 0.85; // 咬口张开时长
const HEAL = 0.9; // 复原时长
const CYCLE = 3.4;
const IMPACT = -2.2; // 咬下注入的角速度冲量：让曲奇「被撞」一下，快速偏到位
const BITE_TILT = -0.34; // 咬着时曲奇保持的偏转（rad，约 -20°）

/* 旋转弹簧分两套参数，分别对应两个动作，两者都保留「弹簧弹性」：
 * - 被咬时(tilt)：明显欠阻尼 —— 注入冲量后曲奇被撞得先冲过 BITE_TILT，
 *   再被弹回到 BITE_TILT 稳住，即「移动到位置」带弹性过冲，不是硬到位。
 * - 复位时(restore)：更慢更欠阻尼 —— 从偏转慢慢回正，冲过头越过 0°
 *   到另一侧，再被弹回正位停稳。关键是慢（ω≈7.7，全程约 1 秒）才看得清；
 *   之前 stiffness 220 时约 0.1 秒摆完，看着像闪一下。 */
const TILT_STIFF = 90;
const TILT_DAMP = 9; // 远小于临界阻尼 2*sqrt(90)≈19 → 欠阻尼，保留过冲回弹
const RESTORE_STIFF = 60;
const RESTORE_DAMP = 2.8;

const bite: Spring = { value: 0, velocity: 0, stiffness: 300, damping: 26, target: 0 };
const angle: Spring = { value: 0, velocity: 0, stiffness: TILT_STIFF, damping: TILT_DAMP, target: 0 };
/* 躲避位移弹簧：被咬时曲奇整体往「远离咬口」方向躲闪的位移。
 * 独立于 bite 弹簧、欠阻尼 → 位移本身带惯性弹性（冲过头再弹回），
 * 而不是跟着咬口瞬间到位。value 目标 1=躲开、0=回位。 */
const RECOIL_DAMP = 8;
const recoil: Spring = { value: 0, velocity: 0, stiffness: 80, damping: RECOIL_DAMP, target: 0 };

/* ---- 彩蛋：快速连点曲奇 → 炸碎 → 碎块向外飞溅 → 再拼回 ----
 * 曲奇是**像素画**，所以切块也按像素网格来：5×5 的正方块，
 * 圆外的格子直接丢弃 —— 方块碎片和像素风一致，扇形/三角切块会非常难看。
 * 复古爆炸感：方块向外飞散并受重力下坠，不旋转（旋转会破坏像素的对齐感）。
 * 飞散 easeOutCubic（爆开快之后减速），拼回 smoothstep（平滑归位）。
 * 拼回完成后用 cycleOffset 把咬合循环拨回周期开头 ——
 * **绝不能改写 elapsed**：它同时驱动桌布滚动，改了背景就会跳变（实测踩过）。 */
type Frag = {
  gx: number; // 碎片左上角在曲奇局部坐标（中心为原点）的位置
  gy: number;
  w: number;
  h: number;
  dx: number; // 飞散位移向量（乘以进度系数 k 使用）
  dy: number;
};
let frags: Frag[] = [];
let shatterT = -1; // <0 = 未触发；否则为炸碎动画已进行秒数
const SHATTER_FLY = 1.05; // 方块飞出画面的时长（要等它们**全部**坠出框外）
const SHATTER_WAIT = 0.45; // 空场停顿：碎片走干净后隔一拍，新曲奇才出现
const DROP_DUR = 0.65; // 新曲奇从屏幕顶部落下的时长（加速下落，不弹跳）
const LAND_DUR = 0.4; // 落地后的「脆裂」表现时长（压扁回弹 + 掉渣）

/* ---- 苦力怕偷曲奇：新曲奇落地停稳后，从画布右下角只「露出个脑袋」，
 * 盯一拍，然后把整块曲奇拖出画面右下角；成就弹窗在叼走**之后**才弹。
 * 头用真实皮肤贴图（assets/creeper-face.png，MHF_Creeper 的标准苦力怕脸），
 * 像素函数 drawCreeper 只作图片未加载时的兜底。
 * 结束后 reset 照旧：cycleOffset 拨回周期开头，曲奇「重新长回来」。 */
const RISE_DUR = 0.55; // 从底边探出头
const STARE_DUR = 0.35; // 露头后盯着曲奇看一拍
const DRAG_DUR = 0.7; // 把曲奇拖出画面
const AFTER_DUR = 0.5; // 拖走后的空场收尾
const DROP_T0 = SHATTER_FLY + SHATTER_WAIT; // 新曲奇开始下落
const LAND_T = DROP_T0 + DROP_DUR; // 落地时刻
const PEEK0 = LAND_T + LAND_DUR; // 开始露头
const PEEK_T = PEEK0 + RISE_DUR; // 露头到位
const DRAG_T = PEEK_T + STARE_DUR; // 开始拖曲奇
const GONE_T = DRAG_T + DRAG_DUR; // 曲奇完全出画
const SHATTER_TOTAL = PEEK0 + RISE_DUR + STARE_DUR + DRAG_DUR + AFTER_DUR;

/* ---- Minecraft 风爆炸粒子：白色小方块，向四周喷射、受重力、渐隐 ---- */
interface McPart {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  life: number;
  age: number;
  shade: number; // 0..1，决定白→灰的明暗，避免一片死白
}
let mcParts: McPart[] = [];

function spawnMcBurst(x: number, y: number, n: number) {
  for (let i = 0; i < n; i++) {
    const a = Math.random() * Math.PI * 2;
    const sp = 80 + Math.random() * 260;
    mcParts.push({
      x,
      y,
      vx: Math.cos(a) * sp,
      vy: Math.sin(a) * sp - 80,
      size: 3.5 + Math.random() * 5.5,
      life: 0.45 + Math.random() * 0.6,
      age: 0,
      shade: Math.random() * 0.55, // 偏白：大多数粒子接近纯白
    });
  }
}

function updateMcParts(dt: number) {
  for (let i = mcParts.length - 1; i >= 0; i--) {
    const p = mcParts[i];
    p.age += dt;
    if (p.age >= p.life) {
      mcParts.splice(i, 1);
      continue;
    }
    p.vx *= Math.exp(-1.2 * dt);
    p.vy += 300 * dt;
    p.x += p.vx * dt;
    p.y += p.vy * dt;
  }
}

function drawMcParts(g: CanvasRenderingContext2D) {
  for (const p of mcParts) {
    g.globalAlpha = Math.max(0, 1 - p.age / p.life);
    const c = Math.round(228 + p.shade * 27); // 228..255：白色为主，略带明暗
    g.fillStyle = `rgb(${c},${c},${c})`;
    // MC 的粒子是正方形，不是圆点
    g.fillRect(p.x - p.size / 2, p.y - p.size / 2, p.size, p.size);
  }
  g.globalAlpha = 1;
}

/** 触觉反馈：navigator.vibrate 在 WebView 里有 VIBRATE 权限时生效。 */
function buzz() {
  try {
    navigator.vibrate?.([28, 40, 55]);
  } catch {
    /* 设备/内核不支持就算了，彩蛋不能因为这个挂掉 */
  }
}

/** 咬合循环的相位偏移：周期相位 = (elapsed + cycleOffset) % CYCLE。
 *  拼回完成时调它把循环拨回「完整」，而不是动 elapsed。 */
let cycleOffset = 0;

/** 快速连点检测：900ms 内点满 7 次触发。 */
let tapTimes: number[] = [];
function onTap() {
  const now = performance.now();
  tapTimes = tapTimes.filter((t) => now - t < 900);
  tapTimes.push(now);
  if (tapTimes.length >= 7 && shatterT < 0) {
    tapTimes = [];
    triggerShatter();
  }
}

/* 成就弹窗：MC 风格「成就已达成！」横幅，触发彩蛋时滑入，2.6s 后收回。
 * 用 setTimeout 而不是动画结束事件 —— 简单可靠，离开页面时清掉即可。 */
const showAchieve = ref(false);
let achieveTimer = 0;
function showAchievement() {
  showAchieve.value = true;
  clearTimeout(achieveTimer);
  achieveTimer = window.setTimeout(() => {
    showAchieve.value = false;
  }, 2600);
}

function triggerShatter() {
  const cells = 5; // 5×5 网格；想更碎改大这个数
  const cell = (R * 2) / cells;
  frags = [];
  for (let iy = 0; iy < cells; iy++) {
    for (let ix = 0; ix < cells; ix++) {
      const gx = -R + ix * cell;
      const gy = -R + iy * cell;
      const ccx = gx + cell / 2;
      const ccy = gy + cell / 2;
      const d = Math.hypot(ccx, ccy);
      if (d > R * 0.98) continue; // 圆外没有曲奇，不生成碎片
      frags.push({
        gx,
        gy,
        w: cell,
        h: cell,
        dx: (ccx / (d || 1)) * R * (0.9 + Math.random() * 0.9) + (Math.random() - 0.5) * 24,
        dy:
          (ccy / (d || 1)) * R * (0.9 + Math.random() * 0.9) +
          (Math.random() - 0.5) * 24 -
          R * 0.2, // 整体稍偏上飘，再被重力拉下来，弧线更自然
      });
    }
  }
  shatterT = 0;
  landDone = false;
  grabBuzzed = false;
  secondPass = false;
  // 炸的是「完整」曲奇：咬口/偏转/躲闪全部瞬间归零 ——
  // 这三个弹簧在炸碎期间不参与绘制，保持 0 才能保证后续衔接无跳变。
  bite.target = 0;
  bite.value = 0;
  bite.velocity = 0;
  angle.target = 0;
  angle.value = 0;
  angle.velocity = 0;
  recoil.target = 0;
  recoil.value = 0;
  recoil.velocity = 0;
  spawnBurst(24); // 曲奇色碎屑
  spawnMcBurst(cssW / 2, cssH / 2, 64); // MC 风白色方块粒子
  buzz(); // 震动一下
  // 成就弹窗不在这里：等苦力怕把曲奇拖走后才弹（见 updatePhysics 的 GONE_T 分支）
}

let prevBiteOpen = false; // 上一次 target 是否"张开"，用于检测咬下/复位那一帧

/* ---- 时间推进 / 咬事件 ---- */
const STEP = 1 / 180; // 固定物理步长
let acc = 0;
let elapsed = 0;

let burstDone = false;

function computeBiteTarget(s: number) {
  if (s < SETTLE) return 0;
  if (s < SETTLE + OPEN) return 1;
  if (s < SETTLE + OPEN + HEAL) return 0; // 愈合阶段
  return 0; // 尾段静止
}

function updatePhysics() {
  // 炸碎动画期间接管物理：只推动画时钟与碎屑，不跑咬合弹簧。
  // elapsed 照常累加（在 frame() 里）—— 桌布滚动依赖它，动它就会跳背景。
  if (shatterT >= 0) {
    shatterT += STEP;
    updateCrumbs(STEP);
    updateMcParts(STEP);
    // 拖走完成：弹成就 + 震动（只触发一次）—— 成就要等曲奇真被叼走后再出
    if (shatterT >= GONE_T && !grabBuzzed) {
      grabBuzzed = true;
      showAchievement();
      buzz();
    }
    if (shatterT >= SHATTER_TOTAL) {
      if (!secondPass) {
        // 苦力怕偷走后：成就已弹，但曲奇不能瞬现 —— 把时钟拨回「下落阶段」
        // 再演一遍新曲奇从顶上落下 + 脆裂落地（第二趟不再有苦力怕/叼走）。
        secondPass = true;
        landDone = false;
        shatterT = DROP_T0;
      } else {
        // 第二趟落地结束：用相位偏移把咬合循环拨回「完整」。
        // （elapsed 不能动 —— 背景桌布的滚动用它计时，改了会跳变。）
        shatterT = -1;
        frags = [];
        prevBiteOpen = false;
        cycleOffset = -(elapsed % CYCLE);
        bite.value = 0;
        bite.velocity = 0;
        angle.target = 0;
        angle.value = 0;
      }
    }
    return;
  }

  const s = (elapsed + cycleOffset) % CYCLE;
  bite.target = computeBiteTarget(s);

  // 检测「咬下」的一帧：目标从闭合(0)翻转到张开(1)
  const isOpen = bite.target >= 0.5;
  if (isOpen && !prevBiteOpen) {
    // 被咬：欠阻尼弹簧 → 曲奇被撞得冲过 BITE_TILT 再弹回到位（有弹性）
    angle.stiffness = TILT_STIFF;
    angle.damping = TILT_DAMP;
    angle.target = BITE_TILT;
    angle.velocity += IMPACT; // 咬的瞬间被「撞」一下
    // 躲避位移：往外躲，欠阻尼 → 冲过头再弹回稳定位置
    recoil.target = 1;
    recoil.velocity += 3.2;
    spawnBurst(7); // 同步喷出碎屑
    burstDone = false;
  } else if (!isOpen && prevBiteOpen) {
    // 复位（愈合开始）：切到「软」弹簧，目标回正位 0。
    // 欠阻尼让它从偏转慢慢回正，并冲过头越过 0° 到另一侧，再弹回停稳。
    angle.stiffness = RESTORE_STIFF;
    angle.damping = RESTORE_DAMP;
    angle.target = 0;
    // 位移也回位，同样带弹性
    recoil.target = 0;
  }
  prevBiteOpen = isOpen;

  // 咬开过程中持续掉落几粒碎屑，让「咬」更有啃食感
  if (isOpen && !burstDone && Math.random() < 0.5) {
    spawnBurst(1);
  }
  // 张开到位后停（value 接近 target 就视为到位，避免整周期喷屑）
  if (isOpen && Math.abs(bite.value - 1) < 0.05) burstDone = true;

  springStep(bite, STEP);
  springStep(angle, STEP);
  springStep(recoil, STEP);
  updateCrumbs(STEP);
  // 炸碎彩蛋的白色粒子在正常路径下也要继续更新——
  // 之前只在炸碎分支里更新，彩蛋一结束粒子就冻在半空「永不消失」（用户实测踩过）。
  updateMcParts(STEP);
}

/* ---- 画布尺寸 / 句柄 ---- */
let last = 0;
let raf = 0;
let dpr = 1;
let cssW = 1;
let cssH = 1;
let ctxRef: CanvasRenderingContext2D | null = null;
let offscreen: HTMLCanvasElement | null = null;
let offCtx: CanvasRenderingContext2D | null = null;

const cookieImg = new Image();
cookieImg.src = logoUrl;
let imgLoaded = false;
cookieImg.onload = () => {
  imgLoaded = true;
};
cookieImg.onerror = () => {
  imgLoaded = false;
};

/* ---- 咬口碎屑粒子 ---- */
interface Crumb {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  life: number;
  age: number;
}
let crumbs: Crumb[] = [];

function spawnBurst(n: number, at?: { x: number; y: number }) {
  if (!ctxRef || n < 1) return;
  const cx = cssW / 2;
  const cy = cssH / 2;
  // 默认从咬口处喷；指定 at 时（比如新曲奇落地的脆裂掉渣）从那里喷
  const c = at
    ? { ...at }
    : localToWorld(
        biteDc * Math.cos(BITE_PHI),
        biteDc * Math.sin(BITE_PHI),
        cx,
        cy,
      );
  for (let i = 0; i < n; i++) {
    const a = Math.atan2(c.y - cy, c.x - cx) + (Math.random() - 0.5) * 2.2;
    const sp = (30 + Math.random() * 90) * (n === 7 ? 1 : 0.5);
    crumbs.push({
      x: c.x,
      y: c.y,
      vx: Math.cos(a) * sp,
      vy: Math.sin(a) * sp - 40,
      size: 1.6 + Math.random() * 3.4,
      life: 0.5 + Math.random() * 0.55,
      age: 0,
    });
  }
}

function updateCrumbs(dt: number) {
  for (let i = crumbs.length - 1; i >= 0; i--) {
    const cr = crumbs[i];
    cr.age += dt;
    if (cr.age >= cr.life) {
      crumbs.splice(i, 1);
      continue;
    }
    cr.vx *= Math.exp(-1.8 * dt);
    cr.vy += 70 * dt;
    cr.x += cr.vx * dt;
    cr.y += cr.vy * dt;
  }
}

/* ---- 曲奇几何 ---- */
function cookieRadius() {
  // 手机上 0.34×高 会让曲奇占掉画布大半（用户嫌「特别大」）——收紧一档
  return Math.round(Math.min(cssH * 0.26, cssW * 0.12));
}
const BITE_PHI = -0.95; // 咬口位于曲奇右上
let R = 60;
let biteR = 30;
let biteDc = 50;

/* 两圆相交面积（R: 曲奇半径, r: 咬口半径, d: 圆心距） */
function lensArea(Rr: number, r: number, d: number): number {
  if (d >= Rr + r) return 0;
  if (d <= Math.abs(Rr - r)) return Math.PI * Math.min(Rr, r) ** 2;
  const t1 = Rr * Rr * Math.acos((d * d + Rr * Rr - r * r) / (2 * d * Rr));
  const t2 = r * r * Math.acos((d * d + r * r - Rr * Rr) / (2 * d * r));
  const t3 =
    0.5 *
    Math.sqrt(
      (-d + Rr + r) * (d + Rr - r) * (d - Rr + r) * (d + Rr + r),
    );
  return t1 + t2 - t3;
}

/* 咬口 = 一个主圆 + 沿其圆周（朝曲奇内部那侧）分布的一圈「大小不一的小圆」。
 * 主圆把曲奇右上啃出一个圆凹口；小圆圆心落在主圆圆周上、半径各不同，
 * 只让圆凹的边缘变成参差的波状，整体仍是"一个凹口"，不是一排整齐的齿。
 * 数据存角度 offset（绕主圆中心、相对「咬口指向曲奇中心」的方向展开）
 * 与半径因子；模块级固定随机，避免逐帧闪烁。 */
const BITE_EDGES: { off: number; rf: number }[] = (() => {
  let s = 7919;
  const rnd = () => {
    s = (s * 16807) % 2147483647;
    return s / 2147483647;
  };
  const N = 9;
  const arr: { off: number; rf: number }[] = [];
  for (let i = 0; i < N; i++) {
    const t = i / (N - 1);
    arr.push({
      off: -1.5 + t * 3 + (rnd() - 0.5) * 0.45, // 沿主圆边缘铺开大半圈
      rf: 0.045 + rnd() * 0.07, // 细碎小圆，只为啃出轻微锯齿
    });
  }
  return arr;
})();

/* 局部(含质心补偿平移 + 旋转) → 画布坐标 */
function localToWorld(px: number, py: number, cx: number, cy: number) {
  const qx = px + centeringDx();
  const qy = py + centeringDy();
  const c = Math.cos(angle.value);
  const s = Math.sin(angle.value);
  return { x: cx + qx * c - qy * s, y: cy + qx * s + qy * c };
}

let centeringDx = () => 0;
let centeringDy = () => 0;

/* ---- 背景：斜向滚动的格子桌布 ----
 * 配色说明：用冷调柔和蓝灰，与 UI 深蓝灰背景(#0b0d12)同系，
 * 又与暖棕曲奇形成冷暖互补对比，让曲奇成为画面唯一焦点。
 * 之前用高饱和正红 #e4574f，既跟深蓝灰 UI 冲突，又和曲奇同属暖色、明度接近，
 * 主体被背景糊住——所以换成低饱和冷色。想换风格改这两个常量即可。
 * 备选(暖褐木质，跟 accent 同系)：CLOTH_A="#4a382a"、CLOTH_B="#6b5442"
 * 备选(经典红白野餐布，更活泼)：CLOTH_A="#c2605a"、CLOTH_B="#f3e6d2" */
const CLOTH_A = "#E19859"; // 调浅的木色，避免过深
const CLOTH_B = "#f4efe6"; // 浅格改暖米白，与木质同系又不刺眼

function drawTablecloth(g: CanvasRenderingContext2D, t: number) {
  const tile = 46;
  const speed = 40; // px/s，x/y 同步 → 斜向
  const off = (speed * t) % tile;
  const nX = Math.ceil(cssW / tile) + 2;
  const nY = Math.ceil(cssH / tile) + 2;
  for (let j = -1; j < nY; j++) {
    for (let i = -1; i < nX; i++) {
      const even = (i + j) & 1;
      g.fillStyle = even ? CLOTH_A : CLOTH_B;
      g.fillRect(i * tile + off, j * tile + off, tile + 1, tile + 1);
    }
  }
}

/* ---- 桌布上的柔和投影 ---- */
function drawShadow(g: CanvasRenderingContext2D, cx: number, cy: number) {
  g.save();
  g.translate(cx, cy + R * 0.55);
  g.scale(1, 0.3);
  const grad = g.createRadialGradient(0, 0, 0, 0, 0, R * 1.15);
  grad.addColorStop(0, "rgba(20,10,5,0.28)");
  grad.addColorStop(1, "rgba(20,10,5,0)");
  g.fillStyle = grad;
  g.beginPath();
  g.arc(0, 0, R * 1.15, 0, Math.PI * 2);
  g.fill();
  g.restore();
}

/* ---- 图片未加载时的备用程序化曲奇 ---- */
function drawCookieShape(g: CanvasRenderingContext2D, rr: number) {
  g.save();
  g.shadowColor = "rgba(0,0,0,0.12)";
  g.shadowBlur = 4;
  const body = g.createRadialGradient(-rr * 0.3, -rr * 0.35, rr * 0.1, 0, 0, rr * 1.1);
  body.addColorStop(0, "#d99a5a");
  body.addColorStop(1, "#a76a38");
  g.fillStyle = body;
  g.beginPath();
  g.arc(0, 0, rr, 0, Math.PI * 2);
  g.fill();
  g.shadowBlur = 0;
  g.fillStyle = "rgba(60,32,16,0.9)";
  const chips = [
    [-0.42, -0.25, 0.1], [0.1, -0.48, 0.12], [0.45, 0.1, 0.09],
    [-0.05, 0.42, 0.11], [-0.5, 0.3, 0.08], [0.35, 0.42, 0.09],
  ];
  for (const [sx, sy, sr] of chips) {
    g.beginPath();
    g.arc(sx * rr, sy * rr, sr * rr, 0, Math.PI * 2);
    g.fill();
  }
  g.restore();
}

/* ---- 在离屏层画曲奇 + 咬口（避免擦穿桌布） ---- */
function drawCookie() {
  if (!offCtx || !offscreen) return;
  offCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
  offCtx.clearRect(0, 0, cssW, cssH);

  const cx = cssW / 2;
  const cy = cssH / 2;
  R = cookieRadius();
  biteR = R * 0.5;
  biteDc = R * 0.82;

  // Procrustes 对齐：把咬掉缺口造成的质心偏移拉回画布中心。
  // 位移由独立的 recoil 弹簧驱动（欠阻尼 → 冲过头再弹回），不再是瞬间到位。
  const fullArea = Math.PI * R * R;
  const removed = lensArea(R, biteR, biteDc);
  const bv = Math.min(1, Math.max(0, bite.value));
  const k = recoil.value * (removed / Math.max(1e-6, fullArea - removed));
  const bx0 = biteDc * Math.cos(BITE_PHI);
  const by0 = biteDc * Math.sin(BITE_PHI);
  // 咬口在曲奇右上（bx0>0, by0<0）。曲奇应往「远离咬口」的方向（左下）躲闪，
  // 所以补偿平移取反：-k*bx0（左）、-k*by0（下）。
  centeringDx = () => -k * bx0;
  centeringDy = () => -k * by0;

  offCtx.save();
  offCtx.translate(cx, cy);
  offCtx.rotate(angle.value);
  offCtx.translate(centeringDx(), centeringDy());

  const d = R * 2;
  if (imgLoaded) {
    offCtx.drawImage(cookieImg, -R, -R, d, d);
  } else {
    drawCookieShape(offCtx, R);
  }

  // 咬口：destination-out 一次性填充「主圆 + 沿其圆周朝曲奇内部的小圆」。
  // 主圆给出基本凹口，边缘小圆（大小不一、圆心落在主圆圆周上）把光滑的
  // 圆凹边界啃成参差波浪，整体仍像一个大凹口而非一排齿。
  // 全部子路径同向 → 非零环绕 = 并集，透明度由 globalAlpha 整体控制。
  const depth = bv;
  if (depth > 0.002) {
    // 咬口圆心指向曲奇中心的方向角，作为小圆展开的基准角
    const towardCenter = Math.atan2(-by0, -bx0);
    const path = new Path2D();
    path.arc(bx0, by0, biteR, 0, Math.PI * 2);
    for (const e of BITE_EDGES) {
      const a = towardCenter + e.off;
      path.arc(bx0 + biteR * Math.cos(a), by0 + biteR * Math.sin(a), biteR * e.rf, 0, Math.PI * 2);
    }
    offCtx.globalCompositeOperation = "destination-out";
    offCtx.globalAlpha = depth;
    offCtx.fill(path);
    offCtx.globalAlpha = 1;
    offCtx.globalCompositeOperation = "source-over";
  }
  offCtx.restore();
}

function drawCrumbs(g: CanvasRenderingContext2D) {
  for (const cr of crumbs) {
    const alpha = Math.max(0, 1 - cr.age / cr.life);
    g.globalAlpha = alpha;
    g.fillStyle = "#8a5a2e";
    g.beginPath();
    g.arc(cr.x, cr.y, cr.size, 0, Math.PI * 2);
    g.fill();
  }
  g.globalAlpha = 1;
}

function drawVignette(g: CanvasRenderingContext2D) {
  const grad = g.createRadialGradient(
    cssW / 2, cssH / 2, Math.min(cssW, cssH) * 0.3,
    cssW / 2, cssH / 2, Math.max(cssW, cssH) * 0.75,
  );
  grad.addColorStop(0, "rgba(0,0,0,0)");
  grad.addColorStop(1, "rgba(30,10,5,0.42)");
  g.fillStyle = grad;
  g.fillRect(0, 0, cssW, cssH);
}

/* ---- 彩蛋阶段一：像素方块飞出画面 ----
 * 每个碎片 = 网格里的一块正方形：裁剪后按「飞散位移 × 进度」平移绘制。
 * 位置是 shatterT 的纯函数（不逐帧积分）；方块不旋转 —— 旋转会破坏
 * 像素画的对齐感（用户反馈：扇形切块非常难看）。
 * 带重力下坠 + 尾段淡出：方块坠出画面下缘，给「掉出去了」的交代。 */
function drawFragments(g: CanvasRenderingContext2D, cx: number, cy: number) {
  const t = shatterT;
  const easeOutCubic = (u: number) => 1 - Math.pow(1 - u, 3);
  const G = 900; // 重力加速度（px/s²）：飞散期末方块已坠出画面下缘（fall≈495px）
  const u = Math.min(1, t / SHATTER_FLY);
  const k = easeOutCubic(u);
  const fall = 0.5 * G * t * t;
  // 最后 0.12s 线性淡出 —— 只给个别还没滚出画框的方块兜底
  const FADE = 0.12;
  const alpha = Math.max(0, Math.min(1, (SHATTER_FLY - t) / FADE + 1));
  g.globalAlpha = alpha;
  for (const f of frags) {
    g.save();
    g.translate(cx + f.dx * k, cy + f.dy * k + fall);
    g.beginPath();
    g.rect(f.gx, f.gy, f.w, f.h);
    g.clip();
    if (imgLoaded) {
      g.drawImage(cookieImg, -R, -R, R * 2, R * 2);
    } else {
      drawCookieShape(g, R);
    }
    g.restore();
  }
  g.globalAlpha = 1;
}

/* ---- 苦力怕头：真实皮肤贴图（MHF_Creeper 标准脸） ----
 * imageSmoothing 关掉做最近邻放大，保持像素感；盯着时轻轻摆头。 */
const creeperFaceImg = new Image();
creeperFaceImg.src = creeperFaceUrl;
let faceLoaded = false;
creeperFaceImg.onload = () => {
  faceLoaded = true;
};

/** 苦力怕 = 身体（素色绿两节）+ 贴图头；绘制在曲奇**之下**，
 * 探头时从曲奇右下缘后钻出来，拖走时被曲奇挡着一起出画。 */
function drawCreeperBody(g: CanvasRenderingContext2D, x: number, y: number, hs: number, t: number) {
  // 身体 + 两条短腿（颜色与脸的皮肤同系）
  const bw = hs * 0.56;
  g.fillStyle = "#4a9c46";
  g.fillRect(x - bw / 2, y + hs / 2 - 2, bw, hs * 0.42);
  g.fillStyle = "#3c7f35";
  const lw = bw * 0.44;
  const legY = y + hs / 2 + hs * 0.42 - 2;
  g.fillRect(x - bw / 2 + bw * 0.06, legY, lw, hs * 0.22);
  g.fillRect(x + bw / 2 - bw * 0.06 - lw, legY, lw, hs * 0.22);
  // 头：真实贴图，最近邻放大；盯着时轻微摆头
  if (!faceLoaded) {
    drawCreeper(g, x, y + hs * 0.875, hs, t, true);
    return;
  }
  g.save();
  g.imageSmoothingEnabled = false;
  const tilt = t < DRAG_T ? Math.sin(t * 6) * 0.05 : 0;
  g.translate(x, y);
  g.rotate(tilt);
  g.drawImage(creeperFaceImg, -hs / 2, -hs / 2, hs, hs);
  g.restore();
}

/* ---- 苦力怕：像素画 + 走进/叼走/跑掉 ----
 * 皮肤：8×8 经典脸（黑眼 + 嘴），底色用种子随机的深浅绿斑驳；
 * 走路 = 位置随时间推进 + 轻微上下颠簸，叼住瞬间震动一次。 */
const CREEPER_SKIN: number[] = (() => {
  let s = 1337;
  const rnd = () => {
    s = (s * 16807) % 2147483647;
    return s / 2147483647;
  };
  return Array.from({ length: 64 }, () => rnd());
})();

function drawCreeper(
  g: CanvasRenderingContext2D,
  x: number,
  groundY: number,
  size: number,
  walkT: number,
  moving: boolean,
) {
  const u = size / 8; // 像素格（头 = 8×8）
  const bob = moving ? Math.abs(Math.sin(walkT * 8)) * u * 0.6 : 0; // 走路颠簸
  const swing = moving ? Math.sin(walkT * 8) * u * 1.3 : 0; // 腿的前后摆动
  // 落地投影：先铺一块柔和椭圆，苦力怕才不是「浮」在桌布上
  g.save();
  g.globalAlpha = 0.2;
  g.fillStyle = "#000";
  g.beginPath();
  g.ellipse(x, groundY + u * 0.2, size * 0.55, u * 0.9, 0, 0, Math.PI * 2);
  g.fill();
  g.restore();

  const top = groundY - 14 * u - bob; // 总高 14u：头8 + 身3 + 腿3
  const hx = x - 4 * u;
  // 腿：两对（侧视前后各一），走路时交替摆动；停住时并拢站直
  g.fillStyle = "#3c7f35";
  g.fillRect(hx + u + swing, top + 11 * u, 2 * u, 3 * u);
  g.fillRect(hx + 5 * u - swing, top + 11 * u, 2 * u, 3 * u);
  // 身体：比头窄一档，衔接头与腿
  g.fillStyle = "#478f3d";
  g.fillRect(hx + 2 * u, top + 8 * u, 4 * u, 3 * u);
  // 头：三档绿的随机斑驳（低噪点，避免「脏」感）
  const skin = ["#3f8a37", "#54ac47", "#68c25a"];
  for (let r = 0; r < 8; r++) {
    for (let c = 0; c < 8; c++) {
      const v = CREEPER_SKIN[r * 8 + c];
      g.fillStyle = v < 0.28 ? skin[0] : v < 0.85 ? skin[1] : skin[2];
      g.fillRect(hx + c * u, top + r * u, u + 0.5, u + 0.5);
    }
  }
  // 经典脸：双眼 + 上窄下宽的嘴（一直裂到头底缘）
  g.fillStyle = "#0d120e";
  g.fillRect(hx + u, top + 2 * u, 2 * u, 2 * u);
  g.fillRect(hx + 5 * u, top + 2 * u, 2 * u, 2 * u);
  g.fillRect(hx + 3 * u, top + 4 * u, 2 * u, 4 * u);
  g.fillRect(hx + 2 * u, top + 5 * u, u, 3 * u);
  g.fillRect(hx + 5 * u, top + 5 * u, u, 3 * u);
  // 深色描边：在浅色桌布上勾出轮廓，避免一团绿糊成背景
  g.strokeStyle = "rgba(18,26,16,0.5)";
  g.lineWidth = 1;
  g.strokeRect(hx + 0.5, top + 0.5, 8 * u - 1, 8 * u - 1);
}

/** 苦力怕在 shatterT 时刻的位置（相对曲奇中心 cx,cy）：
 * 藏在曲奇正后方 → smoothstep 滑到曲奇「中心稍下稍右」处探出头身
 * → 盯一拍 → 拖走阶段与曲奇共用同一位移向量（整体被拽出右下角）。 */
function headPos(t: number, cx: number, cy: number): { x: number; y: number } {
  const hide = { x: cx + R * 0.15, y: cy + R * 0.3 };
  const peek = { x: cx + R * 0.95, y: cy + R * 0.75 };
  if (t < PEEK_T) {
    const k = Math.min(1, Math.max(0, (t - PEEK0) / RISE_DUR));
    const e = k * k * (3 - 2 * k);
    return { x: hide.x + (peek.x - hide.x) * e, y: hide.y + (peek.y - hide.y) * e };
  }
  if (t < DRAG_T) {
    return { x: peek.x, y: peek.y + Math.sin(t * 7) * R * 0.04 }; // 盯着时轻微晃
  }
  const k = Math.min(1, (t - DRAG_T) / DRAG_DUR);
  const ease = k * k;
  // 与 drawFallingCookie 的拖走轨迹完全同位移 → 曲奇像被它抱着一并拽走
  return {
    x: peek.x + (cssW + R * 0.55 - cx) * ease,
    y: peek.y + (cssH + R * 1.35 - cy) * ease,
  };
}

/** 曲奇被拖走的进度 0..1（DRAG 阶段）；>=1 表示已拖出画面。 */
function stealK(t: number): number {
  if (t < DRAG_T) return 0;
  return Math.min(1, (t - DRAG_T) / DRAG_DUR);
}

/* ---- 彩蛋阶段二：新曲奇从屏幕顶部落下，落地「脆裂」 ----
 * 饼干是**脆**的：下落加速（不弹跳），落地硬停，只给一次
 * 「快速压扁 + 立刻回弹」并崩出几粒碎屑 —— 不是 Q 弹，是咔嚓一声碎感。 */
let landDone = false; // 落地演出（掉渣/轻震）只触发一次
let grabBuzzed = false; // 苦力怕叼住的震动只触发一次
let secondPass = false; // 第二趟：苦力怕偷走后，新曲奇再从顶上落一次（补回落下动画）

function drawFallingCookie(g: CanvasRenderingContext2D, cx: number, cy: number) {
  const t0 = SHATTER_FLY + SHATTER_WAIT; // 开始下落的时刻
  const u = Math.min(1, Math.max(0, (shatterT - t0) / DROP_DUR));
  const p = u * u; // 自由落体：加速下坠
  const startY = -R - 24;
  const y = startY + (cy - startY) * p;

  // 落地那一帧：脆裂 —— 掉渣 + 白色扬尘 + 轻震，然后只留一次快速压扁回弹
  const landT = t0 + DROP_DUR;
  if (!landDone && shatterT >= landT) {
    landDone = true;
    spawnBurst(12, { x: cx, y: cy + R * 0.8 }); // 曲奇色碎渣崩开
    spawnMcBurst(cx, cy + R * 0.85, 22); // 白色扬尘
    buzz();
  }

  // 落地压扁：单次快速衰减（无震荡）—— 脆，不是弹
  let sx = 1;
  let sy = 1;
  if (shatterT > landT) {
    const dtc = shatterT - landT;
    const wobble = Math.exp(-9 * dtc);
    sy = 1 - 0.1 * wobble;
    sx = 1 + 0.07 * wobble;
  }

  // 被拖走：stealK 0→1 期间曲奇滑向右下角（跟着苦力怕缩回底边外）；拖走后不再绘制。
  // 第二趟（补落下动画）不走这段 —— 新曲奇正常落地。
  const sk = secondPass ? 0 : stealK(shatterT);
  if (sk >= 1) return;
  const ease = sk * sk; // 越拖越快，像被猛地拽走
  const mx = cx + (cssW + R * 0.55 - cx) * ease;
  const my = y + (cssH + R * 1.35 - y) * ease;

  g.save();
  // 以曲奇**底缘**为原点做压扁，视觉上才是「落在地上被压了一下」
  g.translate(mx, my + R);
  g.rotate(sk * 0.5); // 拖走时顺势一歪
  g.scale(sx, sy);
  if (imgLoaded) {
    g.drawImage(cookieImg, -R, -R * 2, R * 2, R * 2);
  } else {
    g.translate(0, -R);
    drawCookieShape(g, R);
  }
  g.restore();
}

function draw(t: number) {
  const g = ctxRef;
  if (!g) return;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, cssW, cssH);
  drawTablecloth(g, t);
  const cx = cssW / 2;
  const cy = cssH / 2;
  if (shatterT >= 0) {
    // 苦力怕先画（在曲奇之下）：第一趟从曲奇后面钻出来；第二趟（补落下）不出场
    if (shatterT >= PEEK0 && !secondPass) {
      const hp = headPos(shatterT, cx, cy);
      drawCreeperBody(g, hp.x, hp.y, R * 1.15, shatterT);
    }
    // 炸碎阶段：方块先全部坠出画面 → 空场停顿一拍 → 新曲奇才从顶部落下
    if (shatterT < SHATTER_FLY) drawFragments(g, cx, cy);
    else if (shatterT >= SHATTER_FLY + SHATTER_WAIT) drawFallingCookie(g, cx, cy);
    drawMcParts(g);
  } else {
    drawShadow(g, cx, cy);
    drawCookie();
    if (offscreen) g.drawImage(offscreen, 0, 0, cssW, cssH);
    drawMcParts(g);
  }
  drawCrumbs(g);
  drawVignette(g);
}

/* ---- 尺寸 / 渲染循环 ---- */
function resize() {
  const wrap = wrapRef.value;
  const canvas = canvasRef.value;
  if (!wrap || !canvas) return;
  dpr = Math.min(window.devicePixelRatio || 1, 2);
  cssW = Math.max(1, Math.round(wrap.clientWidth));
  cssH = Math.max(1, Math.round(wrap.clientHeight));
  const bw = Math.round(cssW * dpr);
  const bh = Math.round(cssH * dpr);
  if (canvas.width !== bw || canvas.height !== bh) {
    canvas.width = bw;
    canvas.height = bh;
  }
  if (!offscreen || offscreen.width !== bw || offscreen.height !== bh) {
    offscreen = document.createElement("canvas");
    offscreen.width = bw;
    offscreen.height = bh;
    offCtx = offscreen.getContext("2d");
  }
}

function frame(now: number) {
  const dtReal = Math.min((now - last) / 1000, 0.1);
  last = now;
  const canvas = canvasRef.value;
  const active = !!canvas && canvas.clientWidth > 0 && !document.hidden;
  if (active) {
    elapsed += dtReal;
    // 固定小步长累积推进物理，保证弹簧积分稳定顺滑
    acc += dtReal;
    let guard = 0;
    while (acc >= STEP && guard < 40) {
      updatePhysics();
      acc -= STEP;
      guard++;
    }
    draw(elapsed);
  }
  raf = requestAnimationFrame(frame);
}

let ro: ResizeObserver | null = null;

onMounted(() => {
  const canvas = canvasRef.value;
  if (canvas) ctxRef = canvas.getContext("2d");
  resize();
  last = performance.now();
  raf = requestAnimationFrame(frame);
  ro = new ResizeObserver(() => resize());
  if (wrapRef.value) ro.observe(wrapRef.value);
});

onUnmounted(() => {
  cancelAnimationFrame(raf);
  ro?.disconnect();
  crumbs.length = 0;
  clearTimeout(achieveTimer);
});
</script>

<template>
  <div ref="wrapRef" class="about-show-wrap">
    <!-- 彩蛋：900ms 内连点 7 次，曲奇炸碎 → 新曲奇落下 → 苦力怕溜进来叼走 → 成就弹窗 -->
    <canvas ref="canvasRef" class="about-show-canvas" @pointerdown="onTap"></canvas>
    <!-- MC 风格成就横幅：暗色面板 + 黄色标题，滑入/收回 -->
    <Transition name="achv">
      <div v-if="showAchieve" class="achv-banner" aria-live="polite">
        <img class="achv-icon-img" :src="appIconAndroid" alt="" />
        <div class="achv-text">
          <div class="achv-title">成就已达成！</div>
          <div class="achv-name">偷吃曲奇的人</div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.about-show-wrap {
  position: relative;
  width: 100%;
  height: 200px;
  overflow: hidden;
}
.about-show-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: block;
}

/* ---- 成就横幅（MC 风格：暗面板 + 黄标题，居中滑入） ---- */
.achv-banner {
  position: absolute;
  top: 10px;
  left: 50%;
  translate: -50% 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 14px 7px 9px;
  background: #212121;
  border: 2px solid #555;
  box-shadow:
    0 0 0 2px #000,
    0 4px 12px rgba(0, 0, 0, 0.45);
  pointer-events: none; /* 别挡住继续点曲奇 */
  z-index: 2;
}
/* 成就徽章 = 完整曲奇 + 白底圆角（与修复后的桌面图标一致） */
.achv-icon-img {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: #fff;
  object-fit: contain;
  flex-shrink: 0;
}
.achv-title {
  font-size: 12px;
  font-weight: 700;
  color: #ffff55; /* MC 成就的标志性黄 */
  line-height: 1.25;
}
.achv-name {
  font-size: 12px;
  color: #fff;
  line-height: 1.25;
}
.achv-enter-active,
.achv-leave-active {
  transition:
    opacity 0.25s ease,
    translate 0.25s ease;
}
.achv-enter-from,
.achv-leave-to {
  opacity: 0;
  translate: -50% -14px;
}
</style>
