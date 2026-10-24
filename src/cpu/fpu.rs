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

use super::fpu_double::{self, DInstr};
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
    /// Double précision (D0 à D15) et conversions vers la demi-précision.
    Double(DInstr),
    /// `vcvta`, `vcvtn`, `vcvtp`, `vcvtm` : flottant → entier avec un arrondi imposé par l'instruction.
    ToIntRounded {
        signed: bool,
        rounding: u32,
        d: u8,
        m: u8,
    },
    /// `vrinta`, `vrintn`, `vrintp`, `vrintm`, `vrintz`, `vrintr`, `vrintx` : arrondi à un entier, en flottant.
    Rint {
        rounding: u32,
        /// Lève l'exception d'inexactitude quand le résultat diffère (`vrintx`).
        exact: bool,
        d: u8,
        m: u8,
    },
    /// `vsel` : copie `Sn` ou `Sm` selon les indicateurs entiers (`cond` : 0 EQ, 1 VS, 2 GE, 3 GT).
    Vsel {
        cond: u8,
        d: u8,
        n: u8,
        m: u8,
    },
    /// `vmaxnm` (`max`) ou `vminnm` : un NaN silencieux cède la place à l'autre opérande.
    MaxMinNum {
        max: bool,
        d: u8,
        n: u8,
        m: u8,
    },
    /// `vcvt` flottant → virgule fixe, en place : `Sd` reçoit `trunc(Sd × 2^fbits)` saturé sur 16 ou 32 bits.
    ToFixed {
        signed: bool,
        wide: bool,
        fbits: u32,
        d: u8,
    },
    /// `vcvt` virgule fixe → flottant, en place.
    FromFixed {
        signed: bool,
        wide: bool,
        fbits: u32,
        d: u8,
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
    // Extensions de FPv5 (ARMv8) : `vsel`, `vmaxnm`, `vrint*`, `vcvt{a,n,p,m}`.
    if a >> 8 == 0xfe {
        return decode_v8(a, b);
    }
    // `vcvtb` / `vcvtt` : demi-précision (en simple ou en double).
    if a & 0xffbe == 0xeeb2 && bit(b, 6) {
        return fpu_double::decode_half(a, b).map(FpInstr::Double);
    }
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
                    fpu_double::decode_data(a, b).map(FpInstr::Double)
                } else {
                    decode_data(a, b, single)
                }
            } else if double {
                decode_lane(a, b)
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

/// `vmov.32 Dn[x], Rt` et `vmov.32 Rt, Dn[x]` : un des deux mots d'un registre double.
fn decode_lane(a: u32, b: u32) -> Option<FpInstr> {
    // 1110 1110 0 0x L Vn / Rt 1011 N 00 1 0000
    if bit(a, 7) || bit(a, 6) || bits(b, 6, 5) != 0 || bits(b, 3, 0) != 0 || bit(b, 7) {
        return None;
    }
    Some(FpInstr::MovCore {
        to_core: bit(a, 4),
        rt: bits(b, 15, 12) as Reg,
        s: (2 * bits(a, 3, 0) + bits(a, 5, 5)) as u8,
    })
}

/// Instructions de FPv5 à préfixe `0xFE`.
fn decode_v8(a: u32, b: u32) -> Option<FpInstr> {
    if bits(b, 11, 8) == 0b1011 {
        return fpu_double::decode_v8(a, b).map(FpInstr::Double);
    }
    if bits(b, 11, 8) != 0b1010 || bit(b, 4) {
        return None;
    }
    let single = |hi: u32, low_bit: u32| (hi << 1 | low_bit) as u8;
    let d = single(bits(b, 15, 12), bits(a, 6, 6));
    let n = single(bits(a, 3, 0), bits(b, 7, 7));
    let m = single(bits(b, 3, 0), bits(b, 5, 5));
    // Dans les groupes d'arrondi, `rm` : 0 vers le plus proche (égalité à l'écart de zéro), 1 vers
    // le plus proche pair, 2 vers +infini, 3 vers -infini.
    let rounding = |rm: u32| match rm {
        0 => ROUND_AWAY,
        1 => 0,
        2 => 1,
        _ => 2,
    };
    if !bit(a, 7) {
        if bit(b, 6) {
            return None;
        }
        return Some(FpInstr::Vsel {
            cond: bits(a, 5, 4) as u8,
            d,
            n,
            m,
        });
    }
    match (bits(a, 5, 4), bits(a, 3, 2)) {
        (0b00, _) => Some(FpInstr::MaxMinNum {
            max: !bit(b, 6),
            d,
            n,
            m,
        }),
        (0b11, 0b10) if bits(b, 7, 6) == 0b01 => Some(FpInstr::Rint {
            rounding: rounding(bits(a, 1, 0)),
            exact: false,
            d,
            m,
        }),
        (0b11, 0b11) if bit(b, 6) => Some(FpInstr::ToIntRounded {
            signed: bit(b, 7),
            rounding: rounding(bits(a, 1, 0)),
            d,
            m,
        }),
        _ => None,
    }
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

/// Instructions du groupe « opc1 = 1x11 » : constante, copies, valeur absolue, comparaisons, conversions.
fn decode_extension(a: u32, b: u32, d: u8, m: u8) -> Option<FpInstr> {
    let opc2 = bits(a, 3, 0);
    let op = bits(b, 7, 6);
    // vmov.f32 Sd, #imm : opc3 = x0, constante imm4H:imm4L.
    if op & 1 == 0 {
        let imm8 = (bits(a, 3, 0) << 4) | bits(b, 3, 0);
        let sign = (imm8 >> 7) & 1;
        let not_b = ((imm8 >> 6) & 1) ^ 1;
        let b_rep = if (imm8 >> 6) & 1 == 1 { 0b11111 } else { 0 };
        let fraction = imm8 & 0x3f;
        let value = (sign << 31) | (not_b << 30) | (b_rep << 25) | (fraction << 19);
        return Some(FpInstr::MovImm { d, bits: value });
    }
    Some(match opc2 {
        0b0000 if op == 0b01 => FpInstr::Unary {
            op: FpUnary::Vmov,
            d,
            m,
        },
        0b0000 => FpInstr::Unary {
            op: FpUnary::Vabs,
            d,
            m,
        },
        0b0001 if op == 0b01 => FpInstr::Unary {
            op: FpUnary::Vneg,
            d,
            m,
        },
        0b0001 => FpInstr::Unary {
            op: FpUnary::Vsqrt,
            d,
            m,
        },
        // vcvt.f64.f32 Dd, Sm (la version inverse est décodée avec la double précision).
        0b0111 if op == 0b11 => FpInstr::Double(DInstr::FromSingle {
            d: if d & 1 == 0 { d } else { return None },
            m,
        }),
        // vrintr (selon FPSCR), vrintz (vers zéro), vrintx (selon FPSCR, avec exception d'inexactitude).
        0b0110 => FpInstr::Rint {
            rounding: if op == 0b11 { 3 } else { ROUND_FPSCR },
            exact: false,
            d,
            m,
        },
        0b0111 if op == 0b01 => FpInstr::Rint {
            rounding: ROUND_FPSCR,
            exact: true,
            d,
            m,
        },
        // vcvt virgule fixe : opc2 = 1x1U (x : 1 vers la virgule fixe), taille en bit 7, `imm4:i` en 3-0 et 5.
        0b1010 | 0b1011 | 0b1110 | 0b1111 => {
            let wide = bit(b, 7);
            let size = if wide { 32 } else { 16 };
            let imm5 = (bits(b, 3, 0) << 1) | bits(b, 5, 5);
            if imm5 > size {
                return None;
            }
            let (signed, fbits) = (opc2 & 1 == 0, size - imm5);
            if bit(a, 2) {
                FpInstr::ToFixed {
                    signed,
                    wide,
                    fbits,
                    d,
                }
            } else {
                FpInstr::FromFixed {
                    signed,
                    wide,
                    fbits,
                    d,
                }
            }
        }
        0b0100 => FpInstr::Cmp {
            d,
            m: Some(m),
            signal_nan: bit(b, 7),
        },
        0b0101 => FpInstr::Cmp {
            d,
            m: None,
            signal_nan: bit(b, 7),
        },
        // vcvt entier -> flottant : op (bit 7) = 1 pour un entier signé.
        0b1000 => FpInstr::FromInt {
            signed: bit(b, 7),
            d,
            m,
        },
        // vcvt flottant -> entier : opc2 = 110s ; bit 7 : arrondi vers zéro (sinon FPSCR).
        0b1100 | 0b1101 => FpInstr::ToInt {
            signed: opc2 & 1 == 1,
            round_zero: bit(b, 7),
            d,
            m,
        },
        _ => return None,
    })
}

/// Transferts entre registres entiers et registres de contrôle ou simples.
fn decode_core_transfer(a: u32, b: u32, single: impl Fn(u32, u32) -> u8) -> Option<FpInstr> {
    let rt = bits(b, 15, 12) as Reg;
    match (bits(a, 7, 4), bits(a, 3, 0)) {
        // vmov Sn <-> Rt : 1110 1110 000 op Vn / Rt 1010 N 001 0000.
        (op, vn) if op & 0b1110 == 0 && bits(b, 6, 0) & 0b1011111 == 0b0010000 => {
            Some(FpInstr::MovCore {
                to_core: op & 1 == 1,
                rt,
                s: single(vn, bits(b, 7, 7)),
            })
        }
        // vmsr fpscr, Rt / vmrs Rt, fpscr.
        (0b1110, 0b0001) if bits(b, 11, 0) == 0b1010_0001_0000 => Some(FpInstr::Vmsr { rt }),
        (0b1111, 0b0001) if bits(b, 11, 0) == 0b1010_0001_0000 => Some(FpInstr::Vmrs { rt }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Arithmétique flottante à la manière d'ARM
// ---------------------------------------------------------------------------------------------

pub(super) const FPSCR_IOC: u32 = 1 << 0;
pub(super) const FPSCR_DZC: u32 = 1 << 1;
pub(super) const FPSCR_OFC: u32 = 1 << 2;
pub(super) const FPSCR_UFC: u32 = 1 << 3;
pub(super) const FPSCR_IXC: u32 = 1 << 4;
pub(super) const FPSCR_IDC: u32 = 1 << 7;
/// Mise à zéro des dénormalisés.
pub(super) const FPSCR_FZ: u32 = 1 << 24;
/// Mode « NaN par défaut ».
pub(super) const FPSCR_DN: u32 = 1 << 25;
/// Mode d'arrondi (bits 23-22).
pub(super) const FPSCR_RMODE_SHIFT: u32 = 22;
const DEFAULT_NAN: u32 = 0x7fc0_0000;
/// Code d'arrondi « au plus proche, égalité à l'écart de zéro » (en plus de 0 à 3, comme FPSCR).
pub(super) const ROUND_AWAY: u32 = 4;
/// Code d'arrondi « celui de FPSCR » pour `Rint`.
pub(super) const ROUND_FPSCR: u32 = 5;

/// Flottant suivant dans le sens des valeurs croissantes.
pub(super) fn next_up(x: f32) -> f32 {
    if x.is_nan() || x == f32::INFINITY {
        return x;
    }
    if x == 0.0 {
        return f32::from_bits(1);
    }
    let b = x.to_bits();
    f32::from_bits(if x > 0.0 { b + 1 } else { b - 1 })
}

pub(super) fn next_down(x: f32) -> f32 {
    -next_up(-x)
}

fn is_snan(x: f32) -> bool {
    x.is_nan() && x.to_bits() & 0x0040_0000 == 0
}

/// Contexte de calcul : les bits de FPSCR qui influencent le résultat et les drapeaux à lever.
struct Env<'a> {
    fpscr: &'a mut u32,
}

impl Env<'_> {
    fn raise(&mut self, flags: u32) {
        *self.fpscr |= flags;
    }

    fn flush_zero(&self) -> bool {
        *self.fpscr & FPSCR_FZ != 0
    }

    /// Opérande d'entrée : un dénormalisé est remplacé par zéro quand `FZ` est actif.
    fn input(&mut self, x: f32) -> f32 {
        if self.flush_zero() && x != 0.0 && !x.is_normal() && x.is_finite() {
            self.raise(FPSCR_IDC);
            return if x.is_sign_negative() { -0.0 } else { 0.0 };
        }
        x
    }

    /// NaN résultant : celui de l'opérande (rendu silencieux) ou le NaN par défaut.
    fn nan_result(&mut self, x: f32) -> f32 {
        if *self.fpscr & FPSCR_DN != 0 {
            f32::from_bits(DEFAULT_NAN)
        } else {
            f32::from_bits(x.to_bits() | 0x0040_0000)
        }
    }

    /// Traitement des NaN en entrée : un NaN signalant prime et lève « opération invalide », sinon
    /// un NaN silencieux est propagé. Les opérandes sont examinés dans l'ordre donné.
    fn process_nans(&mut self, operands: &[f32]) -> Option<f32> {
        if let Some(&x) = operands.iter().find(|&&x| is_snan(x)) {
            self.raise(FPSCR_IOC);
            return Some(self.nan_result(x));
        }
        operands
            .iter()
            .find(|&&x| x.is_nan())
            .map(|&x| self.nan_result(x))
    }

    fn default_nan(&mut self) -> f32 {
        self.raise(FPSCR_IOC);
        f32::from_bits(DEFAULT_NAN)
    }

    /// Mode d'arrondi de FPSCR (0 au plus proche, 1 vers +inf, 2 vers -inf, 3 vers zéro).
    fn rounding_mode(&self) -> u32 {
        (*self.fpscr >> FPSCR_RMODE_SHIFT) & 3
    }

    /// Corrige un résultat arrondi au plus proche pour les modes d'arrondi dirigés.
    /// `residual` est la valeur exacte moins `r` (seul son signe compte).
    fn directed(&mut self, r: f32, residual: f64, inputs_finite: bool) -> f32 {
        let mode = self.rounding_mode();
        if mode == 0 || !inputs_finite {
            return r;
        }
        if r.is_infinite() {
            let to_max = match mode {
                1 => r < 0.0,
                2 => r > 0.0,
                _ => true,
            };
            if !to_max {
                return r;
            }
            self.raise(FPSCR_OFC | FPSCR_IXC);
            return if r > 0.0 { f32::MAX } else { -f32::MAX };
        }
        let up = match mode {
            1 => residual > 0.0,
            3 => r < 0.0 && residual > 0.0,
            _ => false,
        };
        let down = match mode {
            2 => residual < 0.0,
            3 => r > 0.0 && residual < 0.0,
            _ => false,
        };
        if up {
            next_up(r)
        } else if down {
            next_down(r)
        } else {
            r
        }
    }

    /// Finalise un résultat fini : débordement, soupassement, inexactitude, mise à zéro en sortie.
    fn finish(&mut self, result: f32, inexact: bool, inputs_finite: bool) -> f32 {
        if inputs_finite && result.is_infinite() {
            self.raise(FPSCR_OFC | FPSCR_IXC);
            return result;
        }
        let tiny = result != 0.0 && !result.is_normal() && result.is_finite();
        if (tiny || (result == 0.0 && inexact)) && self.flush_zero() {
            self.raise(FPSCR_UFC);
            return if result.is_sign_negative() { -0.0 } else { 0.0 };
        }
        if inexact {
            if tiny || result == 0.0 {
                self.raise(FPSCR_UFC);
            }
            self.raise(FPSCR_IXC);
        }
        result
    }

    fn add(&mut self, a: f32, b: f32, subtract: bool) -> f32 {
        let (a, b) = (self.input(a), self.input(b));
        if let Some(n) = self.process_nans(&[a, b]) {
            return n;
        }
        let b = if subtract { -b } else { b };
        if a == 0.0 && b == 0.0 && a.is_sign_negative() != b.is_sign_negative() {
            // Zéros de signes opposés : +0, sauf en arrondi vers -infini.
            return if self.rounding_mode() == 2 { -0.0 } else { 0.0 };
        }
        if a.is_infinite() && b.is_infinite() && a.is_sign_negative() != b.is_sign_negative() {
            return self.default_nan();
        }
        let result = a + b;
        // Somme sans erreur d'arrondi (TwoSum) dans f64 pour savoir si le résultat est exact.
        let (x, y) = (f64::from(a), f64::from(b));
        let sum = x + y;
        let bb = sum - x;
        let error = (x - (sum - bb)) + (y - bb);
        let inexact = f64::from(result) != sum || error != 0.0;
        let finite = a.is_finite() && b.is_finite();
        if result == 0.0 && finite && a != 0.0 && !inexact {
            // Somme exactement nulle : +0, sauf en arrondi vers -infini.
            return if a == -b && self.rounding_mode() != 2 {
                0.0
            } else if a == -b {
                -0.0
            } else {
                result
            };
        }
        let result = if result.is_finite() {
            self.directed(result, (sum - f64::from(result)) + error, finite)
        } else {
            self.directed(result, 0.0, finite)
        };
        self.finish(result, inexact && finite, finite)
    }

    fn mul(&mut self, a: f32, b: f32) -> f32 {
        let (a, b) = (self.input(a), self.input(b));
        if let Some(n) = self.process_nans(&[a, b]) {
            return n;
        }
        if (a.is_infinite() && b == 0.0) || (a == 0.0 && b.is_infinite()) {
            return self.default_nan();
        }
        let result = a * b;
        let exact = f64::from(a) * f64::from(b);
        let finite = a.is_finite() && b.is_finite();
        let residual = if result.is_finite() {
            exact - f64::from(result)
        } else {
            0.0
        };
        let inexact = finite && f64::from(result) != exact;
        let result = self.directed(result, residual, finite);
        self.finish(result, inexact, finite)
    }

    fn div(&mut self, a: f32, b: f32) -> f32 {
        let (a, b) = (self.input(a), self.input(b));
        if let Some(n) = self.process_nans(&[a, b]) {
            return n;
        }
        if (a == 0.0 && b == 0.0) || (a.is_infinite() && b.is_infinite()) {
            return self.default_nan();
        }
        if b == 0.0 && a.is_finite() {
            self.raise(FPSCR_DZC);
            return if a.is_sign_negative() != b.is_sign_negative() {
                f32::NEG_INFINITY
            } else {
                f32::INFINITY
            };
        }
        let result = a / b;
        let finite = a.is_finite() && b.is_finite();
        let inexact = finite && b != 0.0 && f64::from(result) * f64::from(b) != f64::from(a);
        let residual = if finite && result.is_finite() {
            (f64::from(a) - f64::from(result) * f64::from(b)) * f64::from(b).signum()
        } else {
            0.0
        };
        let result = self.directed(result, residual, finite);
        self.finish(result, inexact, finite)
    }

    fn sqrt(&mut self, a: f32) -> f32 {
        let a = self.input(a);
        if let Some(n) = self.process_nans(&[a]) {
            return n;
        }
        if a < 0.0 {
            return self.default_nan();
        }
        let result = a.sqrt();
        let inexact = a.is_finite() && f64::from(result) * f64::from(result) != f64::from(a);
        let residual = f64::from(a) - f64::from(result) * f64::from(result);
        let result = self.directed(result, residual, a.is_finite());
        self.finish(result, inexact, a.is_finite())
    }

    /// `addend + a × b` avec un seul arrondi.
    fn fused(&mut self, addend: f32, a: f32, b: f32) -> f32 {
        let (addend, a, b) = (self.input(addend), self.input(a), self.input(b));
        if let Some(n) = self.process_nans(&[addend, a, b]) {
            return n;
        }
        if (a.is_infinite() && b == 0.0) || (a == 0.0 && b.is_infinite()) {
            return self.default_nan();
        }
        let product_inf = a.is_infinite() || b.is_infinite();
        if product_inf && addend.is_infinite() {
            let product_negative = a.is_sign_negative() != b.is_sign_negative();
            if product_negative != addend.is_sign_negative() {
                return self.default_nan();
            }
        }
        let result = a.mul_add(b, addend);
        // Le produit de deux f32 est exact dans f64 ; la somme est vérifiée par TwoSum.
        let (x, y) = (f64::from(a) * f64::from(b), f64::from(addend));
        let sum = x + y;
        let bb = sum - x;
        let error = (x - (sum - bb)) + (y - bb);
        let finite = a.is_finite() && b.is_finite() && addend.is_finite();
        let inexact = f64::from(result) != sum || error != 0.0;
        let residual = if result.is_finite() {
            (sum - f64::from(result)) + error
        } else {
            0.0
        };
        if finite && result == 0.0 && !inexact {
            // Somme exactement nulle : +0, sauf en arrondi vers -infini ; trois zéros de même signe
            // gardent leur signe.
            let product_negative = a.is_sign_negative() != b.is_sign_negative();
            let all_zero = (a == 0.0 || b == 0.0) && addend == 0.0;
            if !(all_zero && product_negative == addend.is_sign_negative()) {
                return if self.rounding_mode() == 2 { -0.0 } else { 0.0 };
            }
        }
        let result = self.directed(result, residual, finite);
        self.finish(result, finite && inexact, finite)
    }

    /// Comparaison : renvoie les indicateurs (N, Z, C, V) pour FPSCR.
    fn compare(&mut self, a: f32, b: f32, signal_nan: bool) -> u32 {
        let (a, b) = (self.input(a), self.input(b));
        if a.is_nan() || b.is_nan() {
            if signal_nan || is_snan(a) || is_snan(b) {
                self.raise(FPSCR_IOC);
            }
            return 0b0011; // non ordonné : C et V
        }
        if a == b {
            0b0110 // égal : Z et C
        } else if a < b {
            0b1000 // inférieur : N
        } else {
            0b0010 // supérieur : C
        }
    }

    /// `vrint*` : arrondi à un entier. `rounding` : 0 à 3 comme `RMode`, `ROUND_AWAY` ou `ROUND_FPSCR`.
    fn round_to_integral(&mut self, x: f32, rounding: u32, exact: bool) -> f32 {
        let x = self.input(x);
        if let Some(n) = self.process_nans(&[x]) {
            return n;
        }
        let rounding = if rounding == ROUND_FPSCR {
            (*self.fpscr >> FPSCR_RMODE_SHIFT) & 3
        } else {
            rounding
        };
        let rounded = match rounding {
            0 => x.round_ties_even(),
            1 => x.ceil(),
            2 => x.floor(),
            ROUND_AWAY => x.round(),
            _ => x.trunc(),
        };
        if exact && rounded != x {
            self.raise(FPSCR_IXC);
        }
        rounded
    }

    /// `vmaxnm` / `vminnm` : un NaN silencieux est ignoré s'il n'y en a qu'un ; -0 < +0.
    fn max_min_num(&mut self, a: f32, b: f32, max: bool) -> f32 {
        let (a, b) = (self.input(a), self.input(b));
        let quiet = |x: f32| x.is_nan() && !is_snan(x);
        let (a, b) = match (quiet(a), quiet(b)) {
            (true, false) if !b.is_nan() => (b, b),
            (false, true) if !a.is_nan() => (a, a),
            _ => (a, b),
        };
        if let Some(n) = self.process_nans(&[a, b]) {
            return n;
        }
        if a == b {
            // Deux zéros de signes différents : le maximum est +0, le minimum -0.
            let negative = if max {
                a.is_sign_negative() && b.is_sign_negative()
            } else {
                a.is_sign_negative() || b.is_sign_negative()
            };
            return if a == 0.0 {
                if negative { -0.0 } else { 0.0 }
            } else {
                a
            };
        }
        if (a > b) == max { a } else { b }
    }

    /// Conversion flottant → virgule fixe (vers zéro, avec saturation) ; le résultat est étendu à 32 bits.
    fn convert_to_fixed(&mut self, x: f32, signed: bool, wide: bool, fbits: u32) -> u32 {
        let x = self.input(x);
        if x.is_nan() {
            self.raise(FPSCR_IOC);
            return 0;
        }
        let size = if wide { 32 } else { 16 };
        let (min, max) = if signed {
            (-(2f64.powi(size - 1)), 2f64.powi(size - 1) - 1.0)
        } else {
            (0.0, 2f64.powi(size) - 1.0)
        };
        let scaled = f64::from(x) * 2f64.powi(fbits as i32);
        let truncated = scaled.trunc();
        let saturated = truncated.clamp(min, max);
        if truncated != saturated {
            self.raise(FPSCR_IOC);
        } else if truncated != scaled {
            self.raise(FPSCR_IXC);
        }
        if signed {
            (saturated as i64) as u32
        } else {
            saturated as u32
        }
    }

    /// Conversion flottant → entier de 32 bits avec saturation.
    fn convert_to_int(&mut self, x: f32, signed: bool, rounding: u32) -> u32 {
        let x = self.input(x);
        if x.is_nan() {
            self.raise(FPSCR_IOC);
            return 0;
        }
        let rounded = match rounding {
            0 => x.round_ties_even(), // au plus proche
            1 => x.ceil(),            // vers +infini
            2 => x.floor(),           // vers -infini
            ROUND_AWAY => x.round(),  // au plus proche, égalité à l'écart de zéro
            _ => x.trunc(),           // vers zéro
        };
        let (min, max) = if signed {
            (-2147483648.0f64, 2147483647.0f64)
        } else {
            (0.0, 4294967295.0)
        };
        let wide = f64::from(rounded);
        let saturated = wide.clamp(min, max);
        if wide != saturated {
            self.raise(FPSCR_IOC);
        } else if rounded != x {
            self.raise(FPSCR_IXC);
        }
        if signed {
            (saturated as i32) as u32
        } else {
            saturated as u32
        }
    }
}

impl Cpu {
    /// Exécute une instruction à virgule flottante.
    pub(super) fn exec_fp<B: Bus>(
        &mut self,
        pc: u32,
        pc4: u32,
        instr: FpInstr,
        bus: &mut B,
    ) -> Result<(), Stop> {
        // Toute instruction flottante marque le contexte flottant comme actif (pour l'empilement
        // lors d'une exception).
        self.control |= 1 << 2;
        let read = |cpu: &Cpu, r: Reg| match r {
            15 => pc4,
            13 => cpu.regs.sp,
            14 => cpu.regs.lr,
            n => cpu.regs.r[n as usize],
        };
        let f = |cpu: &Cpu, i: u8| f32::from_bits(cpu.regs.s[i as usize]);
        match instr {
            FpInstr::Arith { op, d, n, m } => {
                let (dv, nv, mv) = (f(self, d), f(self, n), f(self, m));
                let mut env = Env {
                    fpscr: &mut self.regs.fpscr,
                };
                let result = match op {
                    FpArith::Vadd => env.add(nv, mv, false),
                    FpArith::Vsub => env.add(nv, mv, true),
                    FpArith::Vmul => env.mul(nv, mv),
                    FpArith::Vnmul => negate(env.mul(nv, mv)),
                    FpArith::Vdiv => env.div(nv, mv),
                    FpArith::Vmla => {
                        let p = env.mul(nv, mv);
                        env.add(dv, p, false)
                    }
                    FpArith::Vmls => {
                        let p = env.mul(nv, mv);
                        env.add(dv, negate(p), false)
                    }
                    FpArith::Vnmla => {
                        let p = env.mul(nv, mv);
                        env.add(negate(dv), negate(p), false)
                    }
                    FpArith::Vnmls => {
                        let p = env.mul(nv, mv);
                        env.add(negate(dv), p, false)
                    }
                    FpArith::Vfma => env.fused(dv, nv, mv),
                    FpArith::Vfms => env.fused(dv, negate(nv), mv),
                    FpArith::Vfnma => env.fused(negate(dv), negate(nv), mv),
                    FpArith::Vfnms => env.fused(negate(dv), nv, mv),
                };
                self.regs.s[d as usize] = result.to_bits();
            }
            FpInstr::Unary { op, d, m } => {
                let x = f(self, m);
                let result = match op {
                    FpUnary::Vmov => x.to_bits(),
                    FpUnary::Vabs => x.to_bits() & 0x7fff_ffff,
                    FpUnary::Vneg => x.to_bits() ^ 0x8000_0000,
                    FpUnary::Vsqrt => Env {
                        fpscr: &mut self.regs.fpscr,
                    }
                    .sqrt(x)
                    .to_bits(),
                };
                self.regs.s[d as usize] = result;
            }
            FpInstr::MovImm { d, bits } => self.regs.s[d as usize] = bits,
            FpInstr::Cmp { d, m, signal_nan } => {
                let (a, b) = (f(self, d), m.map_or(0.0, |m| f(self, m)));
                let flags = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .compare(a, b, signal_nan);
                self.regs.fpscr = (self.regs.fpscr & 0x0fff_ffff) | (flags << 28);
            }
            FpInstr::ToInt {
                signed,
                round_zero,
                d,
                m,
            } => {
                let rounding = if round_zero {
                    3
                } else {
                    (self.regs.fpscr >> FPSCR_RMODE_SHIFT) & 3
                };
                let x = f(self, m);
                let value = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .convert_to_int(x, signed, rounding);
                self.regs.s[d as usize] = value;
            }
            FpInstr::Double(instr) => self.exec_double(instr),
            FpInstr::ToIntRounded {
                signed,
                rounding,
                d,
                m,
            } => {
                let x = f(self, m);
                let value = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .convert_to_int(x, signed, rounding);
                self.regs.s[d as usize] = value;
            }
            FpInstr::Rint {
                rounding,
                exact,
                d,
                m,
            } => {
                let x = f(self, m);
                let result = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .round_to_integral(x, rounding, exact);
                self.regs.s[d as usize] = result.to_bits();
            }
            FpInstr::Vsel { cond, d, n, m } => {
                let regs = &self.regs;
                let taken = match cond {
                    0 => regs.z(),
                    1 => regs.v(),
                    2 => regs.n() == regs.v(),
                    _ => !regs.z() && regs.n() == regs.v(),
                };
                let source = if taken { n } else { m };
                self.regs.s[d as usize] = self.regs.s[source as usize];
            }
            FpInstr::MaxMinNum { max, d, n, m } => {
                let (nv, mv) = (f(self, n), f(self, m));
                let result = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .max_min_num(nv, mv, max);
                self.regs.s[d as usize] = result.to_bits();
            }
            FpInstr::ToFixed {
                signed,
                wide,
                fbits,
                d,
            } => {
                let x = f(self, d);
                let value = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .convert_to_fixed(x, signed, wide, fbits);
                self.regs.s[d as usize] = value;
            }
            FpInstr::FromFixed {
                signed,
                wide,
                fbits,
                d,
            } => {
                let raw = self.regs.s[d as usize];
                let value: i64 = match (signed, wide) {
                    (true, true) => i64::from(raw as i32),
                    (true, false) => i64::from(raw as u16 as i16),
                    (false, true) => i64::from(raw),
                    (false, false) => i64::from(raw as u16),
                };
                let result = value as f32;
                if f64::from(result) != value as f64 {
                    self.regs.fpscr |= FPSCR_IXC;
                }
                self.regs.s[d as usize] = (result * 2f32.powi(-(fbits as i32))).to_bits();
            }
            FpInstr::FromInt { signed, d, m } => {
                let raw = self.regs.s[m as usize];
                let (result, exact) = if signed {
                    let v = raw as i32;
                    let r = v as f32;
                    (r, r as f64 == f64::from(v))
                } else {
                    let r = raw as f32;
                    (r, r as f64 == f64::from(raw))
                };
                if !exact {
                    self.regs.fpscr |= FPSCR_IXC;
                }
                self.regs.s[d as usize] = result.to_bits();
            }
            FpInstr::Vmrs { rt } => {
                if rt == 15 {
                    self.regs.set_n(self.regs.fpscr >> 31 != 0);
                    self.regs.set_z(self.regs.fpscr >> 30 & 1 != 0);
                    self.regs.set_c(self.regs.fpscr >> 29 & 1 != 0);
                    self.regs.set_v(self.regs.fpscr >> 28 & 1 != 0);
                } else {
                    let value = self.regs.fpscr;
                    self.write_core(rt, value);
                }
            }
            FpInstr::Vmsr { rt } => self.regs.fpscr = read(self, rt),
            FpInstr::MovCore { to_core, rt, s } => {
                if to_core {
                    let value = self.regs.s[s as usize];
                    self.write_core(rt, value);
                } else {
                    self.regs.s[s as usize] = read(self, rt);
                }
            }
            FpInstr::MovTwoCore {
                to_core,
                rt,
                rt2,
                first,
            } => {
                if to_core {
                    let (lo, hi) = (self.regs.s[first as usize], self.regs.s[first as usize + 1]);
                    self.write_core(rt, lo);
                    self.write_core(rt2, hi);
                } else {
                    self.regs.s[first as usize] = read(self, rt);
                    self.regs.s[first as usize + 1] = read(self, rt2);
                }
            }
            FpInstr::Ldr {
                first,
                words,
                rn,
                offset,
            } => {
                let base = if rn == 15 { pc4 & !3 } else { read(self, rn) };
                let address = base.wrapping_add(offset as u32);
                if address & 3 != 0 {
                    return Err(Stop::UsageFault {
                        pc,
                        kind: UsageFault::Unaligned,
                    });
                }
                for w in 0..u32::from(words) {
                    let value = bus
                        .read32(address.wrapping_add(4 * w))
                        .map_err(Stop::BusFault)?;
                    self.regs.s[first as usize + w as usize] = value;
                }
            }
            FpInstr::Str {
                first,
                words,
                rn,
                offset,
            } => {
                let base = if rn == 15 { pc4 & !3 } else { read(self, rn) };
                let address = base.wrapping_add(offset as u32);
                if address & 3 != 0 {
                    return Err(Stop::UsageFault {
                        pc,
                        kind: UsageFault::Unaligned,
                    });
                }
                for w in 0..u32::from(words) {
                    let value = self.regs.s[first as usize + w as usize];
                    bus.write32(address.wrapping_add(4 * w), value)
                        .map_err(Stop::BusFault)?;
                }
            }
            FpInstr::Ldm {
                first,
                words,
                rn,
                writeback,
                decrement_before,
            }
            | FpInstr::Stm {
                first,
                words,
                rn,
                writeback,
                decrement_before,
            } => {
                let load = matches!(instr, FpInstr::Ldm { .. });
                let base = read(self, rn);
                let bytes = 4 * u32::from(words);
                let start = if decrement_before {
                    base.wrapping_sub(bytes)
                } else {
                    base
                };
                if start & 3 != 0 {
                    return Err(Stop::UsageFault {
                        pc,
                        kind: UsageFault::Unaligned,
                    });
                }
                for w in 0..u32::from(words) {
                    let address = start.wrapping_add(4 * w);
                    let index = first as usize + w as usize;
                    if load {
                        self.regs.s[index] = bus.read32(address).map_err(Stop::BusFault)?;
                    } else {
                        bus.write32(address, self.regs.s[index])
                            .map_err(Stop::BusFault)?;
                    }
                }
                if writeback {
                    let new_base = if decrement_before {
                        start
                    } else {
                        base.wrapping_add(bytes)
                    };
                    self.write_core(rn, new_base);
                }
            }
        }
        Ok(())
    }

    /// Écrit un registre entier (hors PC).
    fn write_core(&mut self, r: Reg, value: u32) {
        match r {
            13 => self.regs.sp = value & !3,
            14 => self.regs.lr = value,
            15 => {}
            n => self.regs.r[n as usize] = value,
        }
    }
}

/// Inverse le signe d'un flottant sans toucher à son contenu (un NaN reste le même NaN).
fn negate(x: f32) -> f32 {
    f32::from_bits(x.to_bits() ^ 0x8000_0000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(fpscr: &mut u32) -> Env<'_> {
        Env { fpscr }
    }

    #[test]
    fn simple_arithmetic_is_exact_and_raises_nothing() {
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).add(1.5, 2.25, false), 3.75);
        assert_eq!(env(&mut fpscr).mul(3.0, 0.5), 1.5);
        assert_eq!(env(&mut fpscr).div(9.0, 3.0), 3.0);
        assert_eq!(env(&mut fpscr).sqrt(16.0), 4.0);
        assert_eq!(fpscr, 0);
    }

    #[test]
    fn inexact_results_raise_ixc() {
        let mut fpscr = 0;
        env(&mut fpscr).div(1.0, 3.0);
        assert_eq!(fpscr, FPSCR_IXC);
        let mut fpscr = 0;
        env(&mut fpscr).add(1.0, 1e-10, false);
        assert_eq!(fpscr, FPSCR_IXC);
    }

    #[test]
    fn division_by_zero_and_invalid_operations() {
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).div(1.0, 0.0), f32::INFINITY);
        assert_eq!(fpscr, FPSCR_DZC);
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).div(0.0, 0.0).to_bits(), DEFAULT_NAN);
        assert_eq!(fpscr, FPSCR_IOC);
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).sqrt(-1.0).to_bits(), DEFAULT_NAN);
        assert_eq!(
            env(&mut fpscr).mul(f32::INFINITY, 0.0).to_bits(),
            DEFAULT_NAN
        );
        assert_eq!(fpscr, FPSCR_IOC);
    }

    #[test]
    fn nan_propagation_follows_arm_rules() {
        let snan = f32::from_bits(0x7f80_0001);
        let qnan = f32::from_bits(0x7fc1_2345);
        let mut fpscr = 0;
        // Un NaN silencieux est propagé tel quel, sans lever d'exception.
        assert_eq!(env(&mut fpscr).add(qnan, 1.0, false).to_bits(), 0x7fc1_2345);
        assert_eq!(fpscr, 0);
        // Un NaN signalant lève IOC et devient silencieux.
        assert_eq!(env(&mut fpscr).add(1.0, snan, false).to_bits(), 0x7fc0_0001);
        assert_eq!(fpscr, FPSCR_IOC);
        // Le signalant l'emporte sur le silencieux, même en deuxième position.
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).mul(qnan, snan).to_bits(), 0x7fc0_0001);
        // En mode « NaN par défaut », le résultat est toujours 0x7fc00000.
        let mut fpscr = FPSCR_DN;
        assert_eq!(env(&mut fpscr).add(qnan, 1.0, false).to_bits(), DEFAULT_NAN);
    }

    #[test]
    fn overflow_and_underflow_flags() {
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).mul(f32::MAX, 2.0), f32::INFINITY);
        assert_eq!(fpscr, FPSCR_OFC | FPSCR_IXC);
        let mut fpscr = 0;
        let tiny = env(&mut fpscr).mul(f32::MIN_POSITIVE, 0.25);
        assert!(tiny != 0.0 && !tiny.is_normal());
        assert_eq!(fpscr, 0, "résultat dénormalisé mais exact");
        let mut fpscr = 0;
        env(&mut fpscr).mul(f32::MIN_POSITIVE, 0.3);
        assert_eq!(fpscr & (FPSCR_UFC | FPSCR_IXC), FPSCR_UFC | FPSCR_IXC);
    }

    #[test]
    fn flush_to_zero_replaces_denormals() {
        let denormal = f32::from_bits(0x0000_0001);
        let mut fpscr = FPSCR_FZ;
        assert_eq!(env(&mut fpscr).add(denormal, 1.0, false), 1.0);
        assert_eq!(fpscr & FPSCR_IDC, FPSCR_IDC);
    }

    #[test]
    fn comparison_flags() {
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).compare(1.0, 1.0, false), 0b0110);
        assert_eq!(env(&mut fpscr).compare(1.0, 2.0, false), 0b1000);
        assert_eq!(env(&mut fpscr).compare(2.0, 1.0, false), 0b0010);
        assert_eq!(env(&mut fpscr).compare(f32::NAN, 1.0, false), 0b0011);
        assert_eq!(fpscr, 0, "vcmp ne signale pas un NaN silencieux");
        env(&mut fpscr).compare(f32::NAN, 1.0, true);
        assert_eq!(fpscr, FPSCR_IOC);
    }

    #[test]
    fn float_to_integer_saturates() {
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).convert_to_int(-1.9, true, 3) as i32, -1);
        assert_eq!(fpscr, FPSCR_IXC);
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).convert_to_int(3e10, true, 3), 0x7fff_ffff);
        assert_eq!(fpscr, FPSCR_IOC);
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).convert_to_int(-5.0, false, 3), 0);
        assert_eq!(fpscr, FPSCR_IOC);
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).convert_to_int(f32::NAN, true, 3), 0);
        assert_eq!(fpscr, FPSCR_IOC);
        // Arrondi au plus proche (pair) et vers les infinis.
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).convert_to_int(2.5, true, 0), 2);
        assert_eq!(env(&mut fpscr).convert_to_int(2.1, true, 1), 3);
        assert_eq!(env(&mut fpscr).convert_to_int(-2.1, true, 2) as i32, -3);
    }

    #[test]
    fn decodes_the_constant_encoding_of_vmov_immediate() {
        // vmov.f32 s0, #1.0 : imm8 = 0x70 ; vmov.f32 s0, #-2.5 : imm8 = 0x84.
        let one = decode(0xeeb7, 0x0a00).unwrap();
        assert_eq!(
            one,
            FpInstr::MovImm {
                d: 0,
                bits: 0x3f80_0000
            }
        );
        let neg = decode(0xeeb8, 0x0a04).unwrap();
        assert_eq!(
            neg,
            FpInstr::MovImm {
                d: 0,
                bits: 0xc020_0000
            }
        );
    }

    #[test]
    fn decodes_vsel_from_real_applications() {
        // vselge.f32 s12, s15, s12 ; vseleq.f32 s14, s13, s14 ; vselgt.f32 s2, s2, s15.
        assert_eq!(
            decode(0xfe27, 0x6a86),
            Some(FpInstr::Vsel {
                cond: 2,
                d: 12,
                n: 15,
                m: 12
            })
        );
        assert_eq!(
            decode(0xfe06, 0x7a87),
            Some(FpInstr::Vsel {
                cond: 0,
                d: 14,
                n: 13,
                m: 14
            })
        );
        assert_eq!(
            decode(0xfe31, 0x1a27),
            Some(FpInstr::Vsel {
                cond: 3,
                d: 2,
                n: 2,
                m: 15
            })
        );
    }

    #[test]
    fn rounding_to_integral_and_max_min_num() {
        let mut fpscr = 0;
        assert_eq!(
            env(&mut fpscr).round_to_integral(2.5, ROUND_AWAY, false),
            3.0
        );
        assert_eq!(env(&mut fpscr).round_to_integral(2.5, 0, false), 2.0);
        assert_eq!(env(&mut fpscr).round_to_integral(-2.5, 2, false), -3.0);
        assert_eq!(env(&mut fpscr).round_to_integral(2.7, 3, false), 2.0);
        let nan = f32::NAN;
        assert_eq!(env(&mut fpscr).max_min_num(nan, 1.0, true), 1.0);
        assert_eq!(env(&mut fpscr).max_min_num(1.0, nan, false), 1.0);
        assert!(env(&mut fpscr).max_min_num(nan, nan, true).is_nan());
        assert_eq!(env(&mut fpscr).max_min_num(-0.0, 0.0, true).to_bits(), 0);
        assert_eq!(
            env(&mut fpscr).max_min_num(-0.0, 0.0, false).to_bits(),
            0x8000_0000
        );
        assert_eq!(env(&mut fpscr).max_min_num(2.0, 3.0, false), 2.0);
    }

    #[test]
    fn fixed_point_conversions() {
        // vcvt.s32.f32 s14, s14, #8 (0xeebe 0x7acc) lu dans une application réelle.
        assert_eq!(
            decode(0xeebe, 0x7acc),
            Some(FpInstr::ToFixed {
                signed: true,
                wide: true,
                fbits: 8,
                d: 14
            })
        );
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).convert_to_fixed(1.5, true, true, 8), 384);
        assert_eq!(
            env(&mut fpscr).convert_to_fixed(-1.5, true, true, 8) as i32,
            -384
        );
        assert_eq!(
            env(&mut fpscr).convert_to_fixed(1000.0, false, false, 0),
            1000
        );
        assert_eq!(fpscr, 0);
        assert_eq!(
            env(&mut fpscr).convert_to_fixed(1e9, true, false, 0),
            0x7fff
        );
        assert_ne!(fpscr & FPSCR_IOC, 0);
    }
}
