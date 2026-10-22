// Copie le module compilé par `just build-wasm` à la racine du paquet (le fichier n'est pas versionné :
// il n'existe que pour publier). `NWASABI_WASM` désigne un autre fichier.
import { copyFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const source =
  process.env.NWASABI_WASM ?? join(here, "../../../target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
if (!existsSync(source)) {
  console.error(`module introuvable : ${source}\nLancer d'abord « just build-wasm ».`);
  process.exit(1);
}
copyFileSync(source, join(here, "../nwasabi.wasm"));
console.log(`copié : ${source}`);
