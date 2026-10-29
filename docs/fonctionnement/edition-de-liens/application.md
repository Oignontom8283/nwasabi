---
title: "Application et export ELF"
description: "L'en-tête d'application de 32 octets, l'installation en flash, les informations de débogage DWARF conservées avec leurs relocations, et l'ELF exécutable donné à GDB."
---

# Application et export ELF

## L'en-tête d'application

Les 32 premiers octets de l'image (`structures::AppHeader`, `APP_HEADER_SIZE`) :

| Décalage | Mot | Contenu |
|---|---|---|
| 0 | `header` | magic `0xDEC0BEBA` |
| 4 | `api_level` | `.rodata.eadk_api_level` |
| 8 | `name_offset` | position du nom (chaîne terminée par un zéro), depuis le début de l'application |
| 12 | `icon_size` | taille de l'icône |
| 16 | `icon_offset` | position de l'icône |
| 20 | `entry_offset` | position de `_start`, **sans le bit Thumb** (« décalage pair » : le script de link GNU lit l'adresse sans lui, le système l'ajoute) |
| 24 | `size` | taille selon l'en-tête : `app_end − flash_start` (sans la copie de `.data`) |
| 28 | `footer` | magic `0xDEC0BEBA` |

`AppHeader::is_valid` exige les deux magics. Les décalages sont **relatifs au début de l'application**. L'icône est une image au format NWI, conservée brute.

## Installation

`DeviceImage::install` écrit l'image à la suite des applications déjà installées, la complète de `0xFF` jusqu'à la fin du secteur de 64 Kio, et remet à jour la position libre suivante. L'application doit avoir été liée avec le
`LayoutParams` de `app_layout()` : `flash_start` doit correspondre à l'adresse libre (sinon `DeviceError::WrongPlace`). Une application qui change de modèle ou de disposition de firmware est donc **liée de nouveau** (c'est ce que fait
`Emulator::install_nwa` quand le profil change).

## Informations de débogage

Les sections `.debug_*` de l'application ne vont pas dans l'image : elles n'ont **pas d'adresse**. `link/linker.rs` les rassemble dans `LinkedApp::debug` (`DebugSection { name, data }`) **avec leurs relocations déjà appliquées** :

- une relocation qui vise du **code** reçoit son adresse définitive ;
- une qui vise une **autre section de débogage** vaut un décalage dans cette section ;
- une qui vise du code **supprimé** à l'édition de liens vaut 0 : le lecteur DWARF ignore alors la fonction ;
- le **bit Thumb** des fonctions n'est pas ajouté : DWARF donne des adresses paires.

Le lecteur ([Informations de débogage](../debogage/dwarf.md)) travaille ensuite sur des octets déjà « liés » : il n'a pas à connaître les relocations.

## Export ELF pour GDB

`LinkedApp::to_elf()` (`link/export.rs`, commande `nwasabi-cli link --elf`, bouton « Télécharger l'ELF » de la vue GDB, commande de session `export_elf`) écrit l'application liée au format **ELF exécutable** que GDB attend de `file app.elf` : une
section par section placée (`.text`, `.rodata`, `.data`, `.bss`, avec leurs adresses d'exécution, drapeaux `ALLOC`/`WRITE`/`EXECINSTR`), la **table des symboles**, et les sections `.debug_*` (sans adresse). GDB y lit les symboles et le DWARF aux
adresses définitives ; **le code et les données d'exécution viennent de Nwasabi** par la connexion `target remote`.

Les adresses dépendent du modèle et du micrologiciel (disposition `24` ou `26.4.0`) : un ELF produit pour une disposition ne vaut pas pour l'autre.
