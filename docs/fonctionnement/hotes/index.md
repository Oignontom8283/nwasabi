---
title: "Les hôtes"
description: "Les quatre façons d'exécuter le cœur de Nwasabi : navigateur (WebAssembly dans un Worker), serveur, ligne de commande et bibliothèque JavaScript, avec leurs responsabilités."
---

# Les hôtes

::: tip En bref
Le cœur ne sait pas ce qu'est « maintenant » ni « un fichier ». Un **hôte** lui apporte tout cela : la page web, le serveur, la ligne de commande ou la bibliothèque JavaScript.
:::


Le cœur est pur : il ne dort pas, ne lit pas d'horloge, n'ouvre pas de fichier. Un **hôte** lui apporte ce qu'il lui manque : le temps réel, les touches, le disque, le réseau, l'affichage. Quatre hôtes existent ; les trois premiers partagent le même
protocole de [session](../debogage/session.md).

| Hôte | Fichier | Exécute le cœur… | Responsabilités |
|---|---|---|---|
| **Page web** | `web/` (Vue 3), `emulator.worker.ts` | en WebAssembly, dans un **Worker** | horloge (`requestAnimationFrame` ou minuterie), touches et gestes, écran, fichiers, conservation locale, interface |
| **Serveur** | `src/server/main.rs` (`nwasabi`) | n'exécute rien (sauf `--gdb-headless`) | sert la page (embarquée dans le binaire), relaie `--nwa`, `--nmk`, `--nfs`, `--flash`, `--nds`, `--src`, pont GDB |
| **Ligne de commande** | `src/cli/` (`nwasabi-cli`) | en natif | exécutions déterministes, liaison, désassemblage, rejeu `.nrp`, session JSON sur l'entrée standard |
| **Bibliothèque JS** | `packages/js` (`@nwasabi/js`), `packages/wasm` (`@nwasabi/wasm`) | en WebAssembly (navigateur ou Node.js) | `Player` : cadence, touches, gestes, écran dans un canvas ou en pixels bruts |

## Pages

- [WebAssembly](./wasm.md) : l'interface `extern "C"`, la mémoire partagée, la construction du module
- [La page web](./web.md) : le Worker, le protocole de messages, l'écran image par image
- [Le serveur](./serveur.md) : routes, fichiers donnés au démarrage, pont GDB
- [La bibliothèque JavaScript](./bibliotheque-js.md) : `Player`, cadence, entrées
