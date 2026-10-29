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
