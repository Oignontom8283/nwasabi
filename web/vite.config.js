import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Le site construit va dans `web-dist/` à la racine du dépôt : c'est ce dossier que le serveur
// `nwasabi` embarque (voir build.rs). Chemins relatifs pour pouvoir le servir de n'importe où.
export default defineConfig({
  base: "./",
  plugins: [vue()],
  build: { outDir: "../web-dist", emptyOutDir: true, assetsInlineLimit: 0 },
});
