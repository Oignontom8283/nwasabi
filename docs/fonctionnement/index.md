---
title: "Fonctionnement interne"
description: "Architecture de Nwasabi pour lecteurs experts : processeur Cortex-M7, mémoire, noyau simulé, éditeur de liens, débogage, hôtes et vérification, avec les invariants et les choix de conception."
---

# Fonctionnement interne

::: tip En bref
Nwasabi est un **faux appareil** assez bon pour tromper une application. Le processeur est copié instruction par instruction ; tout ce qui vient normalement du système de la calculatrice (écran, clavier, temps, fichiers) est joué par un petit « noyau » maison. Cette page est la carte : chaque case a sa page.
:::


Cette section décrit **comment** Nwasabi émule une application NumWorks, et **pourquoi** chaque morceau est fait ainsi. Elle s'adresse à qui
veut lire, vérifier ou prolonger le code : les pages donnent les structures, les constantes, les invariants et les cas limites, avec le nom
des modules. Pour s'en servir, voir plutôt le [Guide](../guide/index.md) ; pour les formats et les numéros d'appels, la [Référence](../reference/compatibilite.md).

::: info Nouveau dans l'émulation ?
Si les mots « registre », « éditeur de liens » ou « svc » ne vous disent rien, commencez par l'[Introduction à l'émulation](./introduction/index.md) : quatre courtes pages pour quelqu'un qui sait programmer.
:::

## Ce qui est émulé, et ce qui ne l'est pas

Nwasabi n'exécute pas Epsilon, le système de la calculatrice. Il exécute **l'application seule**, sur un processeur ARM Cortex-M7 émulé, et
remplace ce que l'application voit du système par un noyau simulé :

- l'application est un objet ELF relogeable (`.nwa`) ; un **éditeur de liens** maison le place en mémoire comme l'outil de flashage officiel ;
- le **processeur** exécute les instructions Thumb-2 (ARMv7E-M, DSP, FPv5-D16) ;
- une **mémoire** à zones, avec droits et suivi des accès, reproduit la carte de la puce ;
- un **appareil** fabrique en mémoire ce qu'Epsilon y laisserait (en-têtes, tables du userland, système de fichiers vide) ;
- chaque `svc` de l'application tombe dans le **noyau simulé**, qui rend l'écran, le clavier, le temps et les fichiers ;
- le tout est piloté par un **émulateur** coopératif, qui rend la main à l'hôte à chaque image, attente ou arrêt.

## Les couches

```mermaid
flowchart TB
  subgraph hotes["Hôtes : ils donnent le temps, les touches, le disque, l'écran"]
    web["Page web<br/>(dans un Worker)"]
    srv["Serveur nwasabi"]
    cli["nwasabi-cli"]
    js["Bibliothèque @nwasabi/js"]
  end
  gdbs["Serveur GDB"]
  session["Session<br/>protocole JSON : l'unique voie de contrôle"]
  subgraph coeur["Le cœur (pur : ni fil, ni horloge, ni fichier)"]
    emu["Emulator<br/>exécution coopérative"]
    dbg["debuginfo<br/>DWARF, expressions"]
    kern["Kernel<br/>appels système, écran, clavier, temps"]
    dev["DeviceImage<br/>profil, structures de l'OS, applications"]
    cpu["Cpu<br/>Thumb-2, DSP, FPU"]
    mem["Memory<br/>zones, droits, suivi, observation"]
    lnk["link + elf<br/>édition de liens"]
  end
  web --> session
  js --> session
  cli --> session
  srv --> gdbs
  gdbs --> session
  session --> emu
  session --> dbg
  emu --> cpu
  emu --> kern
  emu --> dev
  emu --> lnk
  cpu -. "Bus" .-> mem
  cpu -. "Hooks (svc)" .-> kern
  dev --> mem
```

Le trait `cpu::Bus` (la mémoire) et le trait `cpu::Hooks` (les `svc`) sont les seules coutures entre le processeur et le reste : le module
`cpu` n'utilise rien d'autre dans la caisse, ce que vérifie `tests/cpu_independence.rs`.

## Principes qui tiennent tout

