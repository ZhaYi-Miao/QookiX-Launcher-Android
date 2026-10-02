#!/usr/bin/env node
/**
 * i18n 校验器（**独立实现**，故意不复用 extract-i18n.mjs 的 AST 逻辑）：
 * 用最笨的行级正则再扫一遍源码，做两类事 ——
 *
 * 【硬断言】key 完整性，必须全 0：
 *   1. 代码里 `t("key")` 用到、但 zh-CN.json 里没有的 key
 *   2. 还在用旧写法 `t("中文原文")`
 *   3. zh-CN.json 里有、代码里没人用的 key
 *   4. en-US.json 有、zh-CN.json 没有的 key
 *   5. zh-CN / en-US 的空值
 *   6. 同一个 key 在 zh-CN 与 en-US 里的占位符不一致；或调用点没把占位符传全
 *
 * 【可疑清单】可能漏提取的中文（行级启发式，会有假阳性，人工扫一眼即可）：
 *   某行出现的「连续汉字串」在待迁移清单与词表里都找不到。
 *
 * 用法：`npm run i18n:audit`
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src");
const LOCALES = join(SRC, "locales");
const HAN = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;
const HAN_RUN = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]{2,}/g;
const DEBUG_LOG = /\[(fe|api|db|perf|dev|debug|warn)\]/;

const readJson = (p) => {
  try {
    return JSON.parse(readFileSync(p, "utf8"));
  } catch {
    return {};
  }
};

const zh = readJson(join(LOCALES, "zh-CN.json"));
const en = readJson(join(LOCALES, "en-US.json"));
const inventory = readJson(join(LOCALES, "_index.json"));

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (["node_modules", "dist", "locales"].includes(name)) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(vue|ts)$/.test(name) && !name.endsWith(".d.ts")) out.push(p);
  }
  return out;
}

const placeholders = (s) => new Set([...String(s).matchAll(/\{(\w+)\}/g)].map((m) => m[1]));
/** 调用点传了哪些具名参数：`{ count: n }` / `{ count }` */
const argNames = (s) => new Set([...String(s).matchAll(/([\w$]+)\s*[:,}]/g)].map((m) => m[1]));

/**
 * 扫一行里所有 `$t("key")` / `$t("key", { … })`。
 *
 * 必须**平衡括号 + 跳过字符串**：代码里常有嵌套调用
 * `$t("a", { p2: c ? $t("b") : "" })`，用 `[^)]*` 取参数会在内层 `)` 处截断，
 * 既漏掉内层的 `$t("b")`（误报「没人用」），又把外层参数截断（误报「少传占位符」）。
 * 扫完 key 后不跳过参数区，继续往里扫，才能发现嵌套调用。
 */
function scanCalls(line, onCall) {
  const re = /(?<![\w$.])(?:\$t|t)\(/g;
  let m;
  while ((m = re.exec(line))) {
    let i = m.index + m[0].length;
    while (i < line.length && /\s/.test(line[i])) i++;
    const q = line[i];
    if (q !== '"' && q !== "'" && q !== "`") continue;
    let key = "";
    let j = i + 1;
    while (j < line.length && line[j] !== q) {
      if (line[j] === "\\") { key += line[j + 1] ?? ""; j += 2; continue; }
      key += line[j];
      j++;
    }
    // 第二个参数（平衡括号读出来，供占位符核对）
    let k = j + 1;
    while (k < line.length && /\s/.test(line[k])) k++;
    let args = "";
    if (line[k] === ",") {
      const start = k + 1;
      let depth = 1;
      let p = start;
      while (p < line.length && depth > 0) {
        const c = line[p];
        if (c === '"' || c === "'" || c === "`") {
          const qq = c;
          p++;
          while (p < line.length && line[p] !== qq) {
            if (line[p] === "\\") p++;
            p++;
          }
        } else if (c === "(" || c === "{" || c === "[") depth++;
        else if (c === ")" || c === "}" || c === "]") {
          depth--;
          if (!depth) break;
        }
        p++;
      }
      args = line.slice(start, p);
    }
    onCall(key, args);
    re.lastIndex = j + 1; // 继续往参数区里扫，抓嵌套的 $t(...)
  }
}
/**
 * 去掉行内注释与块注释（粗粒度启发式，仅供「可疑清单」用）。
 * 注意用 `[^\n]*` 而不是 `.*$`：文件是 CRLF，`$` 在 `\r` 前不匹配，
 * 会让行尾中文注释整条漏掉（踩过）。
 * 含 `://` 的行只砍「空格 + //」之后的，免得把 URL 切了。
 */
const stripComments = (line) => {
  const s = line.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/<!--[\s\S]*?-->/g, " ");
  return /:\/\//.test(s) ? s.replace(/\s\/\/[^\n]*/, " ") : s.replace(/\s*\/\/[^\n]*/, " ");
};
const isCommentLine = (line) => /^\s*(\/\/|\*|\/\*|<!--)/.test(line);

