// 补 import：上一次 codemod 插 import 时在 </script> 之后找 import（永远找不到），
// 导致 13 个文件漏引。本脚本只做「按模板里实际用到的 <app-*> 补 import」这一件事。
import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, dirname, relative } from "node:path";

const ROOT = join(process.cwd(), "src");
const COMPS = ["AppInput", "AppSwitch", "AppSlider", "AppSelect", "AppSheet", "AppPopup"];

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

for (const file of vueFiles(ROOT)) {
  let out = readFileSync(file, "utf8");
  const headEnd = out.indexOf("</script>");
  const head = out.slice(0, headEnd);
  const bodyNoImports = head.replace(/^import .*?;$/gm, "");
  const needed = COMPS.filter(
    (c) =>
      out.includes("<" + c.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase()) &&
      !new RegExp("import\\s+" + c + "\\s+from").test(out)
  );
  if (!needed.length) continue;
  const lines = [];
  for (const c of needed) {
    let rel = relative(dirname(file), join(ROOT, "ui", c + ".vue")).replace(/\\/g, "/");
    if (!rel.startsWith(".")) rel = "./" + rel;
    lines.push(`import ${c} from "${rel}";`);
  }
  const imports = [...out.matchAll(/^import .*?;$/gm)];
  const last = imports[imports.length - 1];
  out =
    out.slice(0, last.index + last[0].length) +
    "\n" +
    lines.join("\n") +
    out.slice(last.index + last[0].length);
  writeFileSync(file, out, "utf8");
  console.log(`${needed.join(", ")}  →  ${relative(process.cwd(), file)}`);
}
