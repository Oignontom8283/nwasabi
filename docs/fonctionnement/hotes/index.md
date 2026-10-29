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

## Dans le code

| Hôte | Où |
|---|---|
| WebAssembly (interface du module) | [`src/wasm.rs`](code:src/wasm.rs) |
| Page web | [`web/src/`](code:web/src/) : [`emulator.ts`](code:web/src/emulator.ts), [`emulator.worker.ts`](code:web/src/emulator.worker.ts), [`worker-protocol.ts`](code:web/src/worker-protocol.ts) |
| Bibliothèque JavaScript | [`packages/js/src/`](code:packages/js/src/), module : [`packages/wasm/`](code:packages/wasm/) |
| Serveur | [`src/server/main.rs`](code:src/server/main.rs) |
| Ligne de commande | [`src/cli/`](code:src/cli/) : [`main.rs`](code:src/cli/main.rs), [`commands.rs`](code:src/cli/commands.rs), [`options.rs`](code:src/cli/options.rs) |
| Construction | [`build.rs`](code:build.rs), [`justfile`](code:justfile) |

## Pourquoi quatre hôtes sur le même cœur

Le cœur est conçu pour ne dépendre de rien : un hôte n'est qu'un petit programme qui fait **trois choses** — créer une `Session`, lui envoyer des requêtes, lui faire passer le temps. Tout ce qui est « produit » (écran, GDB, historique, formats) est dans le cœur, donc **gratuit** pour un nouvel hôte.

## Pour modifier

- **Un nouvel hôte** (une application de bureau, un plugin) : appeler la session comme [`wasm.rs`](code:src/wasm.rs) ou [`cli/commands.rs`](code:src/cli/commands.rs) le font ; ne pas dupliquer de logique du cœur.
- **Une option de la ligne de commande ou du serveur** : l'ajouter à [`options.rs`](code:src/cli/options.rs) ou à la structure d'options du serveur, la documenter dans le guide ([ligne de commande](../../guide/cli.md), [serveur](../../guide/serveur.md)), et l'éprouver dans [`cli.rs`](code:tests/cli.rs) ou [`server.rs`](code:tests/server.rs).
