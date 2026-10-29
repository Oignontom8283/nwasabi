---
title: "Virgule flottante FPv5"
description: "Comment Nwasabi émule l'unité flottante du Cortex-M7 : simple et double précision, FPSCR, cas particuliers ARM, arrondis, conversions et transferts."
---

# Virgule flottante FPv5-D16

::: tip En bref
Rust sait faire des calculs flottants corrects ; ARM a quelques **manies** (NaN, dénormalisés, drapeaux). Nwasabi calcule avec les `f32`/`f64` de Rust, puis corrige à la main ce qui diffère.
:::


`fpu.rs` (décodage et exécution en simple précision) et `fpu_double.rs` (double précision et demi-précision) émulent l'unité flottante du
Cortex-M7 : **FPv5-D16**, soit seize registres doubles `D0`–`D15` qui recouvrent les trente-deux registres simples `S0`–`S31`
(`Dk` = `S(2k)` pour le mot de poids faible et `S(2k+1)`). Les registres sont stockés comme des **mots de 32 bits** (`regs.s[i]`), jamais
comme des `f32` : une copie d'un registre, un `vldr` ou un `vmov` ne change donc pas un seul bit, même sur un NaN.

## Choix de réalisation

Les calculs utilisent les `f32` et `f64` de Rust, qui arrondissent au plus proche comme le mode par défaut du processeur. Ce qui diffère
d'IEEE-754 « à la Rust » est traité à la main :

| Particularité ARM | Traitement |
|---|---|
| **NaN par défaut** | un résultat NaN issu de deux entiers non NaN est le NaN canonique `0x7FC00000` (`0x7FF8…` en double) ; avec `FPSCR.DN`, tout NaN devient celui-là |
| **Propagation des NaN** | un opérande NaN silencieux est propagé ; un NaN signalant est rendu silencieux et lève `IOC` |
| **Dénormalisés** | `FPSCR.FZ` met à zéro les entrées et les sorties dénormalisées (`IDC` pour une entrée) |
| **Arrondi** | `FPSCR.RMODE` choisit au plus proche, vers +∞, vers −∞ ou vers zéro (`next_up`/`next_down` ajustent le résultat) ; `ROUND_AWAY` et `ROUND_FPSCR` servent aux instructions qui imposent ou délèguent leur arrondi |
| **Exceptions cumulées** | `IOC`, `DZC`, `OFC`, `UFC`, `IXC`, `IDC` sont des drapeaux collants (aucune ne se déroute : les exceptions flottantes ne sont jamais levées) |
| **Exactitude** | pour savoir si un résultat en double précision est exact sans type plus large, les **erreurs d'arrondi sont calculées exactement** (somme à deux termes `TwoSum`, multiplication-addition fusionnée) |

`vfma`, `vfms`, `vfnma`, `vfnms` (un seul arrondi) s'obtiennent par `mul_add` ; `vmla`, `vmls`, `vnmla`, `vnmls` (produit arrondi avant l'addition) par deux opérations.

## Instructions prises en charge

`FpInstr` regroupe :

- **arithmétique** `Arith` : `vmla vmls vnmla vnmls vmul vnmul vadd vsub vdiv vfma vfms vfnma vfnms` ;
- **unaires** `Unary` : `vmov` (registre à registre), `vabs`, `vneg`, `vsqrt` ;
- **constantes** `MovImm` (`vmov.f32 Sd, #imm`, la constante est développée au décodage) ;
- **comparaisons** `Cmp` (`vcmp`, `vcmpe`, avec zéro ou un registre ; `vmrs APSR_nzcv, FPSCR` recopie `N Z C V` dans `xpsr`) ;
- **conversions** : flottant → entier (`ToInt`, `vcvt` vers zéro, `vcvtr` selon `FPSCR`), entier → flottant (`FromInt`), `ToIntRounded` (`vcvta vcvtn vcvtp vcvtm`),
  virgule fixe (`ToFixed`, `FromFixed`, sur 16 ou 32 bits avec `fbits`), demi-précision (`vcvtb`, `vcvtt`) ;
- **arrondis** `Rint` (`vrinta vrintn vrintp vrintm vrintz vrintr vrintx`) ;
- **sélection** `Vsel`, `MaxMinNum` (`vmaxnm`, `vminnm`) ;
- **transferts** : `MovCore` (`vmov Rt, Sn`), `MovTwoCore` (deux mots ou un double), `Vmrs`, `Vmsr` ;
- **mémoire** : `Ldr`/`Str` (`vldr`, `vstr`), `Ldm`/`Stm` (`vldm`, `vstm`, `vpush`, `vpop`), un `words` de 1 (simple) ou 2 (double) ; un accès non aligné sur un mot donne
  `UsageFault::Unaligned`.

