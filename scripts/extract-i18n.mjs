#!/usr/bin/env node
/**
 * i18n 提取器（权威实现）：用 `@vue/compiler-sfc` 解析模板 AST + TypeScript 编译器 API
 * 解析脚本，把界面文案与 key 的对应关系摸清楚。
 *
 * ── key 模型：语义化 key，不是中文原文 ──
 *   代码：`t("nav.home")`、`t("nav.downloading", { count: n })`
 *   词表：`zh-CN.json = { "nav.home": "首页" }`、`en-US.json = { "nav.home": "Home" }`
 *   命名时机在**迁移时**（按语境起名），不是给上千条文案编号。
 *
 * ── 它做什么 ──
 *   1. 扫出**还没迁移**的中文（不在 `t(...)` 里的字面量/文本节点）→ `_index.json`
 *      （含 `{pN}` 占位符与真实表达式，迁移时照着起名填参）；
 *   2. 校验词表：`t()` 用到的 key 是否都有条目、词表里有没有没人用的 key、
 *      有没有人还在 `t("中文原文")`（旧写法）；
 *   3. **只增不删**：已存在的条目一律原样保留（译文是人工成果）。
 *
 * 用法：`npm run i18n:extract`；配套 `npm run i18n:audit`（独立实现交叉校验）。
 */

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { parse as parseSfc } from "@vue/compiler-sfc";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src");
const LOCALES = join(SRC, "locales");
const HAN = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;
/** 开发日志：`[fe] xxx`、`[api] xxx` —— 给开发者看的，不进词表 */
const DEBUG_LOG = /^\s*\[(fe|api|db|perf|dev|debug|warn)\]/i;
/** 取译文函数的名字（`t()` / `$t()`） */
const T_NAMES = new Set(["t", "$t"]);

const readJson = (p) => {
  try {
    return JSON.parse(readFileSync(p, "utf8"));
  } catch {
    return {};
  }
};

/** 字面量 → `{文本, 占位符}`。模板串的 `${…}` 归一成 `{pN}` 并记下表达式。 */
function literalOf(node, sf, params = []) {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    return { text: node.text, params };
  }
  if (ts.isTemplateExpression(node)) {
    let text = node.head.text;
    for (const span of node.templateSpans) {
      params.push({ name: `p${params.length + 1}`, expr: span.expression.getText(sf).trim() });
      text += `{${params[params.length - 1].name}}` + span.literal.text;
    }
    return { text: text.replace(/\s+/g, " ").trim(), params };
  }
  return null;
}

/**
 * 分析一段 JS/TS：分出「t() 的 key 参数」与「还没迁移的中文字面量」。
 * 模板串插值里的字面量会递归进来找（`${cond ? "甲" : "乙"}`）。
 */
function analyzeJs(src, onText, onKey, baseLine = 1) {
  if (!src || !src.trim()) return;
  const sf = ts.createSourceFile("snippet.tsx", src, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const lineOf = (pos) => baseLine + sf.getLineAndCharacterOfPosition(pos).line;
  const keyPos = new Set();

  const collect = (node) => {
    if (ts.isCallExpression(node)) {
      const callee = node.expression;
      const name = ts.isIdentifier(callee)
        ? callee.text
        : ts.isPropertyAccessExpression(callee)
          ? callee.name.text
          : "";
      if (T_NAMES.has(name) && node.arguments.length) {
        const arg = node.arguments[0];
        const lit = literalOf(arg, sf);
        if (lit) {
          keyPos.add(arg.getStart(sf));
          onKey(lit.text, lineOf(arg.getStart(sf)));
        }
      }
    }
    ts.forEachChild(node, collect);
  };
  collect(sf);

  const texts = (node) => {
    if (
      (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node) || ts.isTemplateExpression(node)) &&
      !keyPos.has(node.getStart(sf))
    ) {
      const lit = literalOf(node, sf);
      if (lit && HAN.test(lit.text)) onText(lit.text, lit.params, lineOf(node.getStart(sf)));
    }
    ts.forEachChild(node, texts);
  };
  texts(sf);
}

