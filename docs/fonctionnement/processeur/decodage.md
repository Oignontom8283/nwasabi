---
title: "Décodage et cache d'instructions"
description: "Comment les demi-mots Thumb et Thumb-2 deviennent des valeurs Instr, comment fonctionne le cache à correspondance directe et quand il est vidé."
---

# Décodage et cache d'instructions

## Deux demi-mots au plus

Une instruction Thumb-2 fait 16 ou 32 bits. Le premier demi-mot `hw1` dit laquelle : elle est sur **32 bits** si `hw1 >> 11 >= 0b11101` (les cinq
premiers bits valent `11101`, `11110` ou `11111`), sinon sur 16 bits. `cpu::decode16(hw1)` et `cpu::decode32(hw1, hw2)` rendent
`Option<Instr>` : `None` pour un encodage non reconnu, que `step` transforme en `Stop::Unimplemented { pc, halfword }`.

Séparer le décodage de l'exécution (`decode.rs` / `exec.rs`) a trois avantages : on teste le décodeur seul, le désassembleur le réutilise
([voir plus loin](./desassembleur.md)), et le résultat peut être **mémorisé**.

## La représentation : `Instr`

`Instr` (`instr.rs`) est un `enum` `Copy` dont les variantes regroupent les instructions par famille plutôt que par encodage. Les familles :

| Famille | Variantes |
|---|---|
| traitement de données | `Dp { op, rd, rn, rhs, flags }` (`and eor sub rsb add adc sbc orr orn bic mov mvn tst teq cmp cmn`), `Shift`, `Movw`, `Movt`, `AddSubImm12`, `Adr` |
| multiplication et division | `Mul`, `Mla`, `Mls`, `MulLong`, `Umaal`, `Div`, `Smul16`, `Smmul`, `Smulw`, `Smlalxy`, `Dual16`, `Dual16Long`, `Usad8` |
| DSP | `Saturating`, `Sat`, `Sat16`, `Parallel` (`s/q/sh/u/uq/uh` × `add16 asx sax sub16 add8 sub8`), `Sel`, `Pkh`, `Extend` (`sxtb uxth sxtab16`…), `Rev`, `Rbit`, `Clz`, `Bfi`, `Bfx` |
| branchements | `B`, `BCond`, `Bl`, `Bx`, `Blx`, `Cbz`, `TableBranch` |
| mémoire | `Load`, `Store` (taille `Byte/Half/Word`, signe, adressage `Imm/Reg/Literal`, `Offset/PreIndex/PostIndex`), `Ldrd`, `Strd`, `Ldm`, `Stm`, `Exclusive` |
| état et système | `It`, `Nop`, `Wfi`, `Wfe`, `Svc`, `Bkpt`, `Udf`, `Mrs`, `Msr`, `Cps`, `Barrier` (`dmb dsb isb`), `Clrex` |
| flottant | `Fp(FpInstr)` |

Le second opérande d'un traitement de données est un `Rhs` : une constante (`Imm { value, carry }` — le champ `carry` est la retenue produite
par le codage `ThumbExpandImm`, `None` si elle ne change pas) ou un registre décalé (`Reg { rm, kind, amount }`). `FlagMode` dit si
l'instruction met à jour les drapeaux : `Always` (forme 32 bits avec `S`, `cmp`, `tst`…), `Never`, ou `OutsideIt` — les formes 16 bits comme
`adds r0, r1, r2` ne modifient les drapeaux **qu'en dehors** d'un bloc `IT`.

Les petites fonctions d'arithmétique partagées sont dans `alu.rs` : `add_with_carry`, `shift_with_carry`, `decode_imm_shift`,
`thumb_expand_imm` (le codage des constantes modifiées sur 12 bits), `sign_extend`, `signed_saturate`, `unsigned_saturate`.

## Le cache : `DecodeCache`

Décoder coûte plus cher qu'exécuter, et un programme repasse sans cesse sur les mêmes instructions. `cache.rs` garde, pour une adresse,
l'`Instr` décodée et sa taille.

- **Correspondance directe** : 4096 cases (`SLOTS`), indexées par `(adresse >> 1) & 4095` ; une instruction fait 2 ou 4 octets, 4096 cases couvrent donc 8 Kio de code
  sans conflit. Une case garde l'adresse complète pour détecter un conflit.
- **Une époque** (`Bus::code_epoch`) accompagne le cache. Quand elle change, **tout le cache est vidé**. La mémoire l'incrémente à chaque écriture
  qui touche une ligne déjà marquée « exécutée » : du code copié en RAM puis exécuté, ou une flash reprogrammée, invalide donc le cache sans
  qu'on ait à surveiller chaque écriture dans le processeur.
- **Un bus qui répond `None`** (le défaut du trait) désactive le cache : le processeur reste correct avec n'importe quel bus.

Le cache ne gêne pas le débogage : il est transparent (il ne change aucune valeur observable), et un point d'observation ou un point d'arrêt
fait avancer l'exécution instruction par instruction par `Emulator`, sans rien changer à ce que le cache contient.

## Boucle de base

```rust
fn step_inline(&mut self, bus, hooks) -> Result<(), Stop> {
    let addr = self.regs.pc;
    let (instr, size) = cache.lookup(addr, bus.code_epoch()) or { fetch16 [+ fetch16], decode, cache.insert };
    let ctx = Ctx { addr, pc4: addr + 4, in_it: regs.in_it_block() };
    self.regs.pc = addr + size;      // pc a déjà avancé quand l'instruction s'exécute
    self.cycles += 1;
    // bloc IT : la condition est évaluée avant, l'état IT avance après
    ...
}
```

`pc4` (adresse + 4) est la valeur qu'une instruction lit quand elle utilise `PC` comme opérande, comme sur la puce ; le `pc` du registre, lui,
désigne déjà l'instruction suivante pendant l'exécution (d'où `fault_pc`, qui garde l'adresse de l'instruction fautive).
