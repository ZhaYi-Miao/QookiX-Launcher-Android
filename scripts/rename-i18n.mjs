#!/usr/bin/env node
/**
 * key 重命名工具。编号 key（`settings.12`）升级成语义名（`settings.memory-auto`）就靠它。
 *
 * ── 为什么现在必须做 ──
 * 现在换，工具改一遍代码 + 词表 + 台账就完事；等 en-US 翻译铺开、key 被更多地方引用之后，
 * 每次改名都是三边联动 + 翻译对齐，工作量爆炸。
 *
 * ── 用法 ──
 *   node scripts/rename-i18n.mjs --suggest      生成 `src/locales/_rename.json`（改名建议）
 *   node scripts/rename-i18n.mjs --apply src/locales/_rename.json
 *
 * 改名建议的来源（优先级从高到低）：
 *   1. `_rename.json` 里已有的条目（人工指定 / 上次生成，**永远沿用** —— 保证建议稳定）
 *   2. 常用词词典（exact 中文 → `common.*`）
 *   3. 所在**对象属性名**（`optimization: "优化"` → `utils.categories.optimization`）
 *      / 变量名（`const title = "…"`）/ 调用名（`message.success("…")` → `…-success`）
 *   4. 都没有 → 保持编号不动
 *
 * 安全校验（apply 阶段）：新 key 非空、只含 `[a-z0-9-._]`、不同**原文**不许撞同一个新 key、
 * 循环改名（a→b 且 b→a）会拒绝。代码里的替换是**整 token**（`$t("old"` 带定界符），
 * 避免 `settings.1` 误伤 `settings.12`。
 */
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { parse as parseSfc } from "@vue/compiler-sfc";
import { pinyin } from "pinyin-pro";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src");
const LOCALES = join(SRC, "locales");
const HAN = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;
const T_NAMES = new Set(["t", "$t"]);
const VALID_KEY = /^[a-z0-9][a-z0-9-._]*$/;

const readJson = (p) => {
  try {
    return JSON.parse(readFileSync(p, "utf8"));
  } catch {
    return {};
  }
};

/** 文件 → key 命名空间（与 extract-i18n.mjs 保持一致） */
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
  if (parts[0] === "views" || parts[0] === "components") return kebab(parts[parts.length - 1]);
  return parts.map(kebab).join(".");
}

/** 常用词词典：exact 原文 → `common.*`。只放无歧义、高频的短词。 */
const DICT = {
  取消: "cancel",
  确定: "ok",
  确认: "confirm",
  保存: "save",
  删除: "delete",
  关闭: "close",
  打开: "open",
  复制: "copy",
  导出: "export",
  清空: "clear",
  搜索: "search",
  添加: "add",
  编辑: "edit",
  启用: "enable",
  禁用: "disable",
  刷新: "refresh",
  重试: "retry",
  完成: "done",
  停止: "stop",
  安装: "install",
  卸载: "uninstall",
  更新: "update",
  管理: "manage",
  全部: "all",
  启动: "launch",
  登录: "login",
  备份: "backup",
  恢复: "restore",
  重命名: "rename",
  新建: "new",
  名称: "name",
  版本: "version",
  大小: "size",
  状态: "status",
  路径: "path",
  账号: "account",
  密码: "password",
  用户名: "username",
  实例: "instance",
  游戏: "game",
  皮肤: "skin",
  语言: "language",
  主题: "theme",
  日志: "logs",
  默认: "default",
  自动: "auto",
  手动: "manual",
  成功: "success",
  失败: "failed",
  警告: "warning",
  加载中: "loading",
  已启用: "enabled",
  已禁用: "disabled",
};

