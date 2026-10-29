---
title: "Symboles et sections utiles"
description: "Résolution des symboles entre le runtime, l'application et les données externes, force des définitions, symboles du script de link, variables communes et suppression des sections inutiles."
---

# Symboles et sections utiles

## Table des définitions

Les définitions globales de tous les objets sont réunies dans une table (`linker.rs`). Leur **force** est ordonnée : un symbole **fort** l'emporte sur un **faible**, qui l'emporte sur un **commun** (`COMMON`).
À force égale, **la première définition gagne** (équivalent de `--allow-multiple-definition`) : l'ordre des objets décide, runtime en premier.

Un symbole utilisé par une relocation se résout (`Resolved`) en :

| Résolution | Cas |
|---|---|
| défini dans un objet | celui qui l'utilise ou un autre |
| défini par l'éditeur de liens | les neuf symboles du « script de link » |
| faible, défini nulle part | vaut **0** (un `bl` vers lui devient `nop.w`) |
| non défini | **erreur** `LinkError::UndefinedSymbol` si quelqu'un l'utilise vraiment |

Les symboles que l'éditeur de liens définit lui-même (`runtime::linker_symbols`) :

| Symbole | Valeur |
|---|---|
| `_userland_trampoline_address` | adresse de la table de fonctions du userland (`LayoutParams::trampoline_address`) |
| `_heap_start`, `_heap_end` | tas : après `.bss` et les communs, jusqu'à la fin de la RAM réservée |
| `_data_section_start_flash`, `_data_section_start_ram`, `_data_section_end_ram` | copie initiale de `.data` en flash, et son emplacement d'exécution |
| `_bss_section_start_ram`, `_bss_section_end_ram` | `.bss` |
| `_eadk_app_end` | fin du code et des constantes |

Plus `_eadk_external_data_start` et `_eadk_external_data_size`, définis par l'objet des données externes.

## Sections utiles

L'étape « `gc-sections` » (`Reachability`) part des **racines** : `_start` et les trois sections que l'en-tête d'application lit (niveau d'API, nom, icône). Elle marque tout ce que ces sections référencent par une relocation,
récursivement. Les relocations de type `R_ARM_NONE` **comptent comme des dépendances** (c'est ainsi qu'une table d'exceptions retient sa routine). Les tables de déroulement `.ARM.exidx` sont gardées avec le code qu'elles décrivent
(elles désignent des routines `__aeabi_unwind_cpp_pr0`… que l'éditeur de liens officiel conserve aussi) mais **ne sont pas placées** : personne ne les lit sur Epsilon. `.ARM.extab` est ignorée de même.

## Variables communes

Les symboles `SHN_COMMON` sont réunis **par nom** : taille maximale, alignement maximal. Seules celles que l'application utilise vraiment (`Reachability::commons`) sont placées. Le script de link officiel ne prévoit pas `COMMON` : ces
variables sont placées **après `.bss`**, donc **hors de la zone que `_start` met à zéro** — comportement observé de l'outil officiel, reproduit tel quel : une variable commune non initialisée n'est pas garantie nulle.

## Sections spéciales

`TLS` (variables locales aux threads) est refusé (`LinkError::Unsupported`). Une section qui n'est pas allouée en mémoire (`!SHF_ALLOC`) n'est pas placée ; les `.debug_*` suivent un chemin à part
([Application et export ELF](./application.md#informations-de-debogage)).
