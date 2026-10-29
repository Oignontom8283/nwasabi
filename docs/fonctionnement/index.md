---
title: "Fonctionnement interne"
description: "Architecture de Nwasabi pour lecteurs experts : processeur Cortex-M7, mémoire, noyau simulé, éditeur de liens, débogage, hôtes et vérification, avec les invariants et les choix de conception."
---

# Fonctionnement interne

Cette section décrit **comment** Nwasabi émule une application NumWorks, et **pourquoi** chaque morceau est fait ainsi. Elle s'adresse à qui
veut lire, vérifier ou prolonger le code : les pages donnent les structures, les constantes, les invariants et les cas limites, avec le nom
des modules. Pour s'en servir, voir plutôt le [Guide](../guide/index.md) ; pour les formats et les numéros d'appels, la [Référence](../reference/compatibilite.md).

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
  subgraph hotes[Hôtes]
    web[Page web + Worker]
    srv[Serveur nwasabi]
    cli[nwasabi-cli]
    js[@nwasabi/js]
  end
  session[Session : protocole JSON]
  gdb[Serveur GDB]
  emu[Emulator]
  dbg[debuginfo : DWARF, expressions]
  kern[Kernel : appels système, écran, clavier, temps]
  dev[DeviceImage : profil, structures, applications]
  mem[Memory : zones, droits, suivi, observation]
  cpu[Cpu : Thumb-2, FPU]
  lnk[link + elf : édition de liens]
  web --> session
  js --> session
  cli --> session
  srv --> gdb --> session
  session --> emu
  session --> dbg
  emu --> cpu
  emu --> kern
  emu --> dev
  cpu -. Bus .-> mem
  cpu -. Hooks .-> kern
  dev --> mem
  emu --> lnk
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
| `session/`, `gdb.rs` | protocole JSON, historique, serveur GDB | [Session](./debogage/session.md), [GDB](./debogage/gdb.md) |
| `formats/`, `keymap/`, `json.rs` | formats `.nds`, `.nfd`, `.nfs`, `.nrp`, `.nmk` | [Formats](../reference/formats/index.md) |
| `wasm.rs`, `server/`, `cli/`, `packages/`, `web/` | hôtes | [Hôtes](./hotes/index.md) |

## Lire cette section

Les pages vont du bas vers le haut : [processeur](./processeur/index.md), [mémoire](./memoire/index.md), [noyau](./noyau/index.md),
[éditeur de liens](./edition-de-liens/index.md), [exécution](./execution/index.md), [débogage](./debogage/index.md), [hôtes](./hotes/index.md),
[vérification](./verification/index.md). Chaque page cite les modules et les constantes pour qu'on retrouve le code en une recherche.
