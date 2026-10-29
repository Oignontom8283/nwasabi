---
title: "Implémentation des appels système"
description: "Convention d'appel, table des svc, gestion du tas et de la sortie standard, programmation de la flash et choix faits quand un numéro est inconnu."
---

# Implémentation des appels système

La liste des numéros est dans la [Référence](../../reference/appels-systeme.md). Cette page décrit comment `kernel/svc.rs` les traite.

## Convention

Les arguments sont dans `r0`–`r3` puis sur la pile, le résultat dans `r0` (`r0:r1` pour 64 bits, `s0` pour un flottant : `eadk_battery_voltage`). Le noyau lit `[r0, r1, r2, r3]` **une fois**
à l'entrée. Une erreur d'accès mémoire à un pointeur de l'application (`BusFault` dans le noyau) devient `KernelStop::BusFault` : la faute est rapportée sur le `svc`, juste avant `pc`.

Si la touche `Home` a été demandée (`exit_requested`), **n'importe quel** `svc` s'arrête d'abord sur `HomePressed` : une application qui n'appelle jamais `event_get` quitte donc au prochain appel système,
comme sur l'appareil.

## Table de dispatch

Le grand `match` de `Kernel::handle` regroupe :

- les **appels de l'EADK** que le firmware officiel fournit aux applications : luminosité (1, 2), écran (18, 19, 20, 21), événements (22, 23, 24), clavier (33, 34), hasard (45), temps (48, 49, 50) ;
- les **appels optionnels** (batterie 3, 4, 5 ; USB 52) : ils ne sont **liés** que si `LinkOptions::extended_runtime` est vrai (voir [Éditeur de liens](../edition-de-liens/runtime.md)) ;
- les appels que le **noyau d'Epsilon** expose à des applications privilégiées comme KhiCAS : état de shift et d'alpha (26, 28), effacement d'un secteur (30), écriture de la flash (32) ;
- des appels d'**information** (0 : niveau d'autorisation, 29 : adresse de la chaîne FCC, 39 : version du PCB, 47 : numéro de série, 56 : drapeaux de compilation, 57 : CRC du bootloader, 59 : type du dernier
  redémarrage, 55 : compteur de répétition) ;
- les **appels privés de Nwasabi** (`0x80` `drawString`, `0x81` `_sbrk`, `0x82` `_write`, `0x89` fin) ;
- les entrées vides de la table (`6..=17`, `27`, `31`, `35..=38`, `42..=44`, `51`, `53..=54`, `58`) répondent `0` ; `25`, `40`, `41` ne touchent à rien. Les numéros hors de la table sont **ignorés** (comme sur
  l'appareil), ou arrêtent l'application avec `strict_svc`.

## Cas qui méritent une explication

- **`eadk_event_get`** (23) : le pointeur en `r0` désigne le délai (millisecondes, négatif : sans fin) et reçoit en retour le délai restant. Le premier appel mémorise le début de l'attente (`EventWait`) ; chaque reprise
  recalcule le temps écoulé, prend le plus ancien événement en file ou, à défaut, l'événement de **répétition** de la touche enfoncée (voir [Clavier](./clavier.md)), et rend `EVENT_NONE` quand le délai est écoulé.
  Le noyau reprend au plus tard quand la prochaine répétition est due (`repeat_remaining_ms`).
- **Luminosité** (1, 2) : sur 4 bits, par pas de 16 : écrire 100 puis relire donne 96.
- **`eadk_display_wait_for_vblank`** (21) : incrémente `frames`, rend 1 et s'arrête sur `Frame`. Avec l'horloge virtuelle, le temps saute à la prochaine multiple de `FRAME_US`.
- **`drawString` privé `0x80`** : lit la chaîne C (au plus `MAX_STRING = 4096` octets) ; `r1` vaut `x | y << 16`, `r2 & 0xFF` choisit la grande police, `r3` est la couleur, le fond est lu en haut de la pile. Une application
  appelle `eadk_display_draw_string` par la **table de fonctions du userland** (voir [Appareil](../memoire/appareil.md#les-structures-fabriquees-structures-device-fabric-rs)) : l'éditeur de liens
  y lit l'adresse, qui est le stub `svc #0x80 ; bx lr`.
- **`_sbrk`** (`0x81`) : déplace `brk` dans `[heap.0, heap.1]` ; hors de ces bornes, rend `0xFFFFFFFF`. Le tas est celui que l'éditeur de liens réserve (voir [Édition de liens](../edition-de-liens/placement.md)).
- **`_write`** (`0x82`) : au plus `MAX_WRITE = 65 536` octets, ajoutés à `stdout` (1 Mio au plus) ; avec `stdout_to_screen`, dessinés aussi à `(0, 0)` en petite police, noir sur blanc.
- **Programmation de la flash** (30, 32) : copiée de ce que fait `Ion::Device::Flash`. `erase_sector(n)` : secteurs 0 à 7 de 4 Kio (`0x9000_0000` à `0x9000_8000`), 8 de 32 Kio (`0x9000_8000`), puis 9 à 135 de 64 Kio ; elle refuse
  (rend 0) tout secteur qui commence avant `flash_protected_end` (le firmware et les applications installées) et passe par `Bus::privileged_write` (écrire `0xFF`). `write_flash(destination, source, longueur)` ne peut que
  **passer des bits à 0** (`ancien & nouveau`), refuse au-delà de 1 Mio ou hors de la flash externe, et refuse en dessous de `flash_protected_end`.
- **Numéro de série** (47) : copie 16 caractères plus un zéro dans le tampon de `r0`.
- **Texte d'événement** (22) : copie comme `strlcpy`, rend la longueur du texte entier (0 sans texte ou tampon inutilisable) ; voir [Clavier](./clavier.md).

## Pourquoi `Handled` presque partout

Tant que le noyau traite l'appel lui-même, l'exécution reste dans `Cpu::run` : seul un `KernelStop` interrompt. Le noyau n'empile jamais de contexte d'exception : un `svc` est pour lui un appel de fonction.
