---
title: "Compatibilité et limites"
description: "Ce que Nwasabi reproduit de la calculatrice NumWorks et ce qu'il ne reproduit pas : processeur, système, temps, périphériques, exceptions."
---

# Compatibilité et limites

Ce que l'émulateur reproduit, et ce qu'il ne reproduit pas. L'objectif est qu'une application bien écrite se comporte pareil ici et
sur la calculatrice ; ce n'est pas une reproduction de la puce, et il faut garder en tête les écarts ci-dessous.

## Ce qui est reproduit

- **Le processeur** : un Cortex-M7 (jeu d'instructions Thumb-2, ARMv7E-M), avec les accès mémoire, les indicateurs, les blocs `it`, les
  divisions, les multiplications longues, **toute l'extension DSP** (arithmétique parallèle en huit et seize bits, saturations,
  doubles produits, `usad8`…), les champs de bits et l'unité flottante FPv5 (D0 à D15).
- **Les appels système vus par l'application** : écran, clavier, temps, hasard, luminosité, données externes, tas, sortie texte
  ([la liste](./appels-systeme.md)).
- **La carte mémoire** : flash externe, SRAM, mémoire réservée au noyau, en-têtes et structures du système lus par certaines
  applications ([Modèles et mémoire](./modeles-memoire.md)).
- **Le système de fichiers de la calculatrice** : le tampon de 42 Kio où vivent les scripts Python et les fichiers des
  applications, avec le même format.
- **Les fautes** : une faute d'accès mémoire, une instruction indéfinie ou un accès non aligné arrêtent l'application, et les
  registres de faute du processeur sont remplis comme sur la puce.
- **Le déterminisme** : à entrées égales (touches et temps), l'exécution est identique à chaque fois. C'est ce qui permet de
  [remonter le temps](../guide/deboguer-emulateur.md#remonter-le-temps).

## Ce qui ne l'est pas

### Le système

Le système d'exploitation de la calculatrice (Epsilon) **n'est pas émulé** : il n'est pas fourni, et ses appels sont remplacés par
des fonctions écrites pour Nwasabi. Conséquences :

- les fonctions du système que l'EADK n'expose pas ne sont pas accessibles ;
- les applications qui lisent la mémoire du système (versions, identifiants, positions d'en-têtes) voient des valeurs plausibles,
  qui ne sont pas celles d'un appareil précis ;
- le mode examen cache les applications externes et empêche de les lancer ; seul cet effet est reproduit.

### Le processeur

- **Le temps d'exécution** est un modèle simple : un cycle par instruction. Il n'y a ni cache, ni temps d'accès à la flash, ni
  pipeline. Une application sensible à la vitesse réelle (boucles d'attente chronométrées par le nombre d'instructions) ne se
  comportera pas exactement comme sur la puce. La vitesse affichée peut être celle du modèle (192 ou 540 MHz), une fréquence choisie
  ou le maximum que permet l'ordinateur.
- **Pas d'exceptions utilisables par l'application** : une faute arrête l'exécution au lieu d'appeler un gestionnaire ; seuls les
  registres de faute sont remplis. Il n'y a pas d'interruptions matérielles.
- **Pas de périphériques STM32** (horloges, DMA, bus de communication…) : l'application ne les voit pas.
- **Pas de MPU** : les droits d'accès se limitent à ceux de la carte mémoire (flash en lecture et exécution, mémoire du noyau
  inaccessible).
- **Le jeu d'instructions** est celui d'ARMv7E-M avec FPv5-D16. Il a été contrôlé de deux façons : l'espace des encodages (les 16 bits
  et les 32 bits) a été parcouru en le comparant à des désassembleurs indépendants, et des centaines de programmes d'essai (toutes
  les familles d'instructions, y compris l'extension DSP et la virgule flottante) ont été exécutés par QEMU, dont les résultats
  sont comparés octet pour octet à ceux de Nwasabi. Une instruction qui n'existe pas sur un Cortex-M7 (les instructions de
  coprocesseur, le calcul vectoriel NEON, les registres D16 à D31, les instructions d'ARMv8-A) arrête l'application avec une
  instruction indéfinie, comme sur la puce, sauf que le registre de faute n'en précise pas la cause exacte (coprocesseur absent).

### L'écran, le clavier

- L'écran est de 320 × 240 pixels ; le rétroéclairage n'est qu'un assombrissement de l'image (réglable ou désactivable), pas une
  mesure de luminosité.
- Le clavier est celui de la calculatrice : les touches de l'ordinateur sont liées par un [mapping](./formats/nmk.md) modifiable.
  Les gestes tactiles du mapping (balayages, deux doigts) se traduisent en appuis de touches.

## Modèles et firmwares

Trois modèles (N0110, N0115, N0120) et deux dispositions de firmware : une « 24 » (les appareils relevés, celle que supposent les
applications écrites avec certaines bibliothèques) et la « 26.4.0 » (le système officiel récent). Un modèle ou une version qui n'est
pas dans cette liste n'est pas reproduit. Détails : [Modèles et mémoire](./modeles-memoire.md).

## Si quelque chose diffère de la vraie calculatrice

C'est un cas utile à signaler : d'abord regarder la faute signalée et le journal des appels système pour savoir si l'application demande quelque
chose que l'émulateur ne reproduit pas (un appel système, une instruction, une zone de mémoire). Les limites ci-dessus sont celles
connues ; un écart qui n'y figure pas est probablement un défaut de l'émulateur.
