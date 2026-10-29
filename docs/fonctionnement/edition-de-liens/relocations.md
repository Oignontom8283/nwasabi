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
