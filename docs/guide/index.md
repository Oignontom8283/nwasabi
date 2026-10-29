---
title: "Premiers pas"
description: "Choisir comment utiliser Nwasabi (émulateur web, serveur, ligne de commande ou bibliothèque JavaScript), essayer une application en deux minutes et installer ou compiler le projet."
---

# Premiers pas

Nwasabi exécute les **applications externes** des calculatrices NumWorks (fichiers `.nwa`) sur un autre appareil : un
processeur ARM Cortex-M7 est émulé en Rust, et le système de la calculatrice (écran, clavier, temps, fichiers) est remplacé par une
version simplifiée. Rien n'est téléchargé de chez NumWorks : il ne faut que votre `.nwa`.

## Quatre façons de s'en servir

| Je veux… | J'utilise | Voir |
|---|---|---|
| essayer une application dans mon navigateur, sans rien installer | l'**émulateur web** (`/emulator/` du site) | [L'émulateur web](./emulateur-web.md) |
| l'essayer en local, avec rechargement automatique quand je recompile | le **serveur** `nwasabi` | [Le serveur](./serveur.md) |
| lancer des applications dans des scripts ou des tests, sans navigateur | l'outil `nwasabi-cli` | [La ligne de commande](./cli.md) |
| intégrer l'émulateur dans une page ou un programme Node.js | la bibliothèque `@nwasabi/js` | [La bibliothèque JavaScript](../reference/bibliotheque-js.md) |

Les trois premières se servent du même cœur ; ce qu'elles font se pilote par les mêmes commandes
([le protocole de session](../reference/protocole-session.md)).

## Essayer une application en deux minutes

1. Ouvrez l'émulateur (`/emulator/`) avec votre fichier `.nwa` ([L'émulateur web](./emulateur-web.md)).
2. Les touches se lient par un fichier [`.nmk`](../reference/formats/nmk.md).
3. Quelque chose ne va pas ? Voir [Déboguer dans l'émulateur](./deboguer-emulateur.md) : points d'arrêt, observation, remontée dans le temps, GDB.

Une application peut aussi s'ouvrir par l'adresse : `…/emulator/?nwa=https://exemple.org/jeu.nwa`.

## Des applications qui demandent plus qu'un `.nwa`

Certaines applications lisent, à une adresse de la flash, un programme ou des données flashés à part : ce sont des
[données auxiliaires en flash](../reference/donnees-flash.md). Exemple : [KhiCAS](./khicas.md).

## Installer le serveur

```sh
cargo install nwasabi      # installe la commande `nwasabi` (le serveur, avec l'émulateur)
nwasabi --nwa jeu.nwa
cargo install nwasabi --features cli   # ajoute `nwasabi-cli`
```

La compilation construit l'interface web : Node.js (avec npm) et la cible `wasm32-unknown-unknown` de Rust sont nécessaires
(`NWASABI_NO_WEB=1` les évite, au prix d'une page vide à la place de l'émulateur).

## Compiler le projet

Il faut Rust (édition 2024) et, pour l'interface web, Node.js. Les recettes `just` font le reste :

```sh
just build-server     # le serveur web (embarque l'émulateur)
just build-cli        # l'outil en ligne de commande
just build-wasm       # le module WebAssembly
just serve --nwa jeu.nwa --port 8080
```

`just --list` donne toutes les recettes (tests, documentation, paquets npm…).

## Ce qu'il faut savoir avant de s'y fier

L'émulateur reproduit **ce que voit une application**, pas toute la calculatrice. Il est utile pour développer, tester et
déboguer ; il ne remplace pas un essai sur le vrai appareil avant de publier. [Compatibilité et limites](../reference/compatibilite.md)
dit précisément ce qui est reproduit et ce qui ne l'est pas.
