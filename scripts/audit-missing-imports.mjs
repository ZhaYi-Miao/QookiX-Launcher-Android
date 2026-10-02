// 检查「模板里用到、脚本里没引入」的组件（包括本地组件与 icons）。
// 这类问题构建不报错，只在运行时渲染成空白 —— 手机上表现为「这里的按钮不见了」。
// 用法：node scripts/audit-missing-imports.mjs
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, dirname } from "node:path";

const ROOT = join(process.cwd(), "src");

function files(dir, acc = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) files(p, acc);
    else if (name.endsWith(".vue")) acc.push(p);
  }
  return acc;
}

/** 已知的全局注册组件 / Vant 标签 / 原生标签前缀，不需要 import */
const OK_PREFIX = ["van-", "n-", "router-", "transition", "template", "component", "slot", "teleport", "keep-alive", "suspense"];

let bad = 0;
for (const file of files(ROOT)) {
  const src = readFileSync(file, "utf8");
  const tplStart = src.indexOf("<template>");
  const tplEnd = src.lastIndexOf("</template>");
  if (tplStart === -1 || tplEnd === -1) continue;
  const tpl = src.slice(tplStart, tplEnd);
  const script = src.slice(0, tplStart);

  // 收集模板里用到的 PascalCase / kebab-case 自定义标签
  const tags = new Set();
  for (const m of tpl.matchAll(/<([A-Za-z][\w-]*)/g)) {
    const t = m[1];
    if (OK_PREFIX.some((p) => t.toLowerCase().startsWith(p))) continue;
    if (/^h[1-6]$/.test(t)) continue; // 标题标签，别当组件
    if (/^[a-z]+$/.test(t) && !t.includes("-")) {
      // 纯小写单词：可能是 html 原生标签，白名单外的一律跳过（避免误报）
      const html = ["div","span","p","a","b","i","em","strong","ul","li","ol","h1","h2","h3","h4","h5","h6","button","input","textarea","label","img","canvas","video","audio","pre","code","table","thead","tbody","tr","th","td","form","select","option","br","hr","small","sub","sup","svg","path","circle","rect","line","polyline","polygon","g","defs","use","section","header","footer","main","aside","nav","article","figure","figcaption","summary","details","dialog","progress","time","mark","abbr","cite","q","blockquote","iframe","source","track","picture","map","area","menu","output","ruby","rt","rp","wbr","datalist","fieldset","legend","optgroup","col","colgroup","caption","del","ins","kbd","samp","var"];
      if (html.includes(t)) continue;
    }
    tags.add(t);
  }

  const missing = [];
  for (const tag of tags) {
    // kebab → Pascal：icon-close → IconClose
    const pascal = tag.replace(/(^|-)([a-z0-9])/g, (_m, _p, c) => c.toUpperCase());
    if (new RegExp("\\b" + pascal + "\\b").test(script)) continue;
    if (new RegExp("\\b" + tag + "\\b").test(script)) continue;
    missing.push(tag);
  }
  if (missing.length) {
    bad += missing.length;
    console.log(`${relative(process.cwd(), file)}: ${missing.join(", ")}`);
  }
}
console.log(`\n共 ${bad} 处「模板在用、脚本没引入」`);
