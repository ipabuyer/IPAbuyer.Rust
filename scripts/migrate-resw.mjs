// resw → i18n JSON 迁移脚本（一次性工具）
// 用法: node scripts/migrate-resw.mjs <Resources.resw> <输出.json>
// key 原样保留（含 .Content/.Header 等属性后缀，避免同名不同义合并冲突）
import { readFileSync, writeFileSync } from "node:fs";

const [reswPath, outPath] = process.argv.slice(2);
if (!reswPath || !outPath) {
  console.error("用法: node scripts/migrate-resw.mjs <Resources.resw> <输出.json>");
  process.exit(1);
}

const xml = readFileSync(reswPath, "utf8");
const decode = (s) =>
  s
    .replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(Number(n)))
    .replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCodePoint(parseInt(n, 16)))
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, "&");

const entries = [...xml.matchAll(/<data name="([^"]+)"[^>]*>\s*<value>([\s\S]*?)<\/value>/g)];
const result = {};
for (const [, name, value] of entries) {
  const key = name.trim();
  const text = decode(value).trim();
  if (key in result && result[key] !== text) {
    console.error(`警告: key 重复且值不同: ${key}`);
  }
  result[key] = text;
}
writeFileSync(outPath, JSON.stringify(result, null, 2) + "\n", "utf8");
console.log(`${Object.keys(result).length} keys → ${outPath}`);