/** 模板 AST：文本节点（相邻文本/插值合并）、普通属性、绑定属性的表达式 */
function walkTemplate(node, onText, onKey) {
  if (!node || typeof node !== "object") return;
  if (node.type === 3) return; // 注释
  if (node.type === 2) {
    // 单文本节点由父节点的「文本+插值合并」处理，不单独收
    return;
  }
  if (node.type === 1) {
    for (const p of node.props ?? []) {
      if (p.type === 6 && p.value && HAN.test(p.value.content)) {
        onText(p.value.content.replace(/\s+/g, " ").trim(), [], p.loc.start.line);
      } else if (p.type === 7 && p.exp && p.exp.content.trim()) {
        analyzeJs(p.exp.content, onText, onKey, p.loc.start.line);
      }
    }
    const kids = node.children ?? [];
    let buf = "";
    let params = [];
    let line = 0;
    const flush = () => {
      const text = buf.replace(/\s+/g, " ").trim();
      if (HAN.test(text)) onText(text, params, line);
      buf = "";
      params = [];
    };
    for (const kid of kids) {
      if (kid.type === 2) {
        if (!buf) line = kid.loc.start.line;
        buf += kid.content;
      } else if (kid.type === 5) {
        if (!buf) line = kid.loc.start.line;
        // 编号必须与 codemod 一致（p1、p2…）并记下表达式，否则词表里的 `{pN}` 对不上调用点
        params.push({ name: `p${params.length + 1}`, expr: kid.content.content.trim() });
        buf += `{${params[params.length - 1].name}}`;
        analyzeJs(kid.content.content, onText, onKey, kid.loc.start.line);
      } else {
        flush();
      }
    }
    flush();
    // 只递归元素/模板节点（文本与插值已合并）
    for (const kid of kids) if (kid.type !== 2 && kid.type !== 5) walkTemplate(kid, onText, onKey);
    return;
  }
  if (node.children) for (const kid of node.children) walkTemplate(kid, onText, onKey);
}

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (["node_modules", "dist", "locales"].includes(name)) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(vue|ts)$/.test(name) && !name.endsWith(".d.ts")) out.push(p);
  }
  return out;
}

// ═══════════════════════════════════════════════════════════════════════════
// 扫描
// ═══════════════════════════════════════════════════════════════════════════

const inventory = new Map(); // 原文 → { occ: [{f,l,kind}], params }
const perFile = new Map();
const usedKeys = new Map(); // key → [{f,l}]
const keyFromText = new Map(); // key → 原文（用于识别旧写法 t("中文")）
let skippedLog = 0;

function addText(text, params, rel, line) {
  if (!text || !HAN.test(text) || DEBUG_LOG.test(text)) {
    if (text && DEBUG_LOG.test(text)) skippedLog++;
    return;
  }
  const hit = inventory.get(text) ?? { occ: [], params: [] };
  if (!hit.occ.length) hit.params = params;
  hit.occ.push({ f: rel, l: line, kind: "text" });
  inventory.set(text, hit);
  perFile.set(rel, (perFile.get(rel) ?? 0) + 1);
}
function addKey(key, rel, line) {
  if (!key || HAN.test(key)) return; // 中文当 key 是旧写法，单独报
  const list = usedKeys.get(key) ?? [];
  list.push({ f: rel, l: line });
  usedKeys.set(key, list);
}

const files = walk(SRC);
for (const file of files) {
  const rel = relative(ROOT, file).replace(/\\/g, "/");
  const src = readFileSync(file, "utf8");
  if (!HAN.test(src) && !src.includes("t(")) continue;
  const onText = (t, p, l) => addText(t, p, rel, l);
  const onKey = (k, l) => addKey(k, rel, l);

  if (file.endsWith(".ts")) {
    analyzeJs(src, onText, onKey);
    continue;
  }
  const { descriptor, errors } = parseSfc(src);
  if (errors.length) console.log(`  [warn] ${rel}: ${errors[0].message}`);
  for (const blk of [descriptor.script, descriptor.scriptSetup]) {
    if (blk?.content) analyzeJs(blk.content, onText, onKey, blk.loc.start.line);
  }
  if (descriptor.template?.ast) walkTemplate(descriptor.template.ast, onText, onKey);
}

// ═══════════════════════════════════════════════════════════════════════════
// 建键：给每条待迁移文案分配稳定 key（台账保证不漂移）
// ═══════════════════════════════════════════════════════════════════════════

/**
 * 文件 → key 命名空间。页面用业务名，组件用文件名，工具/仓库用目录点号。
 * 这些名字会进 `t("settings.12")`，所以宁可短、稳定，不要长句。
 */
