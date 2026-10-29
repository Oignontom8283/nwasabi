---
title: "Formats de fichiers"
description: "Les formats JSON de Nwasabi : .nmk, .nfs, .nfd, .nds et .nrp, leur versionnement et la façon de les charger."
---

# Formats de fichiers de Nwasabi

Nwasabi définit quelques formats de fichiers, tous en **JSON** (les contenus binaires y sont encodés en base64),
chacun avec un champ de version obligatoire. Les fichiers invalides sont refusés avec l'endroit de l'erreur.

| Extension | Nom | Contenu |
|---|---|---|
| [`.nmk`](./nmk.md) | Nwasabi Mapping Key | liaisons du clavier de l'ordinateur et des gestes tactiles vers les touches de la calculatrice |
| [`.nfs`](./nfs.md) | Nwasabi File System | les fichiers de la calculatrice (scripts Python, réglages…) |
| [`.nfd`](./nfd.md) | Nwasabi Flash Data | les données auxiliaires en flash d'une application, à des adresses de la flash externe |
| [`.nds`](./nds.md) | Nwasabi Dump State | l'état d'une émulation en pause, à reprendre plus tard |
| [`.nrp`](./nrp.md) | Nwasabi RePlay | l'enregistrement d'une exécution (touches, temps), à rejouer et à vérifier |

Chaque format peut être chargé par un paramètre de l'adresse de l'émulateur web (`?nmk=`, `?nfs=`, `?flash=`, `?nds=`),
une option du serveur `nwasabi` et de l'outil `nwasabi-cli`, comme `?nwa=` pour l'application (le `.nrp`, lui, se rejoue avec
`nwasabi-cli replay`).

## Dans le code

| Format | Lecture et écriture | Spécification |
|---|---|---|
| `.nmk` | [`keymap/`](code:src/keymap/mod.rs) (Rust), [`keymap.ts`](code:packages/js/src/keymap.ts) (JavaScript) | [nmk](./nmk.md) |
| `.nfs` | [`formats/nfs.rs`](code:src/formats/nfs.rs) | [nfs](./nfs.md) |
| `.nfd` | [`formats/nfd.rs`](code:src/formats/nfd.rs) | [nfd](./nfd.md) |
| `.nds` | [`formats/nds.rs`](code:src/formats/nds.rs), [`kernel/state.rs`](code:src/kernel/state.rs) | [nds](./nds.md) |
| `.nrp` | [`formats/nrp.rs`](code:src/formats/nrp.rs) | [nrp](./nrp.md) |

Les tests sont dans [`tests/formats.rs`](code:tests/formats.rs) et [`tests/fixtures.rs`](code:tests/fixtures.rs) (un fichier de chaque format, fabriqué par `nwasabi-cli`).

## Conventions de ces formats

- tout est **JSON**, avec un champ de version obligatoire ;
- un fichier invalide est **refusé**, avec l'endroit de l'erreur (`keyboard.KeyW`, `touch.rules[1].gesture`) ; pour `.nmk`, même un champ inconnu l'est (une faute de frappe ne doit pas passer en silence).

**Modifier un format** : changer sa version si l'ancien fichier ne se relit pas à l'identique, adapter l'écrivain **et** le lecteur dans le même fichier, mettre à jour la spécification de cette section, et ajouter le fichier d'exemple aux `fixtures`.
