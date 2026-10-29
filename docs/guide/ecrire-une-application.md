---
title: "Écrire une application pour l'émulateur"
description: "Compiler une application NumWorks (.nwa) en C, C++ ou Rust qui s'exécute dans Nwasabi : points d'entrée, drapeaux de compilation, appels système et limites du système simulé."
---

# Écrire une application qui marche dans l'émulateur

Une application externe NumWorks est un programme C (ou C++, Rust…) compilé pour un Cortex-M7, lié avec l'outil de NumWorks
(`nwlink`) qui produit un fichier **`.nwa`**. Nwasabi lit ce fichier, fait l'édition de liens comme le ferait le système de la
calculatrice, installe l'application en « flash » et l'exécute. Cette page résume ce qui compte quand on veut qu'elle tourne
ici **et** sur le vrai appareil.

## La chaîne de compilation

1. Compiler les sources pour Cortex-M7 (`arm-none-eabi-gcc -mcpu=cortex-m7 -mthumb`, avec l'unité flottante simple précision si
   besoin), sans bibliothèque standard hébergée.
2. Lier avec `nwlink` pour obtenir le `.nwa`.

Les applications de test du dépôt (`tests/apps/`) sont de petits exemples complets, avec leur recette `just build-test-apps`.

## Ce que l'application peut appeler

Les fonctions de l'**EADK** (la bibliothèque d'applications externes) sont des appels système : l'émulateur les reconnaît par leur
nom, comme le ferait l'édition de liens officielle. Sont fournies : l'écran (`eadk_display_push_rect`, `…_push_rect_uniform`,
`…_pull_rect`, `…_wait_for_vblank`), le clavier (`eadk_keyboard_scan`, `eadk_keyboard_pop_state`,
`eadk_event_get`), le temps (`eadk_timing_millis`, `eadk_timing_msleep`, `eadk_timing_usleep`), le hasard (`eadk_random`), la
luminosité (`eadk_backlight_brightness`, `eadk_backlight_set_brightness`) et les données externes (`eadk_external_data`,
`eadk_external_data_size`). Le tableau des numéros est dans [Les appels système](../reference/appels-systeme.md).

Les appels système inconnus arrêtent l'application avec une faute explicite (réglage `strict_svc`).

## Les contraintes à connaître

- **Écran** : 320 × 240 pixels en 16 bits (RGB565). Rien n'apparaît tant que l'application n'a pas poussé de pixels ;
  `eadk_display_wait_for_vblank` rend la main une fois par image (60 images par seconde).
- **Temps** : par défaut, l'émulateur tient un temps *virtuel* déterministe (voir [Compatibilité](../reference/compatibilite.md)) :
  une application qui dort (`eadk_timing_msleep`) ne fait pas attendre l'ordinateur.
- **Mémoire** : la RAM disponible pour l'application, la pile et le tas viennent du modèle émulé . La commande de session `usage` donne
  leur occupation (pile et tas, avec les plus hautes valeurs atteintes). Voir [Modèles et mémoire](../reference/modeles-memoire.md).
- **Accès exclusifs** : sur la vraie calculatrice, `ldrex` et `strex` sur la RAM provoquent une faute (la RAM y est déclarée
  partageable et il n'y a pas de moniteur global). L'émulateur reproduit ce comportement : une application qui s'en sert plante
  ici comme là-bas. Sur la flash, ils sont acceptés.
- **Flottants** : la simple précision est gérée en entier ; la double précision (D0 à D15) l'est pour les opérations courantes.
  Voir les [limites](../reference/compatibilite.md#le-processeur).
- **Sortie texte** : `printf` vers la sortie standard est possible ; le texte apparaît dans la sortie de l'émulateur (et sur la sortie standard avec la ligne de commande).

## Compiler pour le débogage

Pour du C++ ou du Rust, et pour les expressions, voir [Déboguer en C, C++ et Rust](./deboguer-c-cpp-rust.md).

- **`-g`** : le compilateur écrit dans le `.nwa` les informations de débogage (lignes de source, variables, types, tables d'appel).
  Nwasabi les lit : points d'arrêt par ligne, pile exacte, variables, expressions (voir
  [Déboguer le code source](./deboguer-emulateur.md#deboguer-le-code-source)). Elles ne changent pas le code produit ni le fichier flashé
  sur la calculatrice : elles ne sont pas dans l'image.
- **`-O0`** pour la partie que l'on suit pas à pas : en `-O2`, les variables vivent dans des registres, disparaissent, les fonctions
  sont fondues dans leur appelante.
- **`-fdebug-prefix-map=$(pwd)=.`** réécrit le dossier de compilation dans les chemins des sources : le fichier ne contient pas le chemin de
  votre machine, et il est identique d'une machine à l'autre.
- Ne retirez ni les symboles ni les informations de débogage (`strip`) pendant que vous déboguez : sans symboles, seules les
  adresses s'affichent.

Pour **GDB**, `nwasabi-cli link jeu.nwa --elf jeu.elf` écrit l'application liée au format ELF, avec les adresses définitives :
voir [Utiliser GDB](./deboguer-c-cpp-rust.md#gdb).

## Quand l'application plante

L'émulateur s'arrête sur la première faute et explique laquelle : accès à une adresse qui n'existe pas (pointeur nul ou corrompu),
instruction indéfinie (saut dans des données), accès non aligné avec une instruction qui l'interdit, appel système inconnu… Il
donne l'instruction fautive et remplit les registres de faute du Cortex-M7, que GDB lit comme sur une vraie carte :
[Déboguer dans l'émulateur](./deboguer-emulateur.md#registres-de-faute).
