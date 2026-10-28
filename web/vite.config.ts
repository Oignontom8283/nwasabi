import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Les chemins viennent de `build.rs` (compilation du serveur) :
//   NWASABI_WASM     le module WebAssembly compilé ;
//   NWASABI_WEB_OUT  le dossier où écrire le site (embarqué ensuite dans le binaire).
// Hors de cargo (`npm run dev`, `npm run build` à la main), on prend les emplacements usuels :
// le module construit par `just build-wasm` et un dossier `dist/` local.
const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const wasm = process.env.NWASABI_WASM ?? here("../target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
const outDir = process.env.NWASABI_WEB_OUT ?? here("./dist");

// Texte propre à celui qui compile le site, affiché dans la fenêtre « i » de la barre du haut (Markdown simple) :
//   NWASABI_INFO       le texte lui-même ;
//   NWASABI_INFO_FILE  ou le chemin d'un fichier Markdown ;
//   NWASABI_INFO_TITLE le titre de la fenêtre (par défaut « Informations »).
// Sans texte, le bouton « i » n'apparaît pas.
const infoFile = process.env.NWASABI_INFO_FILE;
const info = infoFile ? readFileSync(infoFile, "utf8") : (process.env.NWASABI_INFO ?? "");

// Chemins relatifs pour pouvoir servir le site de n'importe où.
export default defineConfig({
  base: "./",
  plugins: [vue()],
  define: {
    __NWASABI_INFO__: JSON.stringify(info.trim()),
    __NWASABI_INFO_TITLE__: JSON.stringify(process.env.NWASABI_INFO_TITLE ?? ""),
  },
  // `import "nwasabi.wasm?url"` désigne le module construit (la requête `?url` est conservée).
  resolve: { alias: [{ find: /^nwasabi\.wasm(\?.*)?$/, replacement: `${wasm}$1` }] },
  server: { fs: { allow: [here(".."), here("../target")] } },
  // L'émulateur est une seule page : son code (Vue, les traductions, l'interface) tient dans un fichier d'environ 600 Kio.
  build: { outDir, emptyOutDir: true, assetsInlineLimit: 0, chunkSizeWarningLimit: 700 },
});
