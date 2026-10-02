// codemod：把 naive 的表单控件换成 ui/ 下的 Vant 兼容层组件。
// 用法：node scripts/migrate-controls.mjs [--dry] [n-input|n-switch|n-slider|n-select ...]
//
// 映射：n-input→app-input、n-switch→app-switch、n-slider→app-slider、n-select→app-select
// 契约对齐（v-model:value / :value / :options / @update:value），调用点只换标签名。
//
// 这一版修掉的三类事故（前一版把模板切坏过，务必保持）：
//  1. 属性按空白切分会切碎值（`:show="a !== b"`）→ 引号感知地扫描；
//  2. 开标签不能靠非贪婪正则（属性值里的 `>` 会截断）→ 手写扫描跳过引号内容；
//  3. **自闭合必须原样保留**：`<n-input ... />` 若去掉斜杠又不补回，这个元素就变成
//     「永不闭合」，后面整段内容会被解析成它的子节点（当时把 TS 的类型窄化
//     泄漏到了兄弟节点上才发现）。attrs 一律原样搬运，不做任何空白规整。
import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, dirname, relative } from "node:path";

const DRY = process.argv.includes("--dry");
const MAP = { "n-input": "app-input", "n-switch": "app-switch", "n-slider": "app-slider", "n-select": "app-select" };
const want = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const targets = Object.keys(MAP).filter((t) => !want.length || want.includes(t));
if (!targets.length) {
  console.error("没有可迁移的控件（可用：n-input / n-switch / n-slider / n-select）");
  process.exit(1);
}
const ROOT = join(process.cwd(), "src");

function vueFiles(dir, acc = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (name === "ui") continue;
      vueFiles(p, acc);
    } else if (name.endsWith(".vue")) acc.push(p);
  }
  return acc;
}

/** 引号感知地定位 `<tag ...>`；返回 null 或 {start, gt, attrs, selfClosed} */
function findOpenTag(src, tag, from) {
  const needle = "<" + tag;
  let start = src.indexOf(needle, from);
  while (start !== -1) {
    const after = src[start + needle.length];
    if (!after || !/[\w-]/.test(after)) break; // 防 <n-input-number 误命中
    start = src.indexOf(needle, start + needle.length);
  }
  if (start === -1) return null;
  let i = start + needle.length;
  let quote = null;
  while (i < src.length) {
    const c = src[i];
    if (quote) {
      if (c === quote) quote = null;
    } else if (c === '"' || c === "'") quote = c;
    else if (c === ">") {
      const before = src.slice(start, i).trimEnd();
      return { start, gt: i, attrs: src.slice(start + needle.length, i), selfClosed: before.endsWith("/") };
    }
    i++;
  }
  return null;
}

const kebab = (sym) => "n-" + sym.replace(/^N/, "").replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();

let touched = 0;
for (const file of vueFiles(ROOT)) {
  const src = readFileSync(file, "utf8");
  if (!targets.some((t) => src.includes("<" + t))) continue;
  let out = src;

  for (const tag of targets) {
    const comp = MAP[tag];
    for (;;) {
      const t = findOpenTag(out, tag, 0);
      if (!t) break;
      // attrs 原样搬运；自闭合形态原样保留（关键！）
      const attrs = t.selfClosed ? t.attrs.replace(/\s*\/\s*$/, " /") : t.attrs;
      const open = "<" + comp + attrs + ">";
      let after = out.slice(t.gt + 1);
      const closing = "</" + tag + ">";
      const ci = after.indexOf(closing);
      if (!t.selfClosed && ci !== -1) {
        after = after.slice(0, ci) + "</" + comp + ">" + after.slice(ci + closing.length);
      }
      out = out.slice(0, t.start) + open + after;
      touched++;
    }
  }

  const scriptEnd = out.indexOf("</script>");
  const bodyOnly = out.slice(0, scriptEnd).replace(/^import .*?;$/gm, "");
  const templateOnly = out.slice(scriptEnd);

  // 1) naive-ui import：组件按**精确标签**判断是否还在用（<n-input-number 不能算 <n-input>）
  out = out.replace(/import\s*\{([^}]*)\}\s*from\s*"naive-ui";/, (_m, names) => {
    const kept = names.split(",").map((s) => s.trim()).filter((n) => {
      if (!n) return false;
      if (!/^N[A-Z]/.test(n)) return true; // useMessage / useDialog 等函数
      return new RegExp("<" + kebab(n) + "[\\s/>]").test(bodyOnly + templateOnly);
    });
    return kept.length ? "import { " + kept.join(", ") + " } from \"naive-ui\";" : "";
  });

  // 2) 补 ui/ 组件 import：标签在**模板**（</script> 之后）里找，import 在**全文**里找
  const comps = [...new Set(Object.values(MAP).filter((c) => templateOnly.includes("<" + c)))];
  for (const c of comps) {
    const name = c.replace(/(^|-)([a-z])/g, (_m, _p, ch) => ch.toUpperCase());
    if (new RegExp("import\\s+" + name + "\\s+from").test(out)) continue;
    let rel = relative(dirname(file), join(ROOT, "ui", name + ".vue")).replace(/\\/g, "/");
    if (!rel.startsWith(".")) rel = "./" + rel;
    const line = "import " + name + " from \"" + rel + "\";";
    const imports = [...out.matchAll(/^import .*?;$/gm)];
    const last = imports[imports.length - 1];
    out = out.slice(0, last.index + last[0].length) + "\n" + line + out.slice(last.index + last[0].length);
  }

  console.log(relative(process.cwd(), file));
  if (!DRY) writeFileSync(file, out, "utf8");
}
console.log("\n共 " + touched + " 处控件" + (DRY ? "（dry-run，未写入）" : "已替换"));
