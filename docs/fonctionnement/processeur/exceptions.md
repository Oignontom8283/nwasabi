---
title: "Exceptions et fautes"
description: "Ce que Nwasabi fait d'une exception ou d'une faute du processeur : empilement du contexte, EXC_RETURN, absence de gestionnaires et registres de faute simulés."
---

# Exceptions et fautes

## Deux cas bien séparés

1. **Une faute de l'application** (accès interdit, instruction indéfinie, `bkpt`, `wfi`) : l'exécution **s'arrête** avec une raison (`Stop`). Aucun
   gestionnaire ne s'exécute. L'émulateur note la cause dans les registres de faute simulés ([Appareil](../memoire/appareil.md#registres-de-faute)) pour qu'un
   débogueur voie un plantage ordinaire.
2. **Un `svc` que personne ne traite** (`SvcAction::Raise`) : le processeur lève lui-même l'exception `SVCall` (numéro 11). C'est le mécanisme
   d'un futur mode « firmware complet » ; le noyau simulé actuel traite tous les `svc` lui-même et ne s'en sert pas. Il est exercé par les tests
   du processeur.

## `take_exception`

`exception.rs` implémente l'entrée dans une exception (`Cpu::take_exception(bus, numéro)`) comme le décrit l'architecture ARMv7-M :

1. le **moniteur d'exclusivité local** est effacé ;
2. le **contexte** est empilé sur la pile active : `r0–r3`, `r12`, `lr`, l'adresse de retour (`pc`), `xpsr` — 8 mots (`0x20` octets), plus 18 mots
   (`0x48` en tout) si le contexte flottant est actif (`CONTROL.FPCA`) ;
3. si la pile n'est pas alignée sur 8 octets, un mot de remplissage est ajouté et le bit 9 de `xpsr` (`XPSR_ALIGN`) le mémorise ;
4. `lr` reçoit la valeur **`EXC_RETURN`** qui dit d'où l'on vient : `0xFFFF_FFF1` (depuis le mode handler), `0xFFFF_FFF9` (mode thread, pile principale) ou
   `0xFFFF_FFFD` (mode thread, pile de processus) ; le bit 4 est mis à zéro si un contexte flottant a été empilé ;
5. le processeur passe en mode handler sur la pile principale (`SPSEL` et `FPCA` effacés, état `IT` remis à zéro), `IPSR` reçoit le numéro et `pc` le vecteur lu à
   `vtor + 4 × numéro` (bit 0 retiré).

Le retour se déclenche quand `bx`, `pop {pc}` ou un chargement dans `pc` écrit une valeur `EXC_RETURN` **en mode handler** (`bx_write_pc`) : le contexte est
dépilé et l'état d'avant (mode, pile, `FPCA`) restauré.

## Ce qui n'existe pas

Pas de contrôleur d'interruptions (NVIC), de priorités, de `SysTick`, de basculement `PendSV`, d'imbrication d'exceptions pilotée par priorité,
de MPU. **Un ordonnanceur préemptif ne peut donc pas tourner sur Nwasabi**, et ce n'est pas un objectif : l'émulateur exécute l'application, pas le
système. Une application qui programme ses propres gestionnaires se heurte à la même limite.

## De l'arrêt à la faute

`Emulator::run_inner` convertit chaque `Stop` en `RunStop::Fault(Fault)` :

| `Stop` | `Fault` | Bits des registres de faute |
|---|---|---|
| `BusFault` en lecture ou écriture, zone inexistante ou accès exclusif refusé | `Bus(...)` | `CFSR.PRECISERR` + `BFARVALID`, `BFAR` = adresse |
| `BusFault` en lecture ou écriture, droits refusés | `Bus(...)` | `CFSR.DACCVIOL` + `MMARVALID`, `MMFAR` = adresse |
| `BusFault` au chargement de l'instruction, droits refusés | `Bus(...)` | `CFSR.IACCVIOL` |
| `BusFault` au chargement de l'instruction, zone inexistante | `Bus(...)` | `CFSR.IBUSERR` |
| `Unimplemented` / `UsageFault::UndefinedInstruction` | `Unimplemented` / `UsageFault` | `CFSR.UNDEFINSTR` |
| `UsageFault::InvalidState` | `UsageFault` | `CFSR.INVSTATE` |
| `UsageFault::Unaligned` | `UsageFault` | `CFSR.UNALIGNED` |
| `Breakpoint` | `Breakpoint` | `DFSR.BKPT` |
| `WaitForInterrupt` | `WaitForInterrupt` | |
| `KernelStop::UnknownSvc` (avec `strict_svc`) | `UnknownSvc` | |

Toute faute qui remplit `CFSR` met aussi `HFSR.FORCED` (faute escaladée en HardFault) et `ICSR.VECTACTIVE` à 3. Les bits se **cumulent** et ne s'effacent qu'à la relance
de l'application ; `MMFAR` et `BFAR` gardent la dernière adresse en cause. Un chargement d'instruction est reconnu à `adresse == pc` en lecture.

Les fautes sont rapportées avec l'adresse de l'instruction fautive (`fault_pc`), et non celle du `pc` déjà avancé. Une faute causée par un
pointeur passé à un appel système (par exemple un tampon hors mémoire) est rapportée sur l'instruction `svc`, juste avant `pc`.
