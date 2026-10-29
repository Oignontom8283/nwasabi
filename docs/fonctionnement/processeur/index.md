---
title: "Le processeur"
description: "Le module cpu de Nwasabi : un Cortex-M7 (ARMv7E-M, Thumb-2, DSP, FPv5-D16) autonome, ses états, ses arrêts, ses coutures avec la mémoire et le noyau."
---

# Le processeur

Le module `src/cpu/` émule un **Cortex-M7** : jeu d'instructions **Thumb-2** (ARMv7E-M), extension **DSP**, unité flottante **FPv5-D16** en
simple et double précision. Il est écrit pour pouvoir devenir une caisse à part : il n'utilise ni le reste de Nwasabi ni aucune dépendance
externe, et ne contient rien de propre à NumWorks (ni carte mémoire, ni appels système). `tests/cpu_independence.rs` interdit tout chemin
`crate::` dans ce dossier.

## Les coutures

```rust
pub trait Bus   { read8/16/32, write8/16/32, fetch16, read_block, write_block,
                  check_exclusive, code_epoch, privileged_write }
pub trait Hooks { fn on_svc<B: Bus>(&mut self, imm8: u8, cpu: &mut Cpu, bus: &mut B) -> SvcAction }
```

- **`Bus`** est la mémoire telle que la voit le processeur. Toutes les méthodes prennent `&mut self` : un accès peut avoir des effets de bord
  (suivi des accès, points d'observation). Les valeurs sont petit-boutistes. `fetch16` distingue le chargement d'une instruction de la lecture
  de donnée (droit d'exécution contre droit de lecture). `read_block` et `write_block` ont une version octet par octet par défaut, qu'un bus
  remplace par un accès en bloc. `code_epoch` donne le numéro qui change quand du code déjà exécuté a pu être modifié : `None` (le défaut)
  désactive le cache d'instructions. `privileged_write` est l'écriture du noyau simulé (programmation de la flash), permise même dans une zone
  en lecture seule.
- **`Hooks`** reçoit chaque `svc #imm8`. Il rend `Handled` (on continue), `HandledAndStop` (l'exécution rend la main à l'hôte) ou `Raise` (le
  processeur lève lui-même l'exception `SVCall`, voir [Exceptions](./exceptions.md)). Le noyau simulé est l'implémentation de `Hooks`.

## L'état : `Cpu`

| Champ | Rôle |
|---|---|
| `regs: Registers` | `r[0..=12]`, `sp`, `lr`, `pc`, `xpsr`, et les 32 registres flottants `s[0..32]` (mots de 32 bits) ; `fpscr` |
| `cycles: u64` | un cycle par instruction exécutée, **y compris celles d'un bloc `IT` qui n'ont pas lieu** : c'est le temps du noyau virtuel |
| `msp`, `psp` | la pile qui n'est pas active (la pile active est `regs.sp`) |
| `control`, `handler_mode` | bit 0 `nPRIV`, bit 1 `SPSEL`, bit 2 `FPCA` ; mode « handler » pendant une exception |
| `vtor`, `primask`, `faultmask`, `basepri` | registres système |
| `exclusive: Option<u32>` | moniteur d'exclusivité **local** (posé par `ldrex`, consommé par `strex`) |
| `fault_pc` | adresse de la dernière instruction dont un accès mémoire a été refusé (`pc` a déjà avancé) |
| `cache` | cache des instructions décodées ([Décodage](./decodage.md)) |

`xpsr` garde ensemble les drapeaux `N Z C V Q` (bits 31 à 27), le bit Thumb (24, toujours à 1), `GE[3:0]` (19 à 16), l'état `IT` (bits 26–25 et
15–10) et le numéro d'exception (`ipsr`, bits 8 à 0). `Registers` offre `n() z() c() v() q()`, `set_nz`, `set_nzcv`, `ge`, `it_state`, `advance_it`.

## Les arrêts : `Stop`

`Cpu::run(bus, hooks, max)` exécute au plus `max` instructions et rend la raison de l'arrêt :

| `Stop` | Cause |
|---|---|
| `Budget` | les `max` instructions sont faites |
| `Requested` | un hook a demandé l'arrêt (image prête, attente, fin de l'application…) |
| `BusFault(BusFault)` | accès mémoire refusé : adresse, sens, largeur, et `FaultKind` (`Unmapped`, `AccessDenied`, `ExclusiveUnsupported`) |
| `Unimplemented { pc, halfword }` | encodage valide ou non que le décodeur ne reconnaît pas |
| `UsageFault { pc, kind }` | `udf`, branchement en mode ARM (`InvalidState`), accès non aligné interdit (`Unaligned`) |
| `Breakpoint { imm8 }` | instruction `bkpt` |
| `WaitForInterrupt` | `wfi` ou `wfe` : rien ne pourrait réveiller le processeur |

Il n'y a **pas de gestionnaire d'exception pour les fautes** : le processeur s'arrête, et l'émulateur note la cause dans les registres de faute
simulés ([Appareil](../memoire/appareil.md#registres-de-faute)).

## Ce qui n'y est pas

Pas de pipeline, de cache de données, de temps d'accès à la flash ni de prédiction de branchement : **un cycle par instruction**. Pas de MPU,
de NVIC, de SysTick, de coprocesseurs, de NEON (le Cortex-M7 n'en a pas), de mode ARM. Le modèle de temps reste simple à dessein : le temps
d'une application est celui d'un processeur idéal à la fréquence du modèle, pas celui de la vraie puce.

## Pages

- [Décodage et cache d'instructions](./decodage.md)
- [Exécution](./execution.md) : blocs `IT`, drapeaux, accès mémoire, exclusivité, registres système
- [Virgule flottante](./virgule-flottante.md)
- [Exceptions et fautes](./exceptions.md)
- [Désassembleur](./desassembleur.md)
