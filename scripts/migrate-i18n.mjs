#!/usr/bin/env node
/**
 * i18n 迁移 codemod：把源码里硬编码的中文换成 `$t("key")` 调用。
 *
 * 为什么叫 `$t` 而不是 `t`：`t` 在本项目里被大量当作变量名
 * （`tasks.filter((t) => …)`、`v-for="t in …"`、`t.source`）—— 用 `t` 会被作用域遮蔽，
 * 模板里渲染时直接炸。`$t` 全仓没人用过，且 vue-i18n 也是这个写法。
 *
 * 依据 `src/locales/_index.json` 台账（原文 → key）替换，**不认识的中文会报出来而不是瞎编 key**。
 *
 * 用法：
 *   node scripts/migrate-i18n.mjs --file src/views/HomeView.vue --dry    # 只看会怎么改
 *   node scripts/migrate-i18n.mjs --file src/views/HomeView.vue          # 改这一个文件
 *   node scripts/migrate-i18n.mjs --all                                  # 全部
 *
 * 会跳过：已在 `$t(...)` 里的、开发日志（`[fe] …`）、**参与逻辑判断的**
 * （`x === "中文"`、`msg.includes("中文")`、`s.startsWith("中文")` —— 这类包了 t() 翻译后
 * 会失配，属于数据不是文案）。
 */
