import { defineConfig } from "vitepress";

// Site vitrine de nwasabi : documentation et tutoriels (pour l'instant, seulement l'accueil), avec
// l'émulateur dans le même site statique, à l'adresse /emulator/.
//
// L'émulateur n'est pas une page VitePress : c'est le site de `web/`, construit par `just site` dans
// `docs/public/emulator/` (VitePress recopie `public/` tel quel à la racine du site construit).
// Les liens qui y mènent portent `target: "_self"` pour que VitePress les laisse au navigateur
// au lieu de les traiter comme une page de son routeur.
//
// `SITE_BASE` règle le chemin de publication quand le site n'est pas à la racine du domaine
// (par exemple `/nwasabi/` sur GitHub Pages).
const base = process.env.SITE_BASE ?? "/";

// En développement, le serveur de VitePress répond à `/emulator/` avec sa propre application (donc sa
// page 404) : il ne sert que les fichiers exacts de `public/`, pas l'`index.html` d'un dossier. Ce plugin
// (inutile dans le site construit, où le serveur statique s'en charge) renvoie la page de l'émulateur.
const emulatorIndex = {
  name: "nwasabi-emulator-index",
  configureServer(server) {
    server.middlewares.use((request, _response, next) => {
      const [path, query = ""] = (request.url ?? "").split("?");
      const folder = `${base}emulator`;
      if (path === folder || path === `${folder}/`) {
        request.url = `${folder}/index.html${query ? `?${query}` : ""}`;
      }
      next();
    });
  },
};

export default defineConfig({
  lang: "fr",
  title: "nwasabi",
  description: "Émulateur d'applications externes NumWorks (.nwa) en Rust et WebAssembly",
  base,
  cleanUrls: true,
  vite: { plugins: [emulatorIndex] },
  themeConfig: {
    nav: [
      { text: "Émulateur", link: "/emulator/", target: "_self" },
      { text: "Formats de fichiers", link: "/format/" },
    ],
    socialLinks: [{ icon: "github", link: "https://github.com/Oignontom8283/nwasabi" }],
  },
});