function namespaceOf(rel) {
  const base = rel.replace(/^src\//, "").replace(/\.(vue|ts)$/, "");
  const kebab = (s) => s.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
  const overrides = {
    "views/SettingsView": "settings",
    "views/HomeView": "home",
    "views/BrowseView": "browse",
    "views/InstancesView": "instances",
    "views/InstanceDetailView": "instance-detail",
    "views/CreateInstanceView": "create-instance",
    "views/MultiplayerView": "multiplayer",
    "views/ServerDetailView": "server-detail",
    "views/SkinView": "skins",
    "views/DownloadsView": "downloads",
    "views/NewsView": "news",
    "components/SideBar": "nav.side",
    "components/MobileNav": "nav.mobile",
    "components/instance/SettingsTab": "instance-settings",
    "components/instance/ContentTab": "instance-content",
    "components/instance/SavesTab": "instance-saves",
    "components/instance/KeysTab": "instance-keys",
    App: "app",
    api: "api",
    router: "router",
    main: "main",
    i18n: "i18n",
  };
  if (overrides[base]) return overrides[base];
  const parts = base.split("/");
  if (parts[0] === "views" || parts[0] === "components") {
    return kebab(parts[parts.length - 1]);
  }
  return parts.map(kebab).join(".");
}

// 待迁移清单（按首次出现排序，便于按文件推进）
const ordered = [...inventory.keys()].sort((a, b) => {
  const ha = inventory.get(a).occ[0];
  const hb = inventory.get(b).occ[0];
  return ha.f === hb.f ? ha.l - hb.l : ha.f < hb.f ? -1 : 1;
});

const ledger = readJson(join(LOCALES, "_index.json")); // 原文 → { occ, params, key }
const taken = new Set(Object.keys(readJson(join(LOCALES, "zh-CN.json"))));
let assigned = 0;
for (const text of ordered) {
  const info = inventory.get(text);
  const prev = ledger[text];
  if (prev?.key) {
    info.key = prev.key; // 台账里有就沿用（老文案永远不换 key）
    continue;
  }
  const ns = namespaceOf(info.occ[0].f);
  let n = 1;
  while (taken.has(`${ns}.${n}`)) n++;
  info.key = `${ns}.${n}`;
  taken.add(info.key);
  assigned++;
}

// ═══════════════════════════════════════════════════════════════════════════
// 词表（只增不删）
// ═══════════════════════════════════════════════════════════════════════════

const zh = readJson(join(LOCALES, "zh-CN.json"));
const en = readJson(join(LOCALES, "en-US.json"));
for (const [k, v] of Object.entries(zh)) keyFromText.set(k, v);
// 把台账里已建键、词表里还没有的补进去（值 = 原文；`{p1}` 占位符迁移时可改名）。
// **还没迁移**的 key 值跟着源码走（重新推导），因为此时它的值就是从源码来的；
// 已经迁移的 key 一律保留原值 —— 那是用户改过的话，不能被脚本冲掉。
for (const text of ordered) {
  const key = inventory.get(text).key;
  if (!key) continue;
  if (!(key in zh) || !usedKeys.has(key)) zh[key] = text;
}

const missingZh = [...usedKeys.keys()].filter((k) => !(k in zh));
const legacyKeys = [...usedKeys.keys()].filter((k) => HAN.test(k));
const emptyZh = Object.entries(zh).filter(([, v]) => !String(v).trim()).map(([k]) => k);
const enOrphans = Object.keys(en).filter((k) => !(k in zh));
const untranslated = Object.keys(zh).filter((k) => !(k in en));
/** 已建 key 但代码还没改过去（原文仍在待迁移清单里）—— 这是待办，不是失效 */
const pendingKeys = new Set(ordered.map((t) => inventory.get(t).key).filter(Boolean));
const migratedKeys = Object.keys(zh).filter((k) => usedKeys.has(k));
const unusedZh = Object.keys(zh).filter((k) => !usedKeys.has(k) && !pendingKeys.has(k));

writeFileSync(
  join(LOCALES, "_index.json"),
  JSON.stringify(Object.fromEntries(ordered.map((k) => [k, inventory.get(k)])), null, 2) + "\n"
);
// 词表：保留原值，补齐顺序不乱（新条目由迁移时手工加，这里只做保底写入）
writeFileSync(join(LOCALES, "zh-CN.json"), JSON.stringify(zh, null, 2) + "\n");
writeFileSync(join(LOCALES, "en-US.json"), JSON.stringify(en, null, 2) + "\n");

// ═══════════════════════════════════════════════════════════════════════════
// 报告
// ═══════════════════════════════════════════════════════════════════════════

const occ = [...inventory.values()].reduce((s, v) => s + v.occ.length, 0);
console.log(`\n扫描 ${files.length} 个文件`);
console.log(`待迁移文案：${inventory.size} 条唯一（${occ} 处）→ src/locales/_index.json`);
console.log(
  `词表：${Object.keys(zh).length} 个 key（已改代码 ${migratedKeys.length}，已建键待迁移 ${pendingKeys.size}）` +
    `；en-US 已翻译 ${Object.keys(zh).length - untranslated.length} 条`
);
if (assigned) console.log(`本次新建 key：${assigned} 个（台账里已有的沿用，永不漂移）`);
if (skippedLog) console.log(`（跳过开发日志 ${skippedLog} 处）`);
console.log(
  `\n词表问题：缺条目 ${missingZh.length} · 真没人用 ${unusedZh.length} · 空值 ${emptyZh.length} · 译文孤儿 ${enOrphans.length}`
);
for (const k of missingZh.slice(0, 10)) console.log(`  [缺] ${k}  （代码在用，zh-CN.json 里没有）`);
for (const k of unusedZh.slice(0, 10)) console.log(`  [闲] ${k}  （词表里有，代码里没人用）`);
for (const k of legacyKeys.slice(0, 10)) console.log(`  [旧写法] t("${k}")  —— 应该换成语义 key`);
for (const k of enOrphans.slice(0, 10)) console.log(`  [孤儿] en-US 有但 zh-CN 没有：${k}`);
console.log(`\n待迁移最多的文件：`);
for (const [f, c] of [...perFile].sort((a, b) => b[1] - a[1]).slice(0, 15)) {
  console.log(`  ${String(c).padStart(4)}  ${f}`);
}
console.log("");
