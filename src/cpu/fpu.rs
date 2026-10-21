//! Virgule flottante simple précision (FPv5-D16) : décodage et exécution.
//!
//! Sont gérés : l'arithmétique sur les registres simples (`vadd`, `vmul`, `vfma`, `vsqrt`…), les
//! comparaisons, les conversions entre flottants et entiers, les transferts vers les registres
//! entiers et les accès mémoire (y compris sur des registres doubles, vus comme deux simples).
//! L'arithmétique en double précision n'est pas gérée : les applications visées n'utilisent que la
//! simple précision.
//!
//! Les calculs utilisent les `f32` de Rust (arrondi au plus proche, comme le mode par défaut du
//! processeur). Les cas particuliers propres à ARM sont traités à la main : propagation des NaN,
//! NaN par défaut, mise à zéro des dénormalisés (`FZ`), indicateurs d'exception cumulés.

use super::instr::Reg;
use super::{Bus, Cpu, Stop, UsageFault};

/// Opérations à trois registres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpArith {
    /// `Sd = Sd + Sn × Sm` (produit arrondi avant l'addition).
    Vmla,
    /// `Sd = Sd − Sn × Sm`.
    Vmls,
    /// `Sd = −Sd − Sn × Sm`.
    Vnmla,
    /// `Sd = −Sd + Sn × Sm`.
    Vnmls,
    Vmul,
    /// `Sd = −(Sn × Sm)`.
    Vnmul,
    Vadd,
    Vsub,
    Vdiv,
    /// Multiplication-addition fusionnée (un seul arrondi) : `Sd + Sn × Sm`.
    Vfma,
    /// `Sd − Sn × Sm`, fusionnée.
    Vfms,
    /// `−Sd − Sn × Sm`, fusionnée.
    Vfnma,
    /// `−Sd + Sn × Sm`, fusionnée.
    Vfnms,
}

/// Opérations à deux registres qui ne lèvent pas d'exception sur les NaN, ou presque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpUnary {
    Vmov,
    Vabs,
    Vneg,
    Vsqrt,
}

/// Instruction à virgule flottante décodée. Les numéros `d`, `n`, `m`, `s`, `first` désignent des
/// registres simples (0 à 31) ; un registre double `Dk` occupe `S(2k)` et `S(2k+1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpInstr {
    Arith {
        op: FpArith,
        d: u8,
        n: u8,
        m: u8,
    },
    Unary {
        op: FpUnary,
        d: u8,
        m: u8,
    },
    /// `vmov.f32 Sd, #constante` (bits de la constante déjà développés).
    MovImm {
        d: u8,
        bits: u32,
    },
    /// `vcmp` / `vcmpe` ; `m` à `None` : comparaison avec zéro.
    Cmp {
        d: u8,
        m: Option<u8>,
        signal_nan: bool,
    },
    /// Conversion flottant → entier. `round_zero` : arrondi vers zéro (`vcvt`) ; sinon selon FPSCR (`vcvtr`).
    ToInt {
        signed: bool,
        round_zero: bool,
        d: u8,
        m: u8,
    },
    /// Conversion entier → flottant.
    FromInt {
        signed: bool,
        d: u8,
        m: u8,
    },
    /// `vmrs rt, fpscr` (`rt` = 15 : copie N, Z, C, V dans APSR).
    Vmrs {
        rt: Reg,
    },
    Vmsr {
        rt: Reg,
    },
    /// `vmov rt, Sn` (`to_core`) ou `vmov Sn, rt`.
    MovCore {
        to_core: bool,
        rt: Reg,
        s: u8,
    },
    /// Transfert de deux registres entiers vers/depuis deux registres simples consécutifs (ou un double).
    MovTwoCore {
        to_core: bool,
        rt: Reg,
        rt2: Reg,
        first: u8,
    },
    /// `vldr` / `vstr` : `words` mots (1 pour un simple, 2 pour un double) à base + `offset`.
    Ldr {
        first: u8,
        words: u8,
        rn: Reg,
        offset: i32,
    },
    Str {
        first: u8,
        words: u8,
        rn: Reg,
        offset: i32,
    },
    /// `vldm` / `vpop` : `words` mots à partir de `first`.
    Ldm {
        first: u8,
        words: u8,
        rn: Reg,
        writeback: bool,
        decrement_before: bool,
    },
    /// `vstm` / `vpush`.
    Stm {
        first: u8,
        words: u8,
        rn: Reg,
        writeback: bool,
        decrement_before: bool,
    },
}

