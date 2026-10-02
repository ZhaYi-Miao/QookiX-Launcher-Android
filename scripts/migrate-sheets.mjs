// 一次性 codemod：把 `n-modal preset="card"` 批量换成统一弹层壳 AppSheet。
// 用法：node scripts/migrate-sheets.mjs [--dry]
//
// 换壳规则（与 AppSheet.vue 的 props 对齐）：
//   · 丢掉 n-modal 专属属性：:auto-focus / preset / :bordered / style(width/max-width)
//   · 保留并直传：:show / v-model:show / :title / :mask-closable / class
//   · 关闭标签 </n-modal> → </app-sheet>
//   · 清理不再使用的 NModal import，并补 AppSheet import（按目录深度算相对路径）
import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, dirname, relative } from "node:path";

const DRY = process.argv.includes("--dry");
const ROOT = join(process.cwd(), "src");

/** 收集 src 下所有 .vue（跳过 ui/ 目录本身） */
function vueFiles(dir, acc = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (name === "ui") continue;
      vueFiles(p, acc);
    } else if (name.endsWith(".vue")) {
      acc.push(p);
    }
  }
  return acc;
}

/**
 * 找 `<n-modal ...>` 的完整开标签。
 *
 * 两个踩过的坑（都因为「以为标签不会骗人」）：
 * 1. 非贪婪正则 `<n-modal\b[\s\S]*?>` 会在属性值里的 `>` 处提前截断 ——
 *    `@update:show="(v) => …"` 直接被切成两半。
 * 2. 缩进要从**所在行**取（`m` 是从 `<` 开始匹配的，`^\s*` 永远拿到空串）。
 * 所以这里手写扫描：引号内的一切（含 `>`）都跳过。
 */
function findOpenTag(src, from) {
  const start = src.indexOf("<n-modal", from);
  if (start === -1) return null;
  const lineStart = src.lastIndexOf("\n", start) + 1;
  const indent = src.slice(lineStart, start);
  let i = start + "<n-modal".length;
  let quote = null;
  while (i < src.length) {
    const c = src[i];
    if (quote) {
      if (c === quote) quote = null;
    } else if (c === '"' || c === "'") {
      quote = c;
    } else if (c === ">") {
      return { start, end: i + 1, attrs: src.slice(start + "<n-modal".length, i), indent };
    }
    i++;
  }
  return null;
}

/**
 * 按「引号外的空白」切属性。
 *
 * 踩过的坑：早先用 `raw.split(/\s+(?=[:@\w])/)` 直接按空白切，把属性**值里**的空格
 * 也切开了 —— `:show="dialog !== null"` 被切成 `:show="dialog` / `!==` / `null"`，
 * 模板直接废掉。属性值里出现空格是常态（表达式、style 声明），所以必须跟踪引号。
 */
function splitAttrs(raw) {
  const out = [];
  let cur = "";
  let quote = null;
  for (let i = 0; i < raw.length; i++) {
    const c = raw[i];
    if (quote) {
      cur += c;
      if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      cur += c;
      continue;
    }
    if (/\s/.test(c)) {
      let j = i;
      while (j < raw.length && /\s/.test(raw[j])) j++;
      if (j >= raw.length) break;
      if (/[:@\w-]/.test(raw[j])) {
        out.push(cur);
        cur = "";
        i = j - 1;
        continue;
      }
      // 引号外的空格且下一段不是属性名开头：仍是当前属性的一部分
    }
    cur += c;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

function convertAttrs(raw) {
  const drop = new Set([
    "preset", ":auto-focus", "auto-focus", ":bordered", "bordered",
    ":close-on-esc", "close-on-esc",
  ]);
  const kept = [];
  for (const attr of splitAttrs(raw)) {
    const t = attr.replace(/\s+/g, " ").trim();
    if (!t) continue;
    const name = t.split("=")[0];
    if (drop.has(name)) continue;
    // 固定宽度样式由 AppSheet 的自适应高度接管（保留 height 相关的样式）
    if ((name === "style" || name === ":style") && !/height/.test(t) && /width/.test(t)) continue;
    kept.push(t);
  }
  return kept;
}

let touched = 0;
for (const file of vueFiles(ROOT)) {
  const src = readFileSync(file, "utf8");
  // 闭标签可能已手工换过、只留下 `</n-modal>`（开标签已改的情况），所以两处都要查
  if (!src.includes("<n-modal") && !src.includes("</n-modal>")) continue;

  let count = 0;
  let out = "";
  let cursor = 0;
  for (;;) {
    const tag = findOpenTag(src, cursor);
    if (!tag) {
      out += src.slice(cursor);
      break;
    }
    const kept = convertAttrs(tag.attrs);
    count++;
    out += src.slice(cursor, tag.start);
    if (kept.length === 0) out += "<app-sheet>";
    else if (kept.length === 1) out += `<app-sheet ${kept[0]}>`;
    else out += `<app-sheet\n${kept.map((a) => `${tag.indent}  ${a}`).join("\n")}\n${tag.indent}>`;
    cursor = tag.end;
  }
  out = out.replace(/<\/n-modal>/g, "</app-sheet>");

  // import 清理（NModal 不再使用就摘掉）+ 补 AppSheet
  const bodyOut = out.replace(/import[^;]*;/g, "");
  const usesNModal = /<n-modal|\bNModal\b/.test(bodyOut);
  out = out.replace(/import\s*\{([^}]*)\}\s*from\s*"naive-ui";/, (_m, names) => {
    const kept = names
      .split(",")
      .map((s) => s.trim())
      .filter((n) => n && (usesNModal || n !== "NModal"));
    return `import { ${kept.join(", ")} } from "naive-ui";`;
  });
  if (!/AppSheet/.test(out.split("</script>")[0])) {
    let relPath = relative(dirname(file), join(ROOT, "ui", "AppSheet.vue")).replace(/\\/g, "/");
    if (!relPath.startsWith(".")) relPath = "./" + relPath;
    const importLine = `\nimport AppSheet from "${relPath}";`;
    const imports = [...out.matchAll(/^import .*?;$/gm)];
    if (imports.length) {
      const last = imports[imports.length - 1];
      out = out.slice(0, last.index + last[0].length) + importLine + out.slice(last.index + last[0].length);
    } else {
      out = out.replace(/(<script setup lang="ts">\n)/, `$1${importLine}\n`);
    }
  }

  touched += count;
  console.log(`${count} 处  ${relative(process.cwd(), file)}`);
  if (!DRY) writeFileSync(file, out, "utf8");
}

console.log(`\n共 ${touched} 处弹层${DRY ? "（dry-run，未写入）" : "已替换"}`);