`Double(DInstr)` répète l'arithmétique, les conversions, les comparaisons, la racine, les transferts et les conversions simple↔double sur
`D0`–`D15`.

## Ce qui n'est pas géré

- le format de **demi-précision alternatif** (`FPSCR.AHP`) ;
- les registres `D16`–`D31` : ils n'existent pas sur un Cortex-M7 (FPv5-**D16**) ;
- les exceptions flottantes déroutées (`FPSCR` n'a pas de bits d'activation utiles, aucun gestionnaire ne s'exécute) ;
- **NEON** (`vadd.i32 q0, q1, q2`…) : le Cortex-M7 n'en a pas, l'encodage est un `Unimplemented`.

## Contexte flottant et exceptions

Le bit `CONTROL.FPCA` dit si le contexte flottant est actif ; il agrandit le cadre empilé à l'entrée d'une exception (`S0`–`S15` et `FPSCR`, voir
[Exceptions](./exceptions.md)). L'empilement paresseux (`FPCCR.LSPACT`) n'est pas reproduit : le contexte est copié tout de suite.

## Dans le code

| Où | Quoi |
|---|---|
| [`FpInstr`](code:src/cpu/fpu.rs#FpInstr), [`decode`](code:src/cpu/fpu.rs#decode) | instructions flottantes simples et leur décodage |
| [`exec_fp`](code:src/cpu/fpu.rs#exec_fp) | l'exécution (simple précision et dispatch vers le double) |
| [`FpArith`](code:src/cpu/fpu.rs#FpArith), [`FpUnary`](code:src/cpu/fpu.rs#FpUnary) | les opérations |
| [`next_up`](code:src/cpu/fpu.rs#next_up), [`next_down`](code:src/cpu/fpu.rs#next_down) | ajuster un résultat pour un arrondi dirigé |
| [`FPSCR_FZ`](code:src/cpu/fpu.rs#FPSCR_FZ), [`FPSCR_DN`](code:src/cpu/fpu.rs#FPSCR_DN) | bits de contrôle |
| [`DInstr`](code:src/cpu/fpu_double.rs#DInstr), [`decode_data`](code:src/cpu/fpu_double.rs#decode_data), [`exec_double`](code:src/cpu/fpu_double.rs#exec_double) | double précision |

## Pourquoi des mots de 32 bits et pas des `f32`

Les registres sont stockés comme `u32` (`regs.s[i]`). Une copie (`vmov`, `vldr`) doit conserver **tous les bits**, y compris la charge utile d'un NaN, que les opérations de Rust peuvent modifier en passant par un `f32`. Un `Dk` est simplement la paire `S(2k)`, `S(2k+1)` : la vue double ne coûte rien et reste cohérente avec la vue simple.

## Pourquoi calculer en Rust puis corriger

Écrire un arrondi exact à la main en simple et double précision serait long et plein de pièges ; les `f32` et `f64` de Rust font déjà l'arrondi **au plus proche** correctement. On ne corrige que ce qui diffère d'ARM : NaN par défaut, mise à zéro des dénormalisés (`FZ`), drapeaux collants, arrondis dirigés. Pour savoir si un résultat est **exact** (drapeau `IXC`) sans type plus large, on calcule l'**erreur d'arrondi** exactement (somme à deux termes, `mul_add`).

## Pour modifier

1. Ajouter la variante à `FpInstr` ou `DInstr`, la reconnaître dans [`decode`](code:src/cpu/fpu.rs#decode) (ou `decode_data` / `decode_v8` / `decode_half`).
2. L'exécuter dans [`exec_fp`](code:src/cpu/fpu.rs#exec_fp) / [`exec_double`](code:src/cpu/fpu_double.rs#exec_double), en passant **par les helpers de drapeaux** existants (ne jamais écrire `fpscr` à la main).
3. Ajouter un lot dans [`generate_vectors.py`](code:tests/cpu/generate_vectors.py) (`cases_fp_arith`, `cases_fp_double`…) : les valeurs limites (NaN signalant, dénormalisés, `±0`, infinis) sont ce qui attrape les erreurs.
