// Liens vers le code source depuis la documentation.
//
// Dans une page : `[`step_inline`](code:src/cpu/exec.rs#step_inline)`. Au moment de la construction, ce lien est remplacé par
// l'adresse GitHub du fichier, avec les **lignes** de la définition (`#L102-L155`). Le symbole est cherché dans le fichier
// au moment de la construction : s'il a été renommé ou supprimé, **la construction échoue**, ce qui garde la documentation
// à jour avec le code.
//
// Formes acceptées après `code:` :
//   chemin/du/fichier.rs               tout le fichier
//   chemin/du/dossier/                 un dossier
//   chemin/du/fichier.rs#nom           la définition de `nom` (fn, struct, enum, trait, type, const, static, mod, class, function…)
//   chemin/du/fichier.rs#nom:2         la 2ᵉ définition de ce nom dans le fichier
//   chemin/du/fichier.rs#~texte        la première ligne qui contient `texte`
//   chemin/du/fichier.rs#L10-L40       des lignes données
//
// La branche ou l'étiquette visée est `NWASABI_REF` (par défaut `main`).

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const repo = "https://github.com/Oignontom8283/nwasabi";
const ref = process.env.NWASABI_REF ?? "main";

const cache = new Map();
function lines(file) {
  if (!cache.has(file)) cache.set(file, fs.readFileSync(path.join(root, file), "utf8").split("\n"));
  return cache.get(file);
}

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Expression qui reconnaît la ligne où `name` est défini. */
function definition(name) {
  const n = escape(name);
  return new RegExp(
    `^\\s*(?:pub(?:\\([a-z]+\\))?\\s+)?(?:export\\s+)?(?:default\\s+)?(?:async\\s+)?(?:unsafe\\s+)?(?:extern\\s+"C"\\s+)?(?:const\\s+)?(?:fn|struct|enum|trait|type|mod|static|const|class|interface|function|let|def)\\s+${n}\\b|^\\s*(?:pub(?:\\([a-z]+\\))?\\s+)?(?:static\\s+)?${n}\\s*[:=]|^\\s*(?:(?:private|public|protected|static|async|readonly)\\s+)*${n}\\s*(?:<[^>]*>)?\\(.*\\{\\s*$`,
  );
}

/** Dernière ligne du bloc qui commence à `start` (0-indexé) : accolades appariées, ou la ligne même si elle finit par `;`. */
function end(all, start) {
  let depth = 0;
  let opened = false;
  for (let i = start; i < Math.min(all.length, start + 600); i++) {
    const text = all[i].replace(/\/\/.*$/, "").replace(/"(?:[^"\\]|\\.)*"/g, '""').replace(/'(?:[^'\\]|\\.)'/g, "''");
    for (const c of text) {
      if (c === "{") {
        depth++;
        opened = true;
      } else if (c === "}") depth--;
    }
    if (opened && depth <= 0) return i;
    if (!opened && /;\s*$/.test(text)) return i;
  }
  return start;
}

export function resolve(spec, where) {
  const [file, anchor] = spec.split("#");
  const full = path.join(root, file);
  if (!fs.existsSync(full)) throw new Error(`code: « ${spec} » (${where}) : fichier introuvable`);
  if (file.endsWith("/")) return `${repo}/tree/${ref}/${file}`;
  const base = `${repo}/blob/${ref}/${file}`;
  if (!anchor) return base;
  const range = anchor.match(/^L(\d+)(?:-L(\d+))?$/);
  if (range) return `${base}#L${range[1]}${range[2] ? `-L${range[2]}` : ""}`;
  const all = lines(file);
  let index = -1;
  if (anchor.startsWith("~")) {
    index = all.findIndex((l) => l.includes(anchor.slice(1)));
  } else {
    const [name, nth = "1"] = anchor.split(":");
    const re = definition(name);
    let seen = 0;
    for (let i = 0; i < all.length; i++) {
      if (re.test(all[i]) && ++seen === Number(nth)) {
        index = i;
        break;
      }
    }
  }
  if (index < 0) throw new Error(`code: « ${spec} » (${where}) : définition introuvable dans ${file}`);
  const last = end(all, index);
  return `${base}#L${index + 1}${last > index ? `-L${last + 1}` : ""}`;
}

export function codeLinks(md) {
  md.core.ruler.push("nwasabi_code_links", (state) => {
    const where = state.env?.relativePath ?? "?";
    for (const block of state.tokens) {
      if (block.type !== "inline" || !block.children) continue;
      for (const token of block.children) {
        // Un lien `code:` mal écrit (espaces, parenthèse oubliée) n'est pas reconnu par Markdown : on le détecte.
        if (token.type === "text" && /\]\(<?code:/.test(token.content)) {
          throw new Error(`code: lien mal formé dans ${where} : ${token.content.slice(0, 120)}`);
        }
        if (token.type !== "link_open") continue;
        const href = token.attrGet("href");
        if (!href?.startsWith("code:")) continue;
        token.attrSet("href", resolve(decodeURIComponent(href.slice(5)), where));
      }
    }
  });
}