// ── 扫源码：t() 调用 + 可疑中文 ─────────────────────────────────────────
const used = new Set();
const legacy = [];
const paramMismatch = [];
const suspicious = [];
const known = new Set([...Object.keys(inventory), ...Object.values(zh), ...Object.values(en)]);
const hasKnown = (run) => {
  for (const k of known) if (k.includes(run)) return true;
  return false;
};

const files = walk(SRC);
for (const file of files) {
  const rel = relative(ROOT, file).replace(/\\/g, "/");
  const lines = readFileSync(file, "utf8").split("\n");
  let inStyle = false;
  let inBlock = false;
  let inHtmlComment = false;
  lines.forEach((line, i) => {
    // t("key") / $t("key")。`(?<![\w$.])` 是必须的：否则 `emit(`、`getContext(`、
    // `import(` 这些「以 t( 结尾」的调用会被误当成取译文（第一版就踩了）。
    if (!isCommentLine(line)) {
      scanCalls(line, (key, args) => {
        used.add(key);
        if (HAN.test(key)) legacy.push(`${rel}:${i + 1}  t("${key}")`);
        if (args && zh[key]) {
          const got = argNames(args);
          for (const n of placeholders(zh[key])) {
            if (!got.has(n)) paramMismatch.push(`${rel}:${i + 1}  t("${key}") 少传 {${n}}`);
          }
        }
      });
    }
    // 可疑中文（跳过 style 块、注释、开发日志行）
    if (/<style/i.test(line)) inStyle = true;
    if (inStyle) {
      if (/<\/style>/i.test(line)) inStyle = false;
      return;
    }
    // 跨行 HTML 注释（`<!-- … -->`，模板里很常见，续行不含 `>`）
    if (inHtmlComment) {
      if (line.includes("-->")) inHtmlComment = false;
      return;
    }
    if (line.includes("<!--") && !line.includes("-->")) {
      inHtmlComment = true;
      return;
    }
    // 跨行块注释：`/* …` 开头的注释块，续行往往不以 `*` 开头，必须用状态跟踪
    if (inBlock) {
      if (line.includes("*/")) inBlock = false;
      return;
    }
    if (/\/\*/.test(line) && !/\*\//.test(line)) {
      inBlock = true;
      return;
    }
    if (isCommentLine(line)) return;
    const code = stripComments(line);
    if (!HAN.test(code) || DEBUG_LOG.test(code)) return;
    for (const run of code.match(HAN_RUN) ?? []) {
      if (!hasKnown(run)) suspicious.push(`${rel}:${i + 1}  ${run}`);
    }
  });
}

// ── 硬断言 ──────────────────────────────────────────────────────────────
const missing = [...used].filter((k) => !(k in zh));
// 已建 key 但代码还没改过去的（原文仍在待迁移清单里）属于**待办**，不是「没人用」。
// 注意 `_index.json` 的结构是「原文 → 描述对象」，所以要取 keys 而不是 values。
const pendingValues = new Set(Object.keys(inventory));
const unused = Object.keys(zh).filter((k) => !used.has(k) && !pendingValues.has(zh[k]));
const orphans = Object.keys(en).filter((k) => !(k in zh));
const emptyVals = [
  ...Object.entries(zh).filter(([, v]) => !String(v).trim()).map(([k]) => `zh-CN ${k}`),
  ...Object.entries(en).filter(([, v]) => !String(v).trim()).map(([k]) => `en-US ${k}`),
];
const phMismatch = Object.keys(zh)
  .filter((k) => k in en)
  .filter((k) => {
    const a = placeholders(zh[k]);
    const b = placeholders(en[k]);
    return a.size !== b.size || [...a].some((n) => !b.has(n));
  })
  .map((k) => `${k}  中=${[...placeholders(zh[k])].join(",")} 英=${[...placeholders(en[k])].join(",")}`);

const uniq = [...new Set(suspicious)];
const line = (title, rows, limit = 12) => {
  console.log(`\n${title}  ${rows.length} 条`);
  for (const r of rows.slice(0, limit)) console.log(`  · ${r}`);
};

console.log(`\n扫描 ${files.length} 个文件；代码里用了 ${used.size} 个 key`);
console.log(`词表：zh-CN ${Object.keys(zh).length} 条 / en-US ${Object.keys(en).length} 条`);
console.log(`待迁移清单：${Object.keys(inventory).length} 条`);
console.log("\n── 硬断言（都应为 0）──");
line("【1】代码在用、词表没有的 key", missing);
line("【2】旧写法 t(\"中文原文\")", legacy);
line("【3】词表有、代码没人用的 key", unused);
line("【4】en-US 有、zh-CN 没有的 key", orphans);
line("【5】空值", emptyVals);
line("【6】占位符不一致 / 调用点少传", [...phMismatch, ...paramMismatch]);
line("【7】可能漏提取的中文（启发式，含假阳性）", uniq, 20);
console.log("");
