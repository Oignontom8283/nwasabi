import { defineConfig } from "vitepress";
import { withMermaid } from "vitepress-plugin-mermaid";

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

// Les paramètres d'adresse qui désignent un fichier à ouvrir dans l'émulateur (voir docs/guide/emulateur-web.md). `lang` n'en fait pas partie : sur une page de la documentation, il appartiendra à la langue de la documentation.
const EMULATOR_PARAMS = ["nwa", "data", "nmk", "nfs", "nds", "flash", "model", "firmware"];

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

// `withMermaid` rend les blocs ```mermaid des pages en schémas.
export default withMermaid(defineConfig({
  lang: "fr",
  title: "Nwasabi",
  description: "Émulateur d'applications externes NumWorks (.nwa) en Rust et WebAssembly",
  base,
  cleanUrls: true,
  lastUpdated: true,
  // Le titre de chaque page vient de son en-tête (`title`) : « Page | Nwasabi » dans l'onglet.
  titleTemplate: ":title | Nwasabi",
  // Étiquettes en SVG et police de repli : en HTML, les styles de VitePress (interligne des paragraphes) font couper le texte des nœuds.
  mermaid: { fontFamily: "sans-serif", htmlLabels: false, flowchart: { htmlLabels: false, padding: 16, nodeSpacing: 40, rankSpacing: 60 } },
  // Mermaid et ses dépendances (CommonJS) doivent être préparées par Vite en développement, sinon la page reste blanche.
  // Les schémas de Mermaid sont chargés à la demande, page par page : leurs gros fichiers ne pèsent pas sur les autres pages.
  vite: { plugins: [emulatorIndex], optimizeDeps: { include: ["mermaid"] }, build: { chunkSizeWarningLimit: 1000 } },
  // Les logos sont ceux de `assets/logo/` : `just docs-emulator` les copie dans `public/` (VitePress ne sert que ce dossier).
  // L'icône de l'onglet est la version à fond arrondi ; le logo de la barre et de l'accueil, la version sans fond.
  // Une page de documentation ouverte avec un paramètre de l'émulateur (`?nwa=…`, `?flash=…`) est envoyée vers l'émulateur, paramètres
  // et ancre conservés. Le script est le même pour toutes les pages : le chemin de l'émulateur vient de `base`.
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: `${base}logo-bordered.svg` }],
    ["meta", { name: "author", content: "Oignontom8283" }],
    ["meta", { name: "keywords", content: "NumWorks, émulateur, nwa, Epsilon, WebAssembly, Rust, Cortex-M7, GDB, débogueur" }],
    ["meta", { name: "theme-color", content: "#74AD3A" }],
    ["meta", { property: "og:site_name", content: "Nwasabi" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:locale", content: "fr_FR" }],
    ["meta", { name: "twitter:card", content: "summary" }],
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
      { text: "Référence", link: "/reference/compatibilite" },
      { text: "Fonctionnement", link: "/fonctionnement/" },
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
        {
          text: "Déboguer",
          items: [
            { text: "Dans l'émulateur", link: "/guide/deboguer-emulateur" },
            { text: "En C, C++ et Rust", link: "/guide/deboguer-c-cpp-rust" },
          ],
        },
        {
          text: "Applications",
          items: [{ text: "Exécuter KhiCAS", link: "/guide/khicas" }],
        },
      ],
      "/reference/": [
        {
          text: "Référence",
          items: [
            { text: "Compatibilité et limites", link: "/reference/compatibilite" },
            { text: "Technologies prises en charge", link: "/reference/technologies" },
            { text: "Modèles et mémoire", link: "/reference/modeles-memoire" },
            { text: "Appels système", link: "/reference/appels-systeme" },
            { text: "Données auxiliaires en flash", link: "/reference/donnees-flash" },
            { text: "Protocole de session", link: "/reference/protocole-session" },
            { text: "Bibliothèque JavaScript", link: "/reference/bibliotheque-js" },
          ],
        },
        {
          text: "Formats de fichiers",
          link: "/reference/formats/",
          items: [
            { text: "Vue d'ensemble", link: "/reference/formats/" },
            { text: ".nmk : touches", link: "/reference/formats/nmk" },
            { text: ".nfs : fichiers", link: "/reference/formats/nfs" },
            { text: ".nfd : données en flash", link: "/reference/formats/nfd" },
            { text: ".nds : état", link: "/reference/formats/nds" },
            { text: ".nrp : enregistrement", link: "/reference/formats/nrp" },
          ],
        },
      ],
      "/fonctionnement/": [
        {
          text: "Fonctionnement",
          items: [{ text: "Vue d'ensemble", link: "/fonctionnement/" }],
        },
        {
          text: "Processeur",
          collapsed: false,
          items: [
            { text: "Le processeur", link: "/fonctionnement/processeur/" },
            { text: "Décodage et cache", link: "/fonctionnement/processeur/decodage" },
            { text: "Exécution", link: "/fonctionnement/processeur/execution" },
            { text: "Virgule flottante", link: "/fonctionnement/processeur/virgule-flottante" },
            { text: "Exceptions et fautes", link: "/fonctionnement/processeur/exceptions" },
            { text: "Désassembleur", link: "/fonctionnement/processeur/desassembleur" },
          ],
        },
        {
          text: "Mémoire et appareil",
          collapsed: false,
          items: [
            { text: "La mémoire", link: "/fonctionnement/memoire/" },
            { text: "L'appareil", link: "/fonctionnement/memoire/appareil" },
          ],
        },
        {
          text: "Noyau simulé",
          collapsed: false,
          items: [
            { text: "Le noyau", link: "/fonctionnement/noyau/" },
            { text: "Appels système", link: "/fonctionnement/noyau/appels-systeme" },
            { text: "Écran et police", link: "/fonctionnement/noyau/ecran-et-police" },
            { text: "Clavier et événements", link: "/fonctionnement/noyau/clavier" },
            { text: "Fichiers", link: "/fonctionnement/noyau/fichiers" },
          ],
        },
        {
          text: "Édition de liens",
          collapsed: false,
          items: [
            { text: "Vue d'ensemble", link: "/fonctionnement/edition-de-liens/" },
            { text: "Lecture ELF", link: "/fonctionnement/edition-de-liens/elf" },
            { text: "Symboles et sections", link: "/fonctionnement/edition-de-liens/symboles" },
            { text: "Placement", link: "/fonctionnement/edition-de-liens/placement" },
            { text: "Relocations", link: "/fonctionnement/edition-de-liens/relocations" },
            { text: "Le runtime EADK", link: "/fonctionnement/edition-de-liens/runtime" },
            { text: "Application et ELF", link: "/fonctionnement/edition-de-liens/application" },
          ],
        },
        {
          text: "Exécution",
          collapsed: false,
          items: [{ text: "L'exécution coopérative", link: "/fonctionnement/execution/" }],
        },
        {
          text: "Débogage",
          collapsed: false,
          items: [
            { text: "Vue d'ensemble", link: "/fonctionnement/debogage/" },
            { text: "DWARF", link: "/fonctionnement/debogage/dwarf" },
            { text: "Pile d'appels", link: "/fonctionnement/debogage/pile" },
            { text: "Expressions", link: "/fonctionnement/debogage/expressions" },
            { text: "C++ et Rust", link: "/fonctionnement/debogage/langages" },
            { text: "Historique", link: "/fonctionnement/debogage/historique" },
            { text: "La session", link: "/fonctionnement/debogage/session" },
            { text: "Serveur GDB", link: "/fonctionnement/debogage/gdb" },
          ],
        },
        {
          text: "Hôtes",
          collapsed: false,
          items: [
            { text: "Vue d'ensemble", link: "/fonctionnement/hotes/" },
            { text: "WebAssembly", link: "/fonctionnement/hotes/wasm" },
            { text: "La page web", link: "/fonctionnement/hotes/web" },
            { text: "Le serveur", link: "/fonctionnement/hotes/serveur" },
            { text: "Bibliothèque JavaScript", link: "/fonctionnement/hotes/bibliotheque-js" },
          ],
        },
        {
          text: "Vérification",
          items: [{ text: "Vérification", link: "/fonctionnement/verification/" }],
        },
      ],
    },
    search: { provider: "local" },
    outline: { level: [2, 3], label: "Sur cette page" },
    docFooter: { prev: "Page précédente", next: "Page suivante" },
    lastUpdated: { text: "Dernière modification" },
    returnToTopLabel: "Retour en haut",
    sidebarMenuLabel: "Menu",
    darkModeSwitchLabel: "Thème",
    editLink: { pattern: "https://github.com/Oignontom8283/nwasabi/edit/main/docs/:path", text: "Modifier cette page" },
    socialLinks: [{ icon: "github", link: "https://github.com/Oignontom8283/nwasabi" }],
  },
  // Chaque page reprend son titre et sa description dans les balises sociales (`og:`, `twitter:`).
  transformPageData(pageData) {
    const { title, description } = pageData.frontmatter.title ? pageData.frontmatter : pageData;
    const head = (pageData.frontmatter.head ??= []);
    if (title) {
      head.push(["meta", { property: "og:title", content: `${title} | Nwasabi` }]);
      head.push(["meta", { name: "twitter:title", content: `${title} | Nwasabi` }]);
    }
    if (description) {
      head.push(["meta", { property: "og:description", content: description }]);
      head.push(["meta", { name: "twitter:description", content: description }]);
    }
  },
}));