/** 字面量 → `{文本, 参数}`（与 extract / migrate 同规则） */
function literalOf(node, sf, params = []) {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return { text: node.text, params };
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

/** 这些提示词没有区分度（`message.success($t(…))` 谁都是 success），宁可不要 */
const STOP_HINTS = new Set([
  "label", "title", "name", "text", "content", "value", "key", "item", "type", "id",
  "placeholder", "message", "success", "error", "warning", "info", "summary", "desc",
  "description", "hint", "tip", "default",
  // JS/Promise 语境噪音
  "then", "catch", "finally", "reject", "resolve", "msg", "res", "req", "data", "err",
]);

const slugify = (s) =>
  String(s)
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .replace(/[^a-zA-Z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();

/**
 * 中文原文 → 拼音 slug（截前 6 个音节，够辨识也不会长得离谱）。
 * 这是「词典和上下文提示都没命中」时的兜底 —— 保证 100% 覆盖，
 * 拼音对中文使用者可读（`wei-ding-wei` ≈ 未能定位）。
 */
function pinyinSlug(text) {
  const clean = String(text).replace(/\{p\d+\}/g, " ");
  const syllables = pinyin(clean, { toneType: "none", type: "array" }).filter((x) => /^[a-z]+$/.test(x));
  return syllables.slice(0, 6).join("-");
}

/**
 * 给一个字面量找「上下文提示」候选（按优先级）：
 *   1. 同对象里**兄弟属性的字符串值**：`{ label: $t(…), value: "copy" }` → `copy`（最好，是真 id）
 *   2. 自己所在的属性名：`optimization: $t(…)` → `optimization`
 *   3. 变量名：`const title = $t(…)` → `title`
 * `$t` 包裹层向上穿越；`message.success(…)` 这类泛词调用直接不要。
 */
function hintsOf(node) {
  const out = [];
  let cur = node;
  for (let depth = 0; cur && depth < 8; depth++) {
    const parent = cur.parent;
    if (!parent) break;
    if (ts.isCallExpression(parent) && parent.arguments.includes(cur)) {
      const callee = parent.expression;
      const name = ts.isIdentifier(callee) ? callee.text : ts.isPropertyAccessExpression(callee) ? callee.name.text : "";
      if (!T_NAMES.has(name)) {
        const h = slugify(name);
        if (h && !STOP_HINTS.has(h)) out.push(h);
      }
      cur = parent; // $t 包裹：继续往上找真正的语境
      continue;
    }
    if (ts.isPropertyAssignment(parent) && parent.initializer === cur) {
      const nameOf = (n) => (ts.isIdentifier(n) ? n.text : ts.isStringLiteral(n) ? n.text : "");
      const obj = parent.parent;
      // 兄弟属性的字符串值优先（它们通常是现成的英文 id）
      if (obj && ts.isObjectLiteralExpression(obj)) {
        for (const prop of obj.properties) {
          if (prop !== parent && ts.isPropertyAssignment(prop) && ts.isStringLiteral(prop.initializer)) {
            const h = slugify(prop.initializer.text);
            if (h && !STOP_HINTS.has(h)) out.push(h);
          }
        }
      }
      const own = slugify(nameOf(parent.name));
      if (own && !STOP_HINTS.has(own)) out.push(own);
      return out;
    }
    if (ts.isVariableDeclaration(parent) && parent.initializer === cur && ts.isIdentifier(parent.name)) {
      const h = slugify(parent.name.text);
      if (h && !STOP_HINTS.has(h)) out.push(h);
      return out;
    }
    cur = parent;
  }
  return out;
}

// ═══════════════════════════════════════════════════════════════════════════
// 扫描：key → 提示（每个文件里这个 key 的「最佳提示」）
// ═══════════════════════════════════════════════════════════════════════════

const ledger = readJson(join(LOCALES, "_index.json"));
const zh = readJson(join(LOCALES, "zh-CN.json"));
const en = readJson(join(LOCALES, "en-US.json"));
const zhByText = new Map(Object.entries(zh).map(([k, v]) => [v, k]));

/** key → { hints: Set, files: Set } */
const hintsByKey = new Map();
const usedKeys = new Set();

function addHint(key, hint, file) {
  if (!key) return;
  usedKeys.add(key);
  if (!hint) return;
  const hit = hintsByKey.get(key) ?? { hints: [], files: new Set() };
  hit.hints.push(hint); // 允许重复：出现次数就是优先级
  hit.files.add(file);
  hintsByKey.set(key, hit);
}

function analyzeJs(src, base, rel) {
  if (!src || !src.includes("$t")) return;
  const sf = ts.createSourceFile("s.tsx", src, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const visit = (node) => {
    if (ts.isCallExpression(node)) {
      const callee = node.expression;
      const name = ts.isIdentifier(callee) ? callee.text : ts.isPropertyAccessExpression(callee) ? callee.name.text : "";
      if (T_NAMES.has(name) && node.arguments.length) {
        const arg = node.arguments[0];
        const lit = literalOf(arg, sf);
        // 迁移后的代码里，$t() 的字面量**就是 key**（不是原文）
        if (lit && lit.text in zh) {
          for (const h of hintsOf(arg)) addHint(lit.text, h, rel);
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
}

function walkTemplate(node, src, rel) {
  if (!node || typeof node !== "object") return;
  if (node.type === 3) return;
  if (node.type === 1) {
    for (const p of node.props ?? []) {
      if (p.type === 7 && p.exp && HAN.test(p.exp.content)) {
        analyzeJs(p.exp.content, p.exp.loc?.start?.offset ?? 0, rel);
      }
    }
    for (const kid of node.children ?? []) {
      if (kid.type === 5) analyzeJs(kid.content.content, 0, rel);
      else walkTemplate(kid, src, rel);
    }
    return;
  }
  if (node.children) for (const kid of node.children) walkTemplate(kid, src, rel);
}

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (["node_modules", "dist", "locales"].includes(name)) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(vue|ts)$/.test(name) && !name.endsWith(".d.ts") && name !== "i18n.ts") out.push(p);
  }
  return out;
}

for (const file of walk(SRC)) {
  const rel = relative(ROOT, file).replace(/\\/g, "/");
  const src = readFileSync(file, "utf8");
  const ns = namespaceOf(rel);
  if (file.endsWith(".ts")) {
    analyzeJs(src, 0, rel);
    continue;
  }
  const { descriptor } = parseSfc(src);
  for (const blk of [descriptor.script, descriptor.scriptSetup]) {
    if (blk?.content) analyzeJs(blk.content, 0, rel);
  }
  if (descriptor.template?.ast) walkTemplate(descriptor.template.ast, src, rel);
  void ns;
}

// ═══════════════════════════════════════════════════════════════════════════
// 生成 / 应用
// ═══════════════════════════════════════════════════════════════════════════

const mapPath = join(LOCALES, "_rename.json");
const args = process.argv.slice(2);
// 注意 `--apply` 与 `--apply-list` 是两个不同的参数名，`includes("--apply")` 匹配不到后者
const mode =
  args.includes("--apply") || args.includes("--apply-list") ? "apply" : args.includes("--suggest") ? "suggest" : "";

if (!mode) {
  console.log("用法：node scripts/rename-i18n.mjs --suggest | --apply src/locales/_rename.json");
  process.exit(1);
}

const noPinyin = args.includes("--no-pinyin");

if (mode === "suggest") {
  // 已有建议沿用（稳定），只补新的
  const map = readJson(mapPath);
  const keyByText = new Map(Object.entries(zh).map(([k, v]) => [v, k]));
  const taken = new Set([...Object.keys(zh), ...Object.keys(ledger).map((t) => ledger[t]?.key).filter(Boolean)]);
  let auto = 0;
  for (const [key, info] of hintsByKey) {
    if (map[key]) {
      taken.add(map[key]);
      continue;
    }
    const ns = key.split(".").slice(0, -1).join(".");
    const text = zh[key] ?? "";
    const cands = [];
    // 词典优先（exact 原文 → common.*）
    if (DICT[text]) cands.push(`common.${DICT[text]}`);
    // 上下文提示按出现次数排序
    const counts = new Map();
    for (const h of info.hints) counts.set(h, (counts.get(h) ?? 0) + 1);
    for (const [h] of [...counts].sort((a, b) => b[1] - a[1])) cands.push(`${ns}.${h}`);
    // 拼音兜底（可 --no-pinyin 关掉，改由人工提供英文名）
    if (!noPinyin) {
      const py = pinyinSlug(text);
      if (py) cands.push(`${ns}.${py}`);
    }

    let chosen = "";
    for (const c of cands) {
      if (!VALID_KEY.test(c)) continue;
      if (/\d+$/.test(c.split(".").pop()) || /^p\d+$/.test(c.split(".").pop())) continue; // p1/p2 这类不是名字
      if (!taken.has(c)) {
        chosen = c;
        break;
      }
      // 已被占用但原文相同 → 合并复用同一个 key（同文案本就该共用译文）
      if (zh[c] === text || keyByText.get(text) === c) {
        chosen = c;
        break;
      }
    }
    if (!chosen) continue;
    map[key] = chosen;
    taken.add(chosen);
    if (!/\.\d+$/.test(chosen)) auto++;
  }
  writeFileSync(mapPath, JSON.stringify(map, null, 2) + "\n");
  const named = Object.values(map).filter((v) => !/\.\d+$/.test(v)).length;
  console.log(`\n建议已写入 src/locales/_rename.json：${Object.keys(map).length} 条（其中语义名 ${named} 条）`);
  console.log("人工审阅后：node scripts/rename-i18n.mjs --apply src/locales/_rename.json\n");
  process.exit(0);
}

// ── apply ────────────────────────────────────────────────────────────────

const errors = [];
const map = {};
const listIdx = args.indexOf("--apply-list");
if (listIdx !== -1) {
  // 手写名单：每行 `旧key 英文名`（名字不带命名空间，自动挂在旧 key 的命名空间下）。
  // 这是我（AI）批量人工命名用的通道 —— 一次翻一批，分多个文件传。
  for (const f of args.slice(listIdx + 1).filter((a) => existsSync(a))) {
    for (const raw of readFileSync(f, "utf8").split("\n")) {
      const line = raw.trim();
      if (!line || line.startsWith("#")) continue;
      const m = /^(\S+)[ \t]+([a-z0-9][a-z0-9-]*)$/.exec(line);
      if (!m) {
        errors.push(`无法解析的行（应为 "旧key 新名字"）：${line.slice(0, 70)}`);
        continue;
      }
      const [, oldKey, name] = m;
      if (!(oldKey in zh)) {
        errors.push(`旧 key 不存在：${oldKey}`);
        continue;
      }
      const ns = oldKey.split(".").slice(0, -1).join(".");
      map[oldKey] = `${ns}.${name}`;
    }
  }
} else {
  Object.assign(map, readJson(args[args.indexOf("--apply") + 1] || ""));
}
if (!Object.keys(map).length) {
  console.log("改名映射为空。用法：--apply <_rename.json> | --apply-list <名单.txt>…");
  process.exit(1);
}

// 校验：新 key 合法、不同原文不撞同一新 key、旧 key 必须存在
const byNew = new Map();
const rename = {};
for (const [oldKey, newKey] of Object.entries(map)) {
  if (!(oldKey in zh)) {
    errors.push(`旧 key 不存在：${oldKey}`);
    continue;
  }
  if (!VALID_KEY.test(newKey)) {
    errors.push(`新 key 不合法（只允许 [a-z0-9-._]）：${newKey}`);
    continue;
  }
  const prev = byNew.get(newKey);
  if (prev && zh[prev] !== zh[oldKey]) {
    errors.push(`新 key 撞车且文案不同：${newKey} ← ${prev} / ${oldKey}`);
    continue;
  }
  if (map[newKey] && map[newKey] !== newKey) {
    errors.push(`改名链：${oldKey} → ${newKey}，但 ${newKey} 自己也要改成 ${map[newKey]}`);
    continue;
  }
  byNew.set(newKey, prev ?? oldKey);
  rename[oldKey] = newKey;
}
if (errors.length) {
  // 一条撞车不应该拖死整批：跳过它们（保留原编号），其余照常应用
  console.log(`\n跳过 ${errors.length} 条（保持原编号）：`);
  for (const e of errors.slice(0, 20)) console.log(`  · ${e}`);
}

// 1) 代码：整 token 替换 `$t("old"` → `$t("new"` / `$t('old'` → `$t('new'`
let files = 0;
let sites = 0;
for (const file of walk(SRC)) {
  let src = readFileSync(file, "utf8");
  let changed = 0;
  for (const [oldKey, newKey] of Object.entries(rename)) {
    for (const q of ['"', "'"]) {
      const re = new RegExp(`(\\$t\\(\\s*)${q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}${oldKey.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}${q}`, "g");
      src = src.replace(re, (_m, head) => {
        changed++;
        return `${head}${q}${newKey}${q}`;
      });
    }
  }
  if (changed) {
    writeFileSync(file, src);
    files++;
    sites += changed;
  }
}

// 2) 词表 + 台账：换 key，值原样
const zhNew = {};
for (const [k, v] of Object.entries(zh)) zhNew[rename[k] ?? k] = v;
writeFileSync(join(LOCALES, "zh-CN.json"), JSON.stringify(zhNew, null, 2) + "\n");
const enNew = {};
for (const [k, v] of Object.entries(en)) enNew[rename[k] ?? k] = v;
writeFileSync(join(LOCALES, "en-US.json"), JSON.stringify(enNew, null, 2) + "\n");
const idxNew = {};
for (const [text, info] of Object.entries(ledger)) idxNew[text] = { ...info, key: rename[info.key] ?? info.key };
writeFileSync(join(LOCALES, "_index.json"), JSON.stringify(idxNew, null, 2) + "\n");

console.log(`\n改名 ${Object.keys(rename).length} 个 key：改了 ${files} 个文件 ${sites} 处调用，词表与台账已同步`);
console.log("下一步：npm run i18n:audit（必须全 0）\n");
