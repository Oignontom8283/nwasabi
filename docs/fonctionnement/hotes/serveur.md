---
title: "Le serveur"
description: "Le serveur nwasabi : site embarqué dans le binaire, routes /__nwasabi/, fichiers relus à chaque requête, dossier des sources, pont GDB et garde-fous."
---

# Le serveur

`src/server/main.rs` (binaire `nwasabi`, fonctionnalité `serve`) est un petit serveur **axum**. L'émulation s'exécute dans le navigateur, pas dans le binaire : il **sert la page** et fait le lien avec le disque et le réseau.

## Un seul binaire

`build.rs` construit le site web en trois étapes dans `OUT_DIR` (jamais dans l'arbre source) : (1) s'assurer que la cible `wasm32-unknown-unknown` est installée (`rustup target add`), (2) compiler le module WebAssembly avec un `cargo` imbriqué, (3) construire avec Node
(`npm ci`, `@nwasabi/js`, puis l'interface Vue). `rust-embed` (dossier `NWASABI_WEB_DIST`) embarque le résultat dans le binaire. `NWASABI_NO_WEB=1` saute tout cela : le serveur n'a alors qu'une page qui explique comment construire l'interface.

## Routes

| Route | Rôle |
|---|---|
| `/` et les fichiers du site | la page, depuis le binaire |
| `/__nwasabi/health` | vérification de vie |
| `/__nwasabi/preload` | l'application `--nwa` (que la page charge au démarrage) |
| `/__nwasabi/version` | **empreintes** des fichiers donnés (`nwa`, `data`, `nmk`, `nfs`, `flash`, `nds`) : la page les interroge pour **recharger** quand un fichier change |
| `/__nwasabi/nmk`, `nfs`, `flash`, `nds` | les fichiers `--nmk`, `--nfs`, `--flash`, `--nds`, tels quels (404 sans eux) |
| `/__nwasabi/source?name=…` | texte d'un fichier source de `--src` |
| `/__nwasabi/gdb`, `/gdb/next`, `/gdb/answer`, `/gdb/detach` | pont GDB ([GDB](../debogage/gdb.md#le-pont-avec-la-page-bridge)) |

## Garde-fous

- **Aucun chemin de requête n'est jamais lu.** Les seuls fichiers lus sont ceux donnés au démarrage, **relus à chaque requête** pour que la page puisse les recharger ; les sources de `--src` sont choisies par la **fin de leur chemin** parmi la liste des fichiers du dossier
  (`source_files`) : le plus long accord gagne, sinon le même nom de fichier.
- Les fichiers `.nmk`, `.nfs`, `.nfd`, `.nds` sont **vérifiés au démarrage** : une erreur de format empêche de démarrer, avec l'endroit de l'erreur.
- Le port GDB n'écoute que sur `--host` (la machine seule par défaut). `--gdb-headless` valide le `.nwa` avant d'écouter.
- Le modèle (`--model`) et le micrologiciel (`--firmware`) sont passés à chaque session GDB sans page.

## Options

Voir [Le serveur](../../guide/serveur.md) (guide) : `--nwa`, `--data`, `--nmk`, `--nfs`, `--flash`, `--nds`, `--src`, `--model`, `--firmware`, `--host`, `--port`, `--gdb`, `--gdb-headless`.