| Principe | Conséquence |
|---|---|
| **Le cœur est pur** : pas de fil, pas d'horloge système, pas d'entrée-sortie | l'hôte donne les touches et fait passer le temps ; le même module tourne en natif et en WebAssembly |
| **Déterminisme** : mêmes entrées, même exécution | le temps est virtuel par défaut ; l'historique (remonter le temps), les enregistrements `.nrp` et les tests de référence en dépendent |
| **Une seule voie de contrôle** : la session | le serveur GDB, la page, la ligne de commande et la bibliothèque JS ne font que transporter des requêtes JSON ; il n'y a pas de second chemin d'exécution |
| **On lit la mémoire, pas la configuration** | l'inspection, le système de fichiers et les structures de l'OS sont relus à chaque fois dans la mémoire émulée : si l'application les abîme, on le voit |
| **Rien du firmware officiel n'est embarqué** | les valeurs (en-têtes, tables, textes d'événements) sont des relevés de comportement, refaits ; la police est originale |
| **Une faute arrête, elle ne s'exécute pas** | pas de gestionnaires d'exception : la cause est notée dans les registres de faute simulés et l'application s'arrête |

## Carte des modules

| Module (`src/`) | Rôle | Page |
|---|---|---|
| `cpu/` | décodage, exécution, FPU, exceptions, désassembleur | [Processeur](./processeur/index.md) |
| `memory/` | zones, pages, droits, suivi, points d'observation, instantanés | [Mémoire](./memoire/index.md) |
| `profile/`, `device/`, `structures/`, `inspect/`, `scb.rs`, `settings.rs` | appareil : profils de firmware, structures de l'OS, registres de faute | [Appareil](./memoire/appareil.md) |
| `kernel/` | `svc`, écran, clavier, événements, temps, police | [Noyau simulé](./noyau/index.md) |
| `storage/` | système de fichiers de la calculatrice | [Fichiers](./noyau/fichiers.md) |
| `elf/`, `link/` | lecture ELF, édition de liens, export ELF | [Édition de liens](./edition-de-liens/index.md) |
| `emulator.rs` | exécution coopérative, points d'arrêt, profil | [Exécution](./execution/index.md) |
| `debuginfo/`, `symbols.rs` | DWARF, pile, expressions, noms lisibles | [Débogage](./debogage/index.md) |
| `session/`, `gdb/` | protocole JSON, historique, serveur GDB | [Session](./debogage/session.md), [GDB](./debogage/gdb.md) |
| `formats/`, `keymap/`, `json.rs` | formats `.nds`, `.nfd`, `.nfs`, `.nrp`, `.nmk` | [Formats](../reference/formats/index.md) |
| `wasm.rs`, `server/`, `cli/`, `packages/`, `web/` | hôtes | [Hôtes](./hotes/index.md) |

## Par où commencer selon ce que l'on veut faire

| Si je veux… | Je commence par… |
|---|---|
| comprendre ce qu'est un émulateur | [Introduction à l'émulation](./introduction/index.md) |
| comprendre l'architecture globale | cette page, puis [Exécution](./execution/index.md) |
| ajouter une instruction ARM | [Processeur](./processeur/index.md) → [Décodage](./processeur/decodage.md) → [Exécution](./processeur/execution.md) → [Vérification](./verification/index.md) |
| modifier un appel système | [Noyau simulé](./noyau/index.md) → [Appels système](./noyau/appels-systeme.md) → [Vérification](./verification/index.md) |
| corriger un problème de chargement d'application | [Édition de liens](./edition-de-liens/index.md) → [Relocations](./edition-de-liens/relocations.md) → [Placement](./edition-de-liens/placement.md) |
| comprendre le débogueur | [Session](./debogage/session.md) → [DWARF](./debogage/dwarf.md) → [Pile](./debogage/pile.md) → [GDB](./debogage/gdb.md) → [Historique](./debogage/historique.md) |
| modifier l'interface web | [Hôtes](./hotes/index.md) → [Page web](./hotes/web.md) → [Bibliothèque JavaScript](./hotes/bibliotheque-js.md) |

## Lire cette section

Les pages vont du bas vers le haut : [processeur](./processeur/index.md), [mémoire](./memoire/index.md), [noyau](./noyau/index.md),
[éditeur de liens](./edition-de-liens/index.md), [exécution](./execution/index.md), [débogage](./debogage/index.md), [hôtes](./hotes/index.md),
[vérification](./verification/index.md). Chaque page cite les modules et les constantes pour qu'on retrouve le code en une recherche.
