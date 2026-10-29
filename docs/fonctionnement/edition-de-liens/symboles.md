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

## Dans le code

| Où | Quoi |
|---|---|
| [`Obj`](code:src/link/linker.rs#Obj) | un objet en entrée (origine + ELF) |
| [`resolve`](code:src/link/linker.rs#resolve), [`Resolved`](code:src/link/linker.rs#Resolved) | résolution d'un symbole |
| [`rank`](code:src/link/linker.rs#rank), [`build_globals`](code:src/link/linker.rs#build_globals) | force fort / faible / commun, et table des définitions |
| [`Reachability`](code:src/link/linker.rs#Reachability) | les sections utiles et les communs utilisés |
| [`LINKER_SYMBOLS`](code:src/link/linker.rs#LINKER_SYMBOLS), [`linker_symbols`](code:src/link/runtime.rs#linker_symbols) | les symboles du « script de link » |

## Pourquoi « la première définition gagne »

Le runtime est lié **avant** l'application, comme `eadk.o` avant l'application dans l'outil officiel, et l'éditeur de liens officiel est lancé avec l'équivalent de `--allow-multiple-definition`. Reproduire ce comportement, y compris pour les cas où il peut surprendre (une application qui redéfinit `eadk_random` utilise celle du runtime), est ce qui rend les images **identiques**.

## Pour modifier

- **Changer l'ordre des objets** : [`link`](code:src/link/mod.rs#link) construit le vecteur ; l'ordre change **quelle définition gagne**.
- **Ajouter un symbole du script de link** : [`linker_symbols`](code:src/link/runtime.rs#linker_symbols), [`LINKER_SYMBOLS`](code:src/link/linker.rs#LINKER_SYMBOLS), et sa valeur dans [`layout`](code:src/link/linker.rs#layout).
- **Symptôme « symbole non défini » sur une application qui marche sur l'appareil** : regarder si la fonction manque au runtime ([`EADK_SVC`](code:src/link/runtime.rs#EADK_SVC)) ou si elle est **faible** et non définie (valeur 0, branchement remplacé par `nop.w`).