fn bits(v: u32, hi: u32, lo: u32) -> u32 {
    (v >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

/// Décode une instruction de coprocesseur de 32 bits (flottante) ; `None` si elle n'en est pas une
/// ou n'est pas gérée.
pub fn decode(hw1: u16, hw2: u16) -> Option<FpInstr> {
    let (a, b) = (u32::from(hw1), u32::from(hw2));
    // Instructions « conditionnelles » du coprocesseur 10 (simple) ou 11 (double).
    if a >> 10 != 0b111011 {
        return None;
    }
    let double = match bits(b, 11, 8) {
        0b1010 => false,
        0b1011 => true,
        _ => return None,
    };
    // Registre simple (Vx:bit) ou double ; on renvoie toujours un indice de registre simple.
    let single = |hi: u32, low_bit: u32| (hi << 1 | low_bit) as u8;
    let dreg = |hi: u32, low_bit: u32| (2 * (low_bit << 4 | hi)) as u8;
    match bits(a, 9, 8) {
        // 1110 1100 / 1110 1101 : accès mémoire et transferts de 64 bits.
        0b00 | 0b01 => decode_transfer(a, b, double, single, dreg),
        // 1110 1110 : traitement de données, ou transfert d'un registre entier.
        _ => {
            if bits(b, 4, 4) == 0 {
                if double {
                    None
                } else {
                    decode_data(a, b, single)
                }
            } else if double {
                None
            } else {
                decode_core_transfer(a, b, single)
            }
        }
    }
}

fn decode_transfer(
    a: u32,
    b: u32,
    double: bool,
    single: impl Fn(u32, u32) -> u8,
    dreg: impl Fn(u32, u32) -> u8,
) -> Option<FpInstr> {
    // Transfert de deux registres entiers : 1110 1100 010 L Rt2 / Rt 101z 00 M 1 Vm.
    if bits(a, 9, 5) == 0b00010 {
        let (rt, rt2) = (bits(b, 15, 12) as Reg, bits(a, 3, 0) as Reg);
        let (vm, m) = (bits(b, 3, 0), bits(b, 5, 5));
        if bits(b, 7, 6) != 0b00 || bits(b, 4, 4) != 1 {
            return None;
        }
        let first = if double { dreg(vm, m) } else { single(vm, m) };
        return Some(FpInstr::MovTwoCore {
            to_core: bits(a, 4, 4) == 1,
            rt,
            rt2,
            first,
        });
    }
    // Chargements et rangements : 1110 110 P U D W L Rn / Vd 101z imm8.
    let (p, u, d, w, l) = (bit(a, 8), bit(a, 7), bits(a, 6, 6), bit(a, 5), bit(a, 4));
    let rn = bits(a, 3, 0) as Reg;
    let vd = bits(b, 15, 12);
    let imm8 = bits(b, 7, 0);
    let first = if double { dreg(vd, d) } else { single(vd, d) };
    if p && !w {
        // vldr / vstr : un seul registre, décalage de imm8 × 4.
        let offset = (imm8 * 4) as i32;
        let offset = if u { offset } else { -offset };
        let words = if double { 2 } else { 1 };
        return Some(if l {
            FpInstr::Ldr {
                first,
                words,
                rn,
                offset,
            }
        } else {
            FpInstr::Str {
                first,
                words,
                rn,
                offset,
            }
        });
    }
    // vldm / vstm (incrémentation après) et vldmdb / vstmdb (décrémentation avant ; vpush / vpop).
    let decrement_before = match (p, u, w) {
        (false, true, _) => false,
        (true, false, true) => true,
        _ => return None,
    };
    if double && imm8 % 2 == 1 {
        return None; // anciennes instructions fldmx / fstmx
    }
    if imm8 == 0 || (first as u32 + imm8) > 32 {
        return None;
    }
    let (writeback, words) = (w, imm8 as u8);
    Some(if l {
        FpInstr::Ldm {
            first,
            words,
            rn,
            writeback,
            decrement_before,
        }
    } else {
        FpInstr::Stm {
            first,
            words,
            rn,
            writeback,
            decrement_before,
        }
    })
}

fn bit(v: u32, n: u32) -> bool {
    (v >> n) & 1 != 0
}

fn decode_data(a: u32, b: u32, single: impl Fn(u32, u32) -> u8) -> Option<FpInstr> {
    let opc1 = bits(a, 7, 4);
    let d = single(bits(b, 15, 12), bits(a, 6, 6));
    let n = single(bits(a, 3, 0), bits(b, 7, 7));
    let m = single(bits(b, 3, 0), bits(b, 5, 5));
    let s = bit(b, 6);
    let (p, q, r) = (bit(opc1, 3), bit(opc1, 1), bit(opc1, 0));
    let arith = |op| Some(FpInstr::Arith { op, d, n, m });
    match (p, q, r, s) {
        (false, false, false, false) => arith(FpArith::Vmla),
        (false, false, false, true) => arith(FpArith::Vmls),
        (false, false, true, false) => arith(FpArith::Vnmls),
        (false, false, true, true) => arith(FpArith::Vnmla),
        (false, true, false, false) => arith(FpArith::Vmul),
        (false, true, false, true) => arith(FpArith::Vnmul),
        (false, true, true, false) => arith(FpArith::Vadd),
        (false, true, true, true) => arith(FpArith::Vsub),
        (true, false, false, false) => arith(FpArith::Vdiv),
        (true, false, true, false) => arith(FpArith::Vfnms),
        (true, false, true, true) => arith(FpArith::Vfnma),
        (true, true, false, false) => arith(FpArith::Vfma),
        (true, true, false, true) => arith(FpArith::Vfms),
        (true, true, true, _) => decode_extension(a, b, d, m),
        _ => None,
    }
}

