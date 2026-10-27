import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import type { Plugin } from "vite";
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

// Application installable et utilisable hors ligne : à la fin de la compilation, on écrit le manifeste (nom, icône) et le
// service worker, qui garde en mémoire tous les fichiers du site. Les adresses `__nwasabi/` du serveur (fichiers qui changent
// sur le disque) ne passent jamais par ce cache.
function offlineApp(): Plugin {
  return {
    name: "nwasabi-offline",
    apply: "build",
    generateBundle(_options, bundle) {
      const files = Object.keys(bundle);
      const icon = files.find((name) => /logo-bordered.*\.svg$/.test(name));
      this.emitFile({
        type: "asset",
        fileName: "manifest.webmanifest",
        source: JSON.stringify(
          {
            name: "Nwasabi",
            short_name: "Nwasabi",
            description: "Émulateur de calculatrices NumWorks",
            start_url: "./",
            scope: "./",
            display: "standalone",
            background_color: "#ffffff",
            theme_color: "#ffffff",
            icons: icon ? [{ src: icon, sizes: "any", type: "image/svg+xml", purpose: "any" }] : [],
          },
          null,
          2,
        ),
      });
      const version = createHash("sha256").update(files.join("|")).digest("hex").slice(0, 12);
      this.emitFile({
        type: "asset",
        fileName: "sw.js",
        source: serviceWorker(version, ["./", "manifest.webmanifest", ...files]),
      });
    },
  };
}

function serviceWorker(version: string, files: string[]): string {
  return `// Généré par la compilation (web/vite.config.ts).
const CACHE = "nwasabi-${version}";
const FILES = ${JSON.stringify(files)};

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(FILES)).then(() => self.skipWaiting()));
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((names) => Promise.all(names.filter((name) => name !== CACHE).map((name) => caches.delete(name)))).then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  // Rien d'autre que le site lui-même : ni les adresses du serveur de fichiers, ni d'autres sites.
  if (request.method !== "GET" || url.origin !== location.origin || url.pathname.includes("/__nwasabi/")) return;
  event.respondWith(
    // La page elle-même d'abord par le réseau (pour voir une nouvelle version), les autres fichiers (nommés par leur contenu) d'abord en mémoire.
    request.mode === "navigate"
      ? fetch(request).catch(() => caches.match("./"))
      : caches.match(request).then((hit) => hit || fetch(request)),
  );
});
`;
}

// Chemins relatifs pour pouvoir servir le site de n'importe où.
export default defineConfig({
  base: "./",
  plugins: [vue(), offlineApp()],
  define: {
    __NWASABI_INFO__: JSON.stringify(info.trim()),
    __NWASABI_INFO_TITLE__: JSON.stringify(process.env.NWASABI_INFO_TITLE ?? ""),
  },
  // `import "nwasabi.wasm?url"` désigne le module construit (la requête `?url` est conservée).
  resolve: { alias: [{ find: /^nwasabi\.wasm(\?.*)?$/, replacement: `${wasm}$1` }] },
  server: { fs: { allow: [here(".."), here("../target")] } },
  build: { outDir, emptyOutDir: true, assetsInlineLimit: 0 },
});
