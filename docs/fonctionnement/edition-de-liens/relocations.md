---
title: "Relocations ARM"
description: "Les types de relocation gérés, la formule S + A − P, l'encodage des branchements longs Thumb-2 et de movw/movt, et les erreurs détectées."
---

# Relocations

::: tip En bref
Une relocation est une **adresse à compléter**. Le compilateur écrit « ici, mettre l'adresse de `ma_fonction` » ; l'éditeur de liens, qui connaît enfin les adresses, remplit le trou.
:::


Les fichiers ARM utilisent des relocations de type **REL** : la valeur déjà présente à l'emplacement sert d'**addend** `A`. On y ajoute l'adresse du symbole `S` et, si la valeur est relative, on retranche l'adresse de l'emplacement
lui-même `P`. `T` vaut 1 quand le symbole est une **fonction Thumb** : le bit 0 de `S` signale alors le mode Thumb. `relocate::apply(kind, section, offset, place, target)` calcule la valeur et l'écrit.

| Type | Calcul | Remarques |
|---|---|---|
| `R_ARM_NONE` | rien | sert de dépendance (voir [Symboles](./symboles.md#sections-utiles)) |
| `R_ARM_ABS32`, `R_ARM_TARGET1` | `S + A` (mot de 32 bits) | `TARGET1` vaut `ABS32` sur ce système |
| `R_ARM_REL32` | `S + A − P` | |
| `R_ARM_PREL31` | `S + A − P` sur 31 bits | pour les tables d'exceptions ; une valeur qui sort des 31 bits est une erreur (`Overflow`) |
| `R_ARM_THM_CALL`, `R_ARM_THM_JUMP24` | branchement long `bl` / `b.w`, ±16 Mio | voir ci-dessous |
| `R_ARM_THM_MOVW_ABS_NC`, `R_ARM_THM_MOVT_ABS` | constante 16 bits basse ou haute de `S + A` | `movw`/`movt` |

## Branchements longs : `link/thumb.rs`

Une instruction Thumb-2 de 32 bits s'écrit en deux demi-mots `hw1`, `hw2`. `classify_branch` reconnaît `bl`, `blx` ou `b.w` (encodage T4) d'après eux ; `decode_branch_offset` lit le décalage encodé (champ « cible − (adresse + 4) ») et
`encode_branch_offset` le réécrit en **conservant tout le reste** de l'instruction. La plage est de ±16 Mio (`BRANCH_RANGE`) : au-delà, ou pour un décalage impair, `None` et `RelocError::Overflow`. `movw` et `movt` se reconnaissent par
`classify_mov16` ; `decode_mov16` et `encode_mov16` manipulent la constante `imm4:i:imm3:imm8` sans toucher au registre ni à l'opcode.

## Règles de cohérence

- `THM_CALL` accepte `bl` ou `b.w` ; `THM_JUMP24` n'accepte que `b.w`. Un `blx` (appel vers du code **ARM**, inexistant sur Cortex-M) est refusé : `RelocError::Interworking`.
- Une instruction qui n'est pas celle attendue pour le type de relocation : `RelocError::BadInstruction { kind, hw1, hw2 }`.
- Un appel (`bl`) vers un **symbole faible resté non défini** est remplacé par `nop.w` (`NOP_W = 0xF3AF, 0x8000`) : `S` vaut 0 mais l'appel ne plante pas.
- `MOVW` appliquée à un `movt`, ou l'inverse, est refusée.
- Un type de relocation inconnu : `RelocError::Unsupported`. Un emplacement qui dépasse la section : `RelocError::OutOfSection`.

Chaque erreur remonte en `LinkError::Relocation` avec le **symbole**, la **section** et l'**objet** concernés.

## Dans le code

| Où | Quoi |
|---|---|
| [`apply`](code:src/link/relocate.rs#apply) | une relocation : le `match` sur le type |
| [`RelocError`](code:src/link/relocate.rs#RelocError) | les erreurs |
| [`classify_branch`](code:src/link/thumb.rs#classify_branch), [`decode_branch_offset`](code:src/link/thumb.rs#decode_branch_offset), [`encode_branch_offset`](code:src/link/thumb.rs#encode_branch_offset) | les branchements longs |
| [`classify_mov16`](code:src/link/thumb.rs#classify_mov16), [`encode_mov16`](code:src/link/thumb.rs#encode_mov16) | `movw` / `movt` |
| [`NOP_W`](code:src/link/thumb.rs#NOP_W) | le remplaçant d'un appel faible non défini |
| [`elf/reloc.rs`](code:src/elf/reloc.rs) | les constantes de types |
| l'application d'une relocation à une section | [`linker.rs`](code:src/link/linker.rs#~relocate::apply) |

## Pourquoi seulement ces types

Ce sont ceux que **GCC et `rustc`** produisent pour du code Thumb-2 sans PIC : adresses absolues (`ABS32`), relatives (`REL32`, `PREL31`), appels et sauts longs (`THM_CALL`, `THM_JUMP24`) et constantes 16 bits (`MOVW`/`MOVT`). Le corpus d'applications tierces n'en emploie pas d'autres ; l'éditeur de liens refuse avec `Unsupported(type)` plutôt que de deviner : l'erreur donne le numéro de relocation, qu'on cherche dans la documentation ELF d'ARM.

**Symptômes en pratique** :

| Message | Cause probable |
|---|---|
| `Unsupported(n)` | le compilateur a utilisé un mode (PIC, GOT, TLS…) que l'EADK ne prévoit pas |
| `Interworking` | un appel vers du code ARM (non Thumb) : le fichier mélange deux modes |
| `Overflow` sur un branchement | la cible est à plus de ±16 Mio : le code de l'application est trop grand ou mal placé |
| `BadInstruction` | la relocation ne tombe pas sur l'instruction attendue : fichier objet corrompu |

## Pour modifier

Pour ajouter un type : une branche dans [`apply`](code:src/link/relocate.rs#apply), la constante dans [`reloc.rs`](code:src/elf/reloc.rs), et **deux tests** (une valeur qui passe, une qui déborde) dans [`relocate.rs`](code:src/link/relocate.rs). Écrire la formule dans le tableau de cette page.
