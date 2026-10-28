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

// Les paramètres d'adresse que comprend l'émulateur (voir docs/guide/emulateur-web.md).
const EMULATOR_PARAMS = ["nwa", "data", "nmk", "nfs", "nds", "flash", "lang"];

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
  title: "Nwasabi",
  description: "Émulateur d'applications externes NumWorks (.nwa) en Rust et WebAssembly",
  base,
  cleanUrls: true,
  vite: { plugins: [emulatorIndex] },
  // Les logos sont ceux de `assets/logo/` : `just docs-emulator` les copie dans `public/` (VitePress ne sert que ce dossier).
  // L'icône de l'onglet est la version à fond arrondi ; le logo de la barre et de l'accueil, la version sans fond.
  // Une page de documentation ouverte avec un paramètre de l'émulateur (`?nwa=…`, `?flash=…`) est envoyée vers l'émulateur, paramètres
  // et ancre conservés. Le script est le même pour toutes les pages : le chemin de l'émulateur vient de `base`.
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: `${base}logo-bordered.svg` }],
    [
      "script",
      {},
      `(() => { const p = new URLSearchParams(location.search); if (${JSON.stringify(EMULATOR_PARAMS)}.some((k) => p.has(k))) location.replace(${JSON.stringify(`${base}emulator/`)} + location.search + location.hash); })()`,
    ],
  ],
  themeConfig: {
    logo: "/logo.svg",
    nav: [
      { text: "Émulateur", link: "/emulator/", target: "_self" },
      { text: "Guide", link: "/guide/" },
      { text: "Débogage", link: "/debug/" },
      { text: "Référence", link: "/reference/compatibilite" },
      { text: "Formats de fichiers", link: "/format/" },
    ],
    sidebar: {
      "/guide/": [
        {
          text: "Guide",
          items: [
            { text: "Premiers pas", link: "/guide/" },
            { text: "Écrire une application", link: "/guide/ecrire-une-application" },
            { text: "L'émulateur web", link: "/guide/emulateur-web" },
            { text: "Le serveur", link: "/guide/serveur" },
            { text: "La ligne de commande", link: "/guide/cli" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "Référence",
          items: [
            { text: "Compatibilité et limites", link: "/reference/compatibilite" },
            { text: "Modèles et mémoire", link: "/reference/modeles-memoire" },
            { text: "Appels système", link: "/reference/appels-systeme" },
            { text: "Protocole de session", link: "/reference/protocole-session" },
            { text: "Bibliothèque JavaScript", link: "/reference/bibliotheque-js" },
          ],
        },
      ],
    },
    socialLinks: [{ icon: "github", link: "https://github.com/Oignontom8283/nwasabi" }],
  },
});
