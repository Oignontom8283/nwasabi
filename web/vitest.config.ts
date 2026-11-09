import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";
import vue from "@vitejs/plugin-vue";

// Tests de l'interface sous Node.js, dans un DOM simulé (aucun navigateur). `vite.config.ts` n'est pas repris : il exige le module WebAssembly.
export default defineConfig({
  plugins: [vue()],
  define: { __NWASABI_INFO__: JSON.stringify(""), __NWASABI_INFO_TITLE__: JSON.stringify("") },
  // Le module WebAssembly n'est pas exécuté par ces tests : une adresse factice suffit à résoudre l'import.
  resolve: { alias: [{ find: /^nwasabi\.wasm(\?.*)?$/, replacement: fileURLToPath(new URL("./tests/wasm-url.ts", import.meta.url)) }] },
  test: { environment: "happy-dom", include: ["tests/**/*.test.ts"] },
});
