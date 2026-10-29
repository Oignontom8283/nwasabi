---
title: "Lecture et écriture ELF"
description: "Le lecteur ELF sûr de Nwasabi (aucune panique sur une entrée malformée, bornes configurables) et l'écrivain d'objets qui fabrique le runtime et l'ELF exporté."
---

# Lecture et écriture ELF

## Lecture : `ElfObject::parse`

`elf/` lit les fichiers ELF 32 bits ARM relogeables petit-boutistes. Le lecteur est écrit pour **ne jamais paniquer** sur une entrée malformée — le fichier peut venir d'un inconnu, déposé dans une page web : toutes les positions et les
tailles sont vérifiées, avec des erreurs (`ElfError` : `Truncated`, `NotElf`, `UnsupportedClass`, `UnsupportedMachine`, `SectionOutOfBounds`, `TooMany`, `RelaNotSupported`, `Malformed`…) qui disent où.

Bornes (`Limits`, configurables) : fichier de **64 Mio** au plus, 100 000 sections, 2 000 000 symboles, 8 000 000 relocations.

Structures lues : l'en-tête (`Header` : classe, ordre des octets, machine ARM, nombre de sections), les sections (`Section` : nom, type, drapeaux `SHF_ALLOC/WRITE/EXECINSTR/TLS`, contenu, alignement, `link`, `info`), les symboles
(`Symbol` : nom, valeur, taille, liaison `Local/Global/Weak`, type, section ou `Abs`/`Common`/`Undefined`) et les relocations **REL** (sans addend explicite : la valeur déjà en place sert d'addend), rangées par section à patcher.
Les constantes de relocation utiles sont dans `elf/reloc.rs`.

## Écriture : `ObjectWriter`

`elf/writer.rs` écrit un objet ELF relogeable minimal : sections, symboles, relocations. Il sert à **fabriquer des objets sans compilateur** : les stubs d'appels système du runtime, l'objet des données externes, les objets des tests de l'éditeur
de liens. Le résultat est un `.o` valide que `ElfObject::parse` relit et que les outils GNU savent lire. L'ELF **exécutable** que GDB lit (`LinkedApp::to_elf`) est écrit par le module `link/export.rs` : voir
[Application et export ELF](./application.md#export-elf-pour-gdb).

## Dans le code

| Où | Quoi |
|---|---|
| [`ElfObject::parse`](code:src/elf/mod.rs#parse) | l'entrée du lecteur |
| [`Limits`](code:src/elf/mod.rs#Limits) | les bornes anti-démesure |
| [`header.rs`](code:src/elf/header.rs), [`section.rs`](code:src/elf/section.rs), [`symbol.rs`](code:src/elf/symbol.rs), [`reloc.rs`](code:src/elf/reloc.rs), [`reader.rs`](code:src/elf/reader.rs) | les morceaux du format |
| [`ElfError`](code:src/elf/error.rs#ElfError) | les erreurs, toutes avec leur position |
| [`ObjectWriter`](code:src/elf/writer.rs#ObjectWriter) | l'écriture d'un objet minimal |
| [`elf/tests.rs`](code:src/elf/tests.rs) | tests (y compris des fichiers tronqués ou malformés) |

## Pourquoi un lecteur qui ne panique jamais

Le fichier peut être **déposé par n'importe qui dans une page web**. Un lecteur qui paniquerait sur un décalage faux ferait planter l'onglet (en WebAssembly, `panic = "abort"`). Chaque position et chaque taille est donc vérifiée par `reader.rs`, et les erreurs disent **où** elles se produisent. Les bornes de [`Limits`](code:src/elf/mod.rs#Limits) empêchent aussi un petit fichier de demander des gigaoctets.

## Pour modifier

- **Lire un nouveau champ** : l'ajouter à la structure concernée (`section.rs`…), lire par `reader.rs` (jamais indexer directement), ajouter un test avec une valeur invalide.
- **Prendre en charge `RELA`** (relocations avec addend explicite) : aujourd'hui `RelaNotSupported` ; il faudrait la lecture ici et l'application dans [`relocate.rs`](code:src/link/relocate.rs) (qui suppose `REL`).
