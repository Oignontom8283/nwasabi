---
title: "Introduction à l'émulation"
description: "Pour qui sait programmer mais n'a jamais touché à un émulateur : ce qu'est émuler un processeur, ce que contient un programme compilé, et comment Nwasabi s'y prend. Quatre pages à lire dans l'ordre."
---

# Introduction à l'émulation

::: tip En bref
Un programme compilé est une **suite de nombres** qu'un processeur sait lire. **Émuler**, c'est écrire un autre programme qui lit ces mêmes nombres et fait exactement ce que ferait le vrai processeur. Rien de magique : une grosse boucle et beaucoup de cas particuliers.
:::

Ces quatre pages s'adressent à quelqu'un qui **sait programmer** : variables, fonctions, pointeurs, opérations sur les bits, `if` et boucles. Elles n'ont pas besoin que vous sachiez ce qu'est un registre, ce que fait un éditeur de liens ou pourquoi on parle de « Thumb-2 ». Le reste de la section « Fonctionnement » suppose ce vocabulaire ; ici, on le construit.

| Page | Question à laquelle elle répond |
|---|---|
| [Un processeur, vu par un programmeur](./processeur.md) | Que fait un processeur, concrètement ? Qu'est-ce qu'un registre, une instruction, un saut ? |
| [La mémoire et les adresses](./memoire.md) | Qu'est-ce qu'une adresse ? Pourquoi la mémoire a-t-elle des « zones » ? Qu'est-ce que l'ordre des octets ? |
| [Du code source au programme qui tourne](./du-source-au-programme.md) | Que contient un `.nwa` ? Pourquoi faut-il le « lier » avant de l'exécuter ? |
| [Émuler : la boucle et ses pièges](./emuler.md) | Comment écrit-on un émulateur ? Qu'est-ce qui rend cela difficile, et que choisit Nwasabi ? |

## Le contexte en trois lignes

- Une calculatrice **NumWorks** exécute un système, **Epsilon**, et peut installer des **applications externes** : des fichiers `.nwa`, écrits en C, C++ ou Rust.
- Son processeur est un **ARM Cortex-M7**, dont la famille est la même que dans beaucoup de microcontrôleurs.
- **Nwasabi** exécute ces applications **sans la calculatrice et sans Epsilon**, dans un navigateur ou en ligne de commande.

Dès que vous avez lu ces pages, la [vue d'ensemble](../index.md) de la section devrait se lire sans effort.
