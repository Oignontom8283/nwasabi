---
title: "Placement en flash et en RAM"
description: "L'ordre du script de link reproduit par Nwasabi, le calcul des alignements, la copie initiale de .data, le placement de .bss et des communs, le tas et les contrôles de capacité."
---

# Placement en flash et en RAM

`linker.rs::layout` reproduit le **script de link** de l'outil de flashage officiel : même ordre, mêmes alignements, donc mêmes adresses.

## Classes et rang

Chaque section vivante reçoit une **classe** (`Class`) et un **rang** (`classify`, `named_class`) :

| Classe | Sections | Rang |
|---|---|---|
| `ApiLevel`, `Name`, `Icon` | `.rodata.eadk_api_level`, `.rodata.eadk_app_name`, `.rodata.eadk_app_icon` | 0 |
| `Text` | `.text` (rang 0), `.text.*` (1), autres sections exécutables (2, « orphelines ») | |
| `Rodata` | `.rodata` (0), `.rodata.*` (1), autres sections allouées en lecture seule (2) | |
| `Data` | `.data` (0), `.data.*` (1), autres sections allouées et écrivables (2) | |
| `Bss` | `.bss` (0), `.bss.*` (1), autres sections sans contenu (2) | |

Dans une classe, on trie par rang puis par ordre d'apparition (objet, section) : d'abord les sections nommées exactement `.text`, puis `.text.*`, puis les autres sections de même nature.

## Ordre en flash

```text
flash_start
  [niveau d'API : 4 octets, juste après le magic de l'en-tête]
  [en-tête de 32 octets]
  noms          (.rodata.eadk_app_name)
  icônes        (.rodata.eadk_app_icon)
  .text, .text.*, orphelines exécutables
  .rodata, .rodata.*, orphelines
  ← app_end (valeur de _eadk_app_end, écrite dans l'en-tête)
  copie initiale de .data (SANS alignement : `_start` la copie octet par octet)
```

`place(cursor, ids, min_align)` aligne l'ensemble sur l'alignement le plus exigeant de ses membres (au moins `min_align`) puis chaque section sur le sien.

## RAM

```text
ram_start
  .data  (aligné sur l'alignement maximal du groupe, au moins 4)
  .bss   (idem)  ← _bss_section_start_ram … _bss_section_end_ram
  variables COMMON  (après .bss)
  tas    ← _heap_start … _heap_end = fin de la RAM réservée aux applications
```

La **copie de `.data`** vit en flash juste après les constantes, à `data_flash`, et s'exécute en RAM à `data_ram` ; l'adresse de chaque section `.data` en flash est `data_flash + (adresse_ram − data_ram)`.

## Contrôles de capacité

`image_end` (la fin la plus haute en flash, copie de `.data` comprise) doit tenir dans `[flash_start, flash_end)` : sinon `LinkError::FlashOverflow { needed, available }`. De même, `commons_end` dans la RAM
(`RamOverflow`). Le tas n'est jamais vérifié : c'est ce qui reste.

## Cas particulier : la taille de l'en-tête

La taille que l'en-tête écrit (`app_end − flash_start`) **ne compte pas** la copie de `.data`. L'outil de flashage, lui, place l'application suivante d'après la taille **réelle** de l'image. Si les deux tailles tombent dans des
secteurs de 64 Kio différents, le système cherche l'application suivante au mauvais endroit et la « perd ». Ce défaut du format officiel est **reproduit tel quel**, et signalé : `LinkWarning::AppSizeMismatch` (voir
[`device::list_apps`](../memoire/appareil.md#fabriquer-installer-relire)).
