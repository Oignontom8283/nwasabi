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