import { readdirSync, readFileSync, statSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { parse as parseSfc } from "@vue/compiler-sfc";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src");
const LOCALES = join(SRC, "locales");
const HAN = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;
const DEBUG_LOG = /^\s*\[(fe|api|db|perf|dev|debug|warn)\]/i;
const T_NAMES = new Set(["t", "$t"]);
const COMPARE_OPS = new Set([
  ts.SyntaxKind.EqualsEqualsToken,
  ts.SyntaxKind.EqualsEqualsEqualsToken,
  ts.SyntaxKind.ExclamationEqualsToken,
  ts.SyntaxKind.ExclamationEqualsEqualsToken,
]);
const DATA_METHODS = new Set(["includes", "startsWith", "endsWith", "indexOf", "lastIndexOf", "match", "test"]);

const ledger = JSON.parse(readFileSync(join(LOCALES, "_index.json"), "utf8"));
const keyOf = (text) => ledger[text]?.key;
const norm = (s) => s.replace(/\s+/g, " ").trim();

const args = process.argv.slice(2);
const dry = args.includes("--dry");
const all = args.includes("--all");
const fileArg = args.includes("--file") ? args[args.indexOf("--file") + 1] : null;

const stats = { files: 0, changed: 0, items: 0, skipped: [], unknown: [] };

// ═══════════════════════════════════════════════════════════════════════════
// JS/TS：找「最外层」可替换的字面量
// ═══════════════════════════════════════════════════════════════════════════

/** 这个字面量是不是在 `$t(...)` 参数位 / 逻辑比较 / 开发日志里，不该替换 */
function skipReason(node, sf) {
  const parent = node.parent;
  if (parent && ts.isCallExpression(parent) && parent.arguments[0] === node) {
    const callee = parent.expression;
    const name = ts.isIdentifier(callee) ? callee.text : ts.isPropertyAccessExpression(callee) ? callee.name.text : "";
    if (T_NAMES.has(name)) return "already";
    if (DATA_METHODS.has(name)) return "data";
  }
  if (parent && ts.isBinaryExpression(parent) && COMPARE_OPS.has(parent.operatorToken.kind)) return "data";
  if (DEBUG_LOG.test(node.getText(sf))) return "log";
  return null;
}

/**
 * 字面量 → `{文本, 参数}`，**必须与提取器同规则**：
 * 普通串取 `node.text`（已去引号/反转义），模板串的 `${…}` 编号成 `{p1}` 并记下表达式。
 * 用 `node.getText()` 会带着引号和原始 `${}`，跟台账（`正在启动实例「{p1}」`）对不上 —— 踩过。
 */
function literalOf(node, sf, params = []) {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return { text: node.text, params };
  if (ts.isTemplateExpression(node)) {
    let text = node.head.text;
    for (const span of node.templateSpans) {
      params.push({ name: `p${params.length + 1}`, expr: span.expression.getText(sf).trim() });
      text += `{${params[params.length - 1].name}}` + span.literal.text;
    }
    return { text: norm(text), params };
  }
  return null;
}

/** 一段 JS/TS：收集可替换项（含模板串插值里的嵌套字面量由替换时递归处理） */
function collectJs(src, base, quote, items, report) {
  if (!src || !HAN.test(src)) return;
  const sf = ts.createSourceFile("s.tsx", src, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const visit = (node) => {
    const lit = literalOf(node, sf);
    if (lit) {
      if (HAN.test(lit.text)) {
        const reason = skipReason(node, sf);
        if (reason) report(reason, lit.text);
        else items.push({ kind: "js", quote, text: lit.text, params: lit.params, start: base + node.getStart(sf), end: base + node.getEnd() });
        return; // 命中就不再往里走：嵌套字面量在生成参数时递归处理
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
}

// ═══════════════════════════════════════════════════════════════════════════
// 模板
// ═══════════════════════════════════════════════════════════════════════════

function collectTemplate(node, src, items, report) {
  if (!node || typeof node !== "object") return;
  if (node.type === 3) return; // 注释
  if (node.type === 1) {
    for (const p of node.props ?? []) {
      if (p.type === 6 && p.value && HAN.test(p.value.content)) {
        items.push({ kind: "attr", attr: p.name, text: norm(p.value.content), start: p.loc.start.offset, end: p.loc.end.offset });
      } else if (p.type === 7 && p.exp && HAN.test(p.exp.content) && p.exp.loc) {
        // 绑定属性里的表达式：整段就是 HTML 属性值，字符串要用单引号。
        // 偏移不能用 exp.loc.start.offset（那是引号/属性起点），从节点范围内搜表达式实际位置。
        collectJs(p.exp.content, exprBase(src, p.exp), "'", items, report);
      }
    }
    // 「文本 + 插值」合并成一条
    const kids = node.children ?? [];
    let buf = "";
    let params = [];
    let start = -1;
    let end = -1;
    const flush = () => {
      const text = norm(buf);
      if (HAN.test(text)) items.push({ kind: "text", text, params, start, end });
      buf = "";
      params = [];
      start = -1;
      end = -1;
    };
    for (const kid of kids) {
      if (kid.type === 2) {
        if (start < 0) start = kid.loc.start.offset;
        buf += kid.content;
        end = kid.loc.end.offset;
      } else if (kid.type === 5) {
        if (start < 0) start = kid.loc.start.offset;
        const expr = kid.content.content;
        params.push({ name: `p${params.length + 1}`, expr: expr.trim() });
        buf += `{p${params.length}}`;
        end = kid.loc.end.offset;
        // 插值里的字面量单独也要换（它作为参数表达式被带出去）
        collectJs(expr, exprBase(src, kid.content), "'", items, report);
      } else {
        flush();
      }
    }
    flush();
    for (const kid of kids) if (kid.type !== 2 && kid.type !== 5) collectTemplate(kid, src, items, report);
    return;
  }
  if (node.children) for (const kid of node.children) collectTemplate(kid, src, items, report);
}

/**
 * 某段表达式（插值 / 指令）在文件里的真实起始偏移。
 * Vue AST 的 `loc.start.offset` 对插值指向 `{{`、对指令表达式指向属性起点，
 * 都不能直接用；从节点范围内搜内容本身最稳。
 */
function exprBase(src, expNode) {
  const content = expNode.content ?? expNode.ast?.content ?? "";
  const from = expNode.loc?.start?.offset ?? 0;
  const at = src.indexOf(content, from);
  return at >= 0 ? at : from;
}

// ═══════════════════════════════════════════════════════════════════════════
// 生成替换文本
// ═══════════════════════════════════════════════════════════════════════════

/**
 * 递归改写参数表达式里嵌套的中文（`${cond ? "甲" : "乙"}` 这种）。
 * 只取**最外层**可替换字面量：嵌套的那些在生成参数时由本函数递归处理，
 * 否则内外两层都用原偏移替换会互相错位。
 */
function rewriteExpr(expr, quote) {
  let out = expr;
  const sf = ts.createSourceFile("e.tsx", expr, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const inner = [];
  const visit = (node) => {
    const lit = literalOf(node, sf);
    if (lit) {
      if (HAN.test(lit.text)) {
        if (!skipReason(node, sf)) inner.push({ start: node.getStart(sf), end: node.getEnd(), text: lit.text, params: lit.params });
        return; // 命中就到此为止，嵌套的在 buildCall 里递归
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  inner.sort((a, b) => b.start - a.start);
  for (const it of inner) {
    const repl = buildCall(it.text, it.params ?? [], quote);
    if (repl) out = out.slice(0, it.start) + repl + out.slice(it.end);
  }
  return out;
}

/** key 不存在时返回 null（会报出来，不瞎编） */
function buildCall(text, params, quote) {
  const key = keyOf(text);
  if (!key) {
    stats.unknown.push(`${text.slice(0, 60)}`);
    return null;
  }
  const q = quote;
  const arg = params.length ? `, { ${params.map((p) => `${p.name}: ${rewriteExpr(p.expr, quote)}`).join(", ")} }` : "";
  return `$t(${q}${key}${q}${arg})`;
}

function replacementOf(item) {
  if (item.kind === "text") {
    const call = buildCall(item.text, item.params ?? [], '"');
    return call ? `{{ ${call} }}` : null;
  }
  if (item.kind === "attr") {
    const call = buildCall(item.text, [], "'");
    return call ? `:${item.attr}="${call}"` : null;
  }
  const call = buildCall(item.text, item.params ?? [], item.quote);
  return call;
}

// ═══════════════════════════════════════════════════════════════════════════
// 处理单个文件
// ═══════════════════════════════════════════════════════════════════════════

function importPathFor(file) {
  const rel = relative(dirname(file), join(SRC, "i18n")).replace(/\\/g, "/");
  return rel.startsWith(".") ? rel : `./${rel}`;
}

/**
 * 生成「插入 import」这条改动（**偏移基于原始源码**）。
 *
 * 必须在替换之前算好坐标：第一版是替换完再拿原内容去做 `indexOf` 定位 ——
 * 内容已被改掉自然找不到，于是走了兜底分支，把一整个 `<script setup>` 插进了文件中间（踩过）。
 */
function importEdit(src, file, sfc) {
  const line = `import { t as $t } from "${importPathFor(file)}";`;
  if (src.includes(line) || /import\s*\{[^}]*\bt\s+as\s+\$t\b/.test(src)) return null;

  if (file.endsWith(".ts")) {
    let pos = 0;
    if (src.startsWith("/*")) {
      const end = src.indexOf("*/");
      pos = end === -1 ? 0 : end + 2;
    }
    const m = /^(\s*(?:\/\/[^\n]*\n)*\s*)/.exec(src.slice(pos));
    const at = pos + (m ? m[0].length : 0);
    return { start: at, end: at, text: line + "\n" };
  }

  const block = sfc?.descriptor?.scriptSetup ?? sfc?.descriptor?.script;
  if (block?.content) {
    const contentStart = src.indexOf(block.content);
    if (contentStart >= 0) {
      const head = /^(\s*(?:\/\*\*[\s\S]*?\*\/|\/\/[^\n]*\n)*\s*)/.exec(block.content);
      const at = contentStart + (head ? head[0].length : 0);
      return { start: at, end: at, text: line + "\n" };
    }
  }
  // 没有 script 块（纯模板组件）：补一个
  const tpl = sfc?.descriptor?.template;
  if (tpl?.loc?.start?.offset != null) {
    const at = tpl.loc.start.offset;
    return { start: at, end: at, text: `<script setup lang="ts">\n${line}\n</script>\n\n` };
  }
  return null;
}

function migrate(file) {
  const rel = relative(ROOT, file).replace(/\\/g, "/");
  let src = readFileSync(file, "utf8");
  if (!HAN.test(src)) return null;

  const items = [];
  const reportSkip = (reason, text) => stats.skipped.push(`${rel}  [${reason}] ${text.slice(0, 40)}`);
  let sfc = null;

  if (file.endsWith(".vue")) {
    sfc = parseSfc(src);
    for (const blk of [sfc.descriptor.script, sfc.descriptor.scriptSetup]) {
      if (blk?.content) {
        const base = src.indexOf(blk.content);
        if (base >= 0) collectJs(blk.content, base, '"', items, reportSkip);
      }
    }
    if (sfc.descriptor.template?.ast) collectTemplate(sfc.descriptor.template.ast, src, items, reportSkip);
  } else {
    collectJs(src, 0, '"', items, reportSkip);
  }

  if (!items.length) return null;

  // 去掉被外层覆盖的嵌套项（嵌套的靠 rewriteExpr 递归处理）
  items.sort((a, b) => a.start - b.start);
  const kept = [];
  let lastEnd = -1;
  for (const it of items) {
    if (it.start < lastEnd) continue;
    kept.push(it);
    lastEnd = it.end;
  }

  // 所有改动（含 import）的偏移都基于**原始源码**，最后一次性按序施加
  const edits = [];
  let count = 0;
  for (const it of kept) {
    const repl = replacementOf(it);
    if (!repl) continue;
    edits.push({ start: it.start, end: it.end, text: repl });
    count++;
  }
  if (!count) return null;
  const imp = importEdit(src, file, sfc);
  if (imp) edits.push(imp);

  edits.sort((a, b) => a.start - b.start || b.end - a.end);
  let out = "";
  let pos = 0;
  for (const e of edits) {
    if (e.start < pos) continue; // 兜底：正常不会重叠
    out += src.slice(pos, e.start) + e.text;
    pos = e.end;
  }
  out += src.slice(pos);

  if (!dry) writeFileSync(file, out);
  stats.files++;
  stats.changed++;
  stats.items += count;
  return { rel, count, addedImport: !!imp };
}

// ═══════════════════════════════════════════════════════════════════════════

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (["node_modules", "dist", "locales"].includes(name)) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(vue|ts)$/.test(name) && !name.endsWith(".d.ts") && name !== "i18n.ts") out.push(p);
  }
  return out;
}

const targets = fileArg ? [join(ROOT, fileArg)] : all ? walk(SRC) : [];
if (!targets.length) {
  console.log("用法：node scripts/migrate-i18n.mjs --file <相对路径> [--dry] | --all [--dry]");
  process.exit(1);
}
const results = targets.map((f) => (existsSync(f) ? migrate(f) : null)).filter(Boolean);
console.log(`\n${dry ? "[dry-run] " : ""}处理 ${targets.length} 个文件，改动 ${results.length} 个，替换 ${stats.items} 处`);
for (const r of results.slice(0, 40)) console.log(`  ${String(r.count).padStart(4)} 处${r.addedImport ? " +import" : ""}  ${r.rel}`);
if (stats.unknown.length) {
  console.log(`\n⚠ 台账里没有 key 的文案 ${stats.unknown.length} 条（没动它们）：`);
  for (const t of [...new Set(stats.unknown)].slice(0, 15)) console.log(`  · ${t}`);
}
const byReason = {};
for (const s of stats.skipped) {
  const m = /\[(\w+)\]/.exec(s);
  byReason[m?.[1] ?? "?"] = (byReason[m?.[1] ?? "?"] ?? 0) + 1;
}
console.log(`\n跳过：${JSON.stringify(byReason)}（already=已迁过，log=开发日志，data=逻辑判断/数据）`);
console.log("");
