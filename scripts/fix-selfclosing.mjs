// 修：上一步把自闭合标签的 `/>` 去掉后没补回，导致后续内容被解析成该组件的子节点。
// 判据：某文件里如果**完全没有**对应的闭标签，说明原来的用法全是自闭合 → 补回 `/>`。
import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = join(process.cwd(), "src");
const TAGS = ["app-input", "app-switch", "app-slider", "app-select"];

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

/** 引号感知地找 `<tag ...>` 的结束 `>`，并报告该标签是否已自闭合 */
function scan(src, tag) {
  const hits = [];
  let from = 0;
  for (;;) {
    const start = src.indexOf("<" + tag, from);
    if (start === -1) break;
    const after = src[start + tag.length + 1];
    if (after && /[\w-]/.test(after)) {
      from = start + tag.length + 1;
      continue;
    }
    let i = start + tag.length + 1;
    let quote = null;
    while (i < src.length) {
      const c = src[i];
      if (quote) {
        if (c === quote) quote = null;
      } else if (c === '"' || c === "'") quote = c;
      else if (c === ">") break;
      i++;
    }
    hits.push({ start, gt: i, selfClosed: src[i - 1] === "/" });
    from = i + 1;
  }
  return hits;
}

let fixed = 0;
for (const file of vueFiles(ROOT)) {
  let out = readFileSync(file, "utf8");
  const orig = out;
  for (const tag of TAGS) {
    const hasClosing = out.includes("</" + tag + ">");
    const hits = scan(out, tag);
    if (!hits.length || hasClosing) continue; // 有闭标签 = 成对用法，不动
    for (const h of hits.reverse()) {
      if (h.selfClosed) continue;
      out = out.slice(0, h.gt - 1).trimEnd() + " />" + out.slice(h.gt);
      fixed++;
    }
  }
  if (out !== orig) {
    writeFileSync(file, out, "utf8");
    console.log("  修正 " + relative(process.cwd(), file));
  }
}
console.log("\n共补回 " + fixed + " 个自闭合斜杠");
