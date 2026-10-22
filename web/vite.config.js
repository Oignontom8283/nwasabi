import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Les chemins viennent de `build.rs` (compilation du serveur) :
//   NWASABI_WASM     le module WebAssembly compilé ;
//   NWASABI_WEB_OUT  le dossier où écrire le site (embarqué ensuite dans le binaire).
// Hors de cargo (`npm run dev`, `npm run build` à la main), on prend les emplacements usuels :
// le module construit par `just build-wasm` et un dossier `dist/` local.
const here = (path) => fileURLToPath(new URL(path, import.meta.url));
const wasm = process.env.NWASABI_WASM ?? here("../target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
const outDir = process.env.NWASABI_WEB_OUT ?? here("./dist");

// Chemins relatifs pour pouvoir servir le site de n'importe où.
export default defineConfig({
  base: "./",
  plugins: [vue()],
  // `import "nwasabi.wasm?url"` désigne le module construit (la requête `?url` est conservée).
  resolve: { alias: [{ find: /^nwasabi\.wasm(\?.*)?$/, replacement: `${wasm}$1` }] },
  server: { fs: { allow: [here(".."), here("../target")] } },
  build: { outDir, emptyOutDir: true, assetsInlineLimit: 0 },
});
