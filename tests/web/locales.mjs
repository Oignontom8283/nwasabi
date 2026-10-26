// Les fichiers de langue de l'interface web : mêmes clés partout, aucun texte vide, et toutes les clés
// écrites en dur dans le code (`t("…")`) existent.
//
//   node tests/web/locales.mjs

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const web = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "web", "src");
const flatten = (object, prefix = "") =>
  Object.entries(object).flatMap(([key, value]) =>
    typeof value === "object" ? flatten(value, `${prefix}${key}.`) : [[`${prefix}${key}`, value]],
  );
const load = (code) => Object.fromEntries(flatten(JSON.parse(readFileSync(join(web, "locales", `${code}.json`), "utf8"))));
const locales = Object.fromEntries(readdirSync(join(web, "locales")).map((f) => [f.replace(".json", ""), load(f.replace(".json", ""))]));

const problems = [];
const codes = Object.keys(locales);
const all = new Set(codes.flatMap((c) => Object.keys(locales[c])));
for (const key of all) {
  for (const code of codes) {
    const text = locales[code][key];
    if (text === undefined) problems.push(`${code} : clé manquante « ${key} »`);
    else if (String(text).trim() === "") problems.push(`${code} : texte vide « ${key} »`);
  }
}
// Les paramètres {nom} doivent être les mêmes dans toutes les langues.
for (const key of all) {
  const params = codes.map((c) => [...String(locales[c][key] ?? "").matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(","));
  if (new Set(params).size > 1) problems.push(`paramètres différents pour « ${key} » : ${params.join(" | ")}`);
}

// Clés utilisées dans le code : `t("a.b")`, `t('a.b')` ; les clés construites (`t(\`a.${x}\`)`) ne sont pas vérifiées.
const files = [];
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (/\.(vue|js)$/.test(name)) files.push(path);
  }
};
walk(web);
const used = new Set();
for (const file of files) {
  for (const m of readFileSync(file, "utf8").matchAll(/\bt\(\s*["']([\w.]+)["']/g)) used.add(m[1]);
}
for (const key of used) if (!all.has(key)) problems.push(`utilisée mais absente des langues : « ${key} »`);

if (problems.length) {
  console.log(problems.join("\n"));
  process.exit(1);
}
console.log(`ok   ${all.size} textes, ${codes.length} langues (${codes.join(", ")}), ${used.size} clés utilisées dans le code`);
