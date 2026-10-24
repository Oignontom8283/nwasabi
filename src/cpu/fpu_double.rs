//! Double précision (D0 à D15) et demi-précision (`vcvtb`, `vcvtt`) du FPv5.
//!
//! Un registre double `Dk` occupe `S(2k)` (mot de poids faible) et `S(2k+1)` ; les numéros `d`, `n`,
//! `m` des instructions de ce module désignent, selon l'opérande, un registre simple (0 à 31) ou
//! le registre simple de poids faible d'un double (0, 2, … 30).
//!
//! L'arithmétique suit les mêmes règles que la simple précision (voir `fpu.rs`) : propagation des
//! NaN, NaN par défaut, mise à zéro des dénormalisés, indicateurs cumulés. Pour savoir si un
//! résultat est exact sans disposer d'un type plus large, les erreurs d'arrondi sont calculées
//! exactement (TwoSum, multiplication-addition fusionnée).
//!
//! Non géré : le format de demi-précision alternatif (`FPSCR.AHP`).

use super::Cpu;
use super::fpu::{
    FPSCR_DN, FPSCR_DZC, FPSCR_FZ, FPSCR_IDC, FPSCR_IOC, FPSCR_IXC, FPSCR_OFC, FPSCR_RMODE_SHIFT,
    FPSCR_UFC, FpArith, FpUnary, ROUND_AWAY, ROUND_FPSCR, next_down, next_up,
};

/// Instruction en double ou demi-précision décodée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DInstr {
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
    /// `vmov.f64 Dd, #constante` (bits de la constante déjà développés).
    MovImm {
        d: u8,
        bits: u64,
    },
    /// `vcmp` / `vcmpe` ; `m` à `None` : comparaison avec zéro.
    Cmp {
        d: u8,
        m: Option<u8>,
        signal_nan: bool,
    },
    /// Double → entier ; `d` est un registre simple, `m` un double.
    ToInt {
        signed: bool,
        round_zero: bool,
        d: u8,
        m: u8,
    },
    /// Double → entier avec arrondi imposé (`vcvta`, `vcvtn`, `vcvtp`, `vcvtm`).
    ToIntRounded {
        signed: bool,
        rounding: u32,
        d: u8,
        m: u8,
    },
    /// Entier → double ; `d` est un double, `m` un registre simple.
    FromInt {
        signed: bool,
        d: u8,
        m: u8,
    },
    /// `vcvt.f32.f64 Sd, Dm`.
    ToSingle {
        d: u8,
        m: u8,
    },
    /// `vcvt.f64.f32 Dd, Sm`.
    FromSingle {
        d: u8,
        m: u8,
    },
    /// Double → virgule fixe, en place (résultat dans le mot de poids faible).
    ToFixed {
        signed: bool,
        wide: bool,
        fbits: u32,
        d: u8,
    },
    /// Virgule fixe (mot de poids faible) → double, en place.
    FromFixed {
        signed: bool,
        wide: bool,
        fbits: u32,
        d: u8,
    },
    Rint {
        rounding: u32,
        exact: bool,
        d: u8,
        m: u8,
    },
    Vsel {
        cond: u8,
        d: u8,
        n: u8,
        m: u8,
    },
    MaxMinNum {
        max: bool,
        d: u8,
        n: u8,
        m: u8,
    },
    /// `vcvtb` / `vcvtt` : demi-précision (moitié basse ou haute d'un registre simple) vers ou
    /// depuis un simple (`double` faux) ou un double.
    Half {
        top: bool,
        to_half: bool,
        double: bool,
        d: u8,
        m: u8,
    },
}

fn bits(v: u32, hi: u32, lo: u32) -> u32 {
    (v >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

fn bit(v: u32, n: u32) -> bool {
    (v >> n) & 1 != 0
}

fn single(hi: u32, low_bit: u32) -> u8 {
    (hi << 1 | low_bit) as u8
}

/// Registre double `D(low_bit:hi)` ; seuls D0 à D15 existent.
fn dreg(hi: u32, low_bit: u32) -> Option<u8> {
    (low_bit == 0).then_some((2 * hi) as u8)
}

/// `vcvtb` et `vcvtt` : `1110 1110 1 D 11 001 op / Vd 101 sz T 1 M 0 Vm`.
pub(super) fn decode_half(a: u32, b: u32) -> Option<DInstr> {
    if bits(b, 11, 9) != 0b101 || !bit(b, 6) || bit(b, 4) {
        return None;
    }
    let (double, to_half) = (bit(b, 8), bit(a, 0));
    let (vd, d_bit) = (bits(b, 15, 12), bits(a, 6, 6));
    let (vm, m_bit) = (bits(b, 3, 0), bits(b, 5, 5));
    let (d, m) = match (double, to_half) {
        // Demi → simple : Sd, Sm.  Simple → demi : Sd, Sm.
        (false, _) => (single(vd, d_bit), single(vm, m_bit)),
        // Demi → double : Dd, Sm.
        (true, false) => (dreg(vd, d_bit)?, single(vm, m_bit)),
        // Double → demi : Sd, Dm.
        (true, true) => (single(vd, d_bit), dreg(vm, m_bit)?),
    };
    Some(DInstr::Half {
        top: bit(b, 7),
        to_half,
        double,
        d,
        m,
    })
}

/// Traitement de données en double précision (`1110 1110 ... 1011 ... 0`).
pub(super) fn decode_data(a: u32, b: u32) -> Option<DInstr> {
    let (vd, d_bit) = (bits(b, 15, 12), bits(a, 6, 6));
    let (vn, n_bit) = (bits(a, 3, 0), bits(b, 7, 7));
    let (vm, m_bit) = (bits(b, 3, 0), bits(b, 5, 5));
    let opc1 = bits(a, 7, 4);
    let s = bit(b, 6);
    let (p, q, r) = (bit(opc1, 3), bit(opc1, 1), bit(opc1, 0));
    let dd = dreg(vd, d_bit);
    if !(p && q && r) {
        let (d, n, m) = (dd?, dreg(vn, n_bit)?, dreg(vm, m_bit)?);
        let op = match (p, q, r, s) {
            (false, false, false, false) => FpArith::Vmla,
            (false, false, false, true) => FpArith::Vmls,
            (false, false, true, false) => FpArith::Vnmls,
            (false, false, true, true) => FpArith::Vnmla,
            (false, true, false, false) => FpArith::Vmul,
            (false, true, false, true) => FpArith::Vnmul,
            (false, true, true, false) => FpArith::Vadd,
            (false, true, true, true) => FpArith::Vsub,
            (true, false, false, false) => FpArith::Vdiv,
            (true, false, true, false) => FpArith::Vfnms,
            (true, false, true, true) => FpArith::Vfnma,
            (true, true, false, false) => FpArith::Vfma,
            (true, true, false, true) => FpArith::Vfms,
            _ => return None,
        };
        return Some(DInstr::Arith { op, d, n, m });
    }
    // Groupe « opc1 = 1x11 » : constante, copies, comparaisons, conversions.
    let opc2 = bits(a, 3, 0);
    let op = bits(b, 7, 6);
    if op & 1 == 0 {
        // vmov.f64 Dd, #imm : sign, NOT(b), b répété 8 fois, 6 bits de fraction.
        let imm8 = (bits(a, 3, 0) << 4) | bits(b, 3, 0);
        let sign = u64::from((imm8 >> 7) & 1);
        let not_b = u64::from(((imm8 >> 6) & 1) ^ 1);
        let b_rep = if (imm8 >> 6) & 1 == 1 { 0xff } else { 0 };
        let fraction = u64::from(imm8 & 0x3f);
        let bits = (sign << 63) | (not_b << 62) | (b_rep << 54) | (fraction << 48);
        return Some(DInstr::MovImm { d: dd?, bits });
    }
    let (dm, sm, ss) = (dreg(vm, m_bit), single(vm, m_bit), single(vd, d_bit));
    let unary = |op| Some(DInstr::Unary { op, d: dd?, m: dm? });
    match opc2 {
        0b0000 if op == 0b01 => unary(FpUnary::Vmov),
        0b0000 => unary(FpUnary::Vabs),
        0b0001 if op == 0b01 => unary(FpUnary::Vneg),
        0b0001 => unary(FpUnary::Vsqrt),
        0b0100 => Some(DInstr::Cmp {
            d: dd?,
            m: Some(dm?),
            signal_nan: bit(b, 7),
        }),
        0b0101 => Some(DInstr::Cmp {
            d: dd?,
            m: None,
            signal_nan: bit(b, 7),
        }),
        0b0110 => Some(DInstr::Rint {
            rounding: if op == 0b11 { 3 } else { ROUND_FPSCR },
            exact: false,
            d: dd?,
            m: dm?,
        }),
        0b0111 if op == 0b01 => Some(DInstr::Rint {
            rounding: ROUND_FPSCR,
            exact: true,
            d: dd?,
            m: dm?,
        }),
        // vcvt.f32.f64 Sd, Dm
        0b0111 => Some(DInstr::ToSingle { d: ss, m: dm? }),
        0b1000 => Some(DInstr::FromInt {
            signed: bit(b, 7),
            d: dd?,
            m: sm,
        }),
        0b1100 | 0b1101 => Some(DInstr::ToInt {
            signed: opc2 & 1 == 1,
            round_zero: bit(b, 7),
            d: ss,
            m: dm?,
        }),
        0b1010 | 0b1011 | 0b1110 | 0b1111 => {
            let wide = bit(b, 7);
            let size = if wide { 32 } else { 16 };
            let imm5 = (bits(b, 3, 0) << 1) | bits(b, 5, 5);
            if imm5 > size {
                return None;
            }
            let (signed, fbits, d) = (opc2 & 1 == 0, size - imm5, dd?);
            Some(if bit(a, 2) {
                DInstr::ToFixed {
                    signed,
                    wide,
                    fbits,
                    d,
                }
            } else {
                DInstr::FromFixed {
                    signed,
                    wide,
                    fbits,
                    d,
                }
            })
        }
        _ => None,
    }
}

/// Instructions de FPv5 à préfixe `0xFE` en double précision.
pub(super) fn decode_v8(a: u32, b: u32) -> Option<DInstr> {
    if bit(b, 4) {
        return None;
    }
    let d = dreg(bits(b, 15, 12), bits(a, 6, 6));
    let n = dreg(bits(a, 3, 0), bits(b, 7, 7));
    let m = dreg(bits(b, 3, 0), bits(b, 5, 5));
    // Destination ou source simple des conversions : `Vd:D` et `Vm:M`.
    let sd = single(bits(b, 15, 12), bits(a, 6, 6));
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
        return Some(DInstr::Vsel {
            cond: bits(a, 5, 4) as u8,
            d: d?,
            n: n?,
            m: m?,
        });
    }
    match (bits(a, 5, 4), bits(a, 3, 2)) {
        (0b00, _) => Some(DInstr::MaxMinNum {
            max: !bit(b, 6),
            d: d?,
            n: n?,
            m: m?,
        }),
        (0b11, 0b10) if bits(b, 7, 6) == 0b01 => Some(DInstr::Rint {
            rounding: rounding(bits(a, 1, 0)),
            exact: false,
            d: d?,
            m: m?,
        }),
        (0b11, 0b11) if bit(b, 6) => Some(DInstr::ToIntRounded {
            signed: bit(b, 7),
            rounding: rounding(bits(a, 1, 0)),
            d: sd,
            m: m?,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Arithmétique en double précision à la manière d'ARM
// ---------------------------------------------------------------------------------------------

/// En dessous, les erreurs d'arrondi calculées directement seraient perdues (soupassement).
const TINY: f64 = 1.0e-270;
/// Facteur d'échelle (2^600) pour calculer ces erreurs hors de la zone de soupassement.
const SCALE: f64 = 4.149515568880993e180;
const DEFAULT_NAN: u64 = 0x7ff8_0000_0000_0000;
const QUIET_BIT: u64 = 1 << 51;

fn is_snan(x: f64) -> bool {
    x.is_nan() && x.to_bits() & QUIET_BIT == 0
}

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

    fn input(&mut self, x: f64) -> f64 {
        if self.flush_zero() && x != 0.0 && !x.is_normal() && x.is_finite() {
            self.raise(FPSCR_IDC);
            return if x.is_sign_negative() { -0.0 } else { 0.0 };
        }
        x
    }

    fn nan_result(&mut self, x: f64) -> f64 {
        if *self.fpscr & FPSCR_DN != 0 {
            f64::from_bits(DEFAULT_NAN)
        } else {
            f64::from_bits(x.to_bits() | QUIET_BIT)
        }
    }

    fn process_nans(&mut self, operands: &[f64]) -> Option<f64> {
        if let Some(&x) = operands.iter().find(|&&x| is_snan(x)) {
            self.raise(FPSCR_IOC);
            return Some(self.nan_result(x));
        }
        operands
            .iter()
            .find(|&&x| x.is_nan())
            .map(|&x| self.nan_result(x))
    }

    fn default_nan(&mut self) -> f64 {
        self.raise(FPSCR_IOC);
        f64::from_bits(DEFAULT_NAN)
    }

    fn rounding_mode(&self) -> u32 {
        (*self.fpscr >> FPSCR_RMODE_SHIFT) & 3
    }

    /// Corrige un résultat arrondi au plus proche pour les modes d'arrondi dirigés ;
    /// `residual` est la valeur exacte moins `r` (seul son signe compte).
    fn directed(&mut self, r: f64, residual: f64, inputs_finite: bool) -> f64 {
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
            return if r > 0.0 { f64::MAX } else { -f64::MAX };
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
        let step = |x: f64, up: bool| {
            if x == 0.0 {
                let tiny = f64::from_bits(1);
                return if up { tiny } else { -tiny };
            }
            let b = x.to_bits();
            f64::from_bits(if (x > 0.0) == up { b + 1 } else { b - 1 })
        };
        if up {
            step(r, true)
        } else if down {
            step(r, false)
        } else {
            r
        }
    }

    fn finish(&mut self, result: f64, inexact: bool, inputs_finite: bool) -> f64 {
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

    fn add(&mut self, a: f64, b: f64, subtract: bool) -> f64 {
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
        let finite = a.is_finite() && b.is_finite();
        // TwoSum : l'erreur d'arrondi de la somme, exactement.
        let bb = result - a;
        let error = (a - (result - bb)) + (b - bb);
        let inexact = finite && result.is_finite() && error != 0.0;
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
        let result = self.directed(result, if inexact { error } else { 0.0 }, finite);
        self.finish(result, inexact, finite)
    }

    fn mul(&mut self, a: f64, b: f64) -> f64 {
        let (a, b) = (self.input(a), self.input(b));
        if let Some(n) = self.process_nans(&[a, b]) {
            return n;
        }
        if (a.is_infinite() && b == 0.0) || (a == 0.0 && b.is_infinite()) {
            return self.default_nan();
        }
        let result = a * b;
        let finite = a.is_finite() && b.is_finite();
        let residual = if !(finite && result.is_finite()) {
            0.0
        } else if result.abs() >= TINY || a == 0.0 || b == 0.0 {
            a.mul_add(b, -result)
        } else {
            // Produit proche du soupassement : l'erreur serait perdue, on met tout à l'échelle.
            (a * SCALE).mul_add(b * SCALE, -(result * SCALE * SCALE))
        };
        let inexact = residual != 0.0;
        let result = self.directed(result, residual, finite);
        self.finish(result, inexact, finite)
    }

    fn div(&mut self, a: f64, b: f64) -> f64 {
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
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        let result = a / b;
        let finite = a.is_finite() && b.is_finite();
        let remainder = if !(finite && result.is_finite()) {
            0.0
        } else if result.abs() >= TINY && a.abs() >= TINY {
            (-result).mul_add(b, a)
        } else {
            (-result * SCALE).mul_add(b, a * SCALE)
        };
        let inexact = remainder != 0.0;
        let result = self.directed(result, remainder * b.signum(), finite);
        self.finish(result, inexact, finite)
    }

    fn sqrt(&mut self, a: f64) -> f64 {
        let a = self.input(a);
        if let Some(n) = self.process_nans(&[a]) {
            return n;
        }
        if a < 0.0 {
            return self.default_nan();
        }
        let result = a.sqrt();
        let residual = if !a.is_finite() {
            0.0
        } else if a >= TINY || a == 0.0 {
            (-result).mul_add(result, a)
        } else {
            (-result * SCALE).mul_add(result * SCALE, a * SCALE * SCALE)
        };
        let inexact = residual != 0.0;
        let result = self.directed(result, residual, a.is_finite());
        self.finish(result, inexact, a.is_finite())
    }

    /// `addend + a × b` avec un seul arrondi.
    fn fused(&mut self, addend: f64, a: f64, b: f64) -> f64 {
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
        let finite = a.is_finite() && b.is_finite() && addend.is_finite();
        let mut inexact = false;
        let mut residual = 0.0;
        let product_tiny = a != 0.0 && b != 0.0 && (a * b).abs() < TINY;
        if finite && result.is_finite() {
            if product_tiny && addend.abs() >= 2f64.powi(-176) {
                // Le produit, non nul, est très inférieur à l'addende : il ne fait que décaler le résultat.
                let product_negative = a.is_sign_negative() != b.is_sign_negative();
                residual = if product_negative { -1.0 } else { 1.0 };
                inexact = true;
            } else {
                // Produit exact = p + ep ; somme exacte = s + es ; le résultat est exact si
                // (s − résultat) + es + ep est nul. Près du soupassement, tout est mis à l'échelle.
                let k = if product_tiny { SCALE } else { 1.0 };
                let (a, b, c, r) = (a * k, b * k, addend * k * k, result * k * k);
                let p = a * b;
                let ep = a.mul_add(b, -p);
                let s = p + c;
                let bb = s - p;
                let es = (p - (s - bb)) + (c - bb);
                residual = (s - r) + es + ep;
                inexact = residual != 0.0;
            }
        }
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
        self.finish(result, inexact, finite)
    }

    fn compare(&mut self, a: f64, b: f64, signal_nan: bool) -> u32 {
        let (a, b) = (self.input(a), self.input(b));
        if a.is_nan() || b.is_nan() {
            if signal_nan || is_snan(a) || is_snan(b) {
                self.raise(FPSCR_IOC);
            }
            return 0b0011;
        }
        if a == b {
            0b0110
        } else if a < b {
            0b1000
        } else {
            0b0010
        }
    }

    fn round_with(&self, x: f64, rounding: u32) -> f64 {
        let rounding = if rounding == ROUND_FPSCR {
            (*self.fpscr >> FPSCR_RMODE_SHIFT) & 3
        } else {
            rounding
        };
        match rounding {
            0 => x.round_ties_even(),
            1 => x.ceil(),
            2 => x.floor(),
            ROUND_AWAY => x.round(),
            _ => x.trunc(),
        }
    }

    fn round_to_integral(&mut self, x: f64, rounding: u32, exact: bool) -> f64 {
        let x = self.input(x);
        if let Some(n) = self.process_nans(&[x]) {
            return n;
        }
        let rounded = self.round_with(x, rounding);
        if exact && rounded != x {
            self.raise(FPSCR_IXC);
        }
        rounded
    }

    fn max_min_num(&mut self, a: f64, b: f64, max: bool) -> f64 {
        let (a, b) = (self.input(a), self.input(b));
        let quiet = |x: f64| x.is_nan() && !is_snan(x);
        let (a, b) = match (quiet(a), quiet(b)) {
            (true, false) if !b.is_nan() => (b, b),
            (false, true) if !a.is_nan() => (a, a),
            _ => (a, b),
        };
        if let Some(n) = self.process_nans(&[a, b]) {
            return n;
        }
        if a == b {
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

    /// Double → entier de 32 bits avec saturation.
    fn convert_to_int(&mut self, x: f64, signed: bool, rounding: u32) -> u32 {
        let x = self.input(x);
        if x.is_nan() {
            self.raise(FPSCR_IOC);
            return 0;
        }
        let rounded = self.round_with(x, rounding);
        let (min, max) = if signed {
            (-2147483648.0f64, 2147483647.0f64)
        } else {
            (0.0, 4294967295.0)
        };
        let saturated = rounded.clamp(min, max);
        if rounded != saturated {
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

    /// Double → virgule fixe (vers zéro, saturation), étendu à 32 bits.
    fn convert_to_fixed(&mut self, x: f64, signed: bool, wide: bool, fbits: u32) -> u32 {
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
        let scaled = x * 2f64.powi(fbits as i32);
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

    /// Simple → double (toujours exact ; un NaN garde sa charge utile).
    fn single_to_double(&mut self, x: f32) -> f64 {
        if x.is_nan() {
            if x.to_bits() & 0x0040_0000 == 0 {
                self.raise(FPSCR_IOC);
            }
            if *self.fpscr & FPSCR_DN != 0 {
                return f64::from_bits(DEFAULT_NAN);
            }
            let payload = u64::from(x.to_bits() & 0x007f_ffff) << 29;
            let sign = u64::from(x.to_bits() >> 31) << 63;
            return f64::from_bits(sign | 0x7ff0_0000_0000_0000 | QUIET_BIT | payload);
        }
        // Un dénormalisé simple est normal en double : la mise à zéro se fait avant l'élargissement.
        if self.flush_zero() && x != 0.0 && x.is_subnormal() {
            self.raise(FPSCR_IDC);
            return if x.is_sign_negative() { -0.0 } else { 0.0 };
        }
        f64::from(x)
    }

    /// Double → simple, arrondi selon FPSCR.
    fn double_to_single(&mut self, x: f64) -> f32 {
        if x.is_nan() {
            if is_snan(x) {
                self.raise(FPSCR_IOC);
            }
            if *self.fpscr & FPSCR_DN != 0 {
                return f32::from_bits(0x7fc0_0000);
            }
            let payload = ((x.to_bits() >> 29) & 0x007f_ffff) as u32;
            let sign = ((x.to_bits() >> 63) as u32) << 31;
            return f32::from_bits(sign | 0x7f80_0000 | 0x0040_0000 | payload);
        }
        let x = self.input(x);
        let nearest = x as f32;
        let finite = x.is_finite();
        let residual = if finite && nearest.is_finite() {
            x - f64::from(nearest)
        } else {
            0.0
        };
        let inexact = finite && nearest.is_finite() && residual != 0.0;
        let mode = self.rounding_mode();
        let mut result = nearest;
        if finite && mode != 0 {
            if nearest.is_infinite() {
                let to_max = match mode {
                    1 => nearest < 0.0,
                    2 => nearest > 0.0,
                    _ => true,
                };
                if to_max {
                    self.raise(FPSCR_OFC | FPSCR_IXC);
                    return if nearest > 0.0 { f32::MAX } else { -f32::MAX };
                }
            } else if (mode == 1 && residual > 0.0)
                || (mode == 3 && nearest < 0.0 && residual > 0.0)
            {
                result = next_up(nearest);
            } else if (mode == 2 && residual < 0.0)
                || (mode == 3 && nearest > 0.0 && residual < 0.0)
            {
                result = next_down(nearest);
            }
        }
        if finite && result.is_infinite() {
            self.raise(FPSCR_OFC | FPSCR_IXC);
            return result;
        }
        let tiny = result.is_finite() && result != 0.0 && !result.is_normal();
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

    /// Demi-précision → double (exact).
    fn half_to_double(&mut self, h: u16) -> f64 {
        let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
        let (exp, frac) = (i32::from((h >> 10) & 0x1f), u64::from(h & 0x3ff));
        match exp {
            0 => sign * (frac as f64) * 2f64.powi(-24),
            31 if frac == 0 => sign * f64::INFINITY,
            31 => {
                if frac & 0x200 == 0 {
                    self.raise(FPSCR_IOC);
                }
                if *self.fpscr & FPSCR_DN != 0 {
                    return f64::from_bits(DEFAULT_NAN);
                }
                let s = u64::from(h >> 15) << 63;
                f64::from_bits(s | 0x7ff0_0000_0000_0000 | QUIET_BIT | (frac << 42))
            }
            _ => sign * (1.0 + frac as f64 / 1024.0) * 2f64.powi(exp - 15),
        }
    }

    /// Double → demi-précision (format IEEE), arrondi selon FPSCR.
    fn double_to_half(&mut self, x: f64) -> u16 {
        let x = self.input(x);
        let sign: u16 = if x.is_sign_negative() { 0x8000 } else { 0 };
        if x.is_nan() {
            if is_snan(x) {
                self.raise(FPSCR_IOC);
            }
            if *self.fpscr & FPSCR_DN != 0 {
                return 0x7e00;
            }
            let payload = ((x.to_bits() >> 42) & 0x1ff) as u16;
            return sign | 0x7e00 | payload;
        }
        if x.is_infinite() {
            return sign | 0x7c00;
        }
        let a = x.abs();
        if a == 0.0 {
            return sign;
        }
        let mode = (*self.fpscr >> FPSCR_RMODE_SHIFT) & 3;
        let negative = sign != 0;
        // Pas de quantification : 2^-24 en dessous de 2^-14, sinon 2^(e-10).
        let e = if a < 2f64.powi(-14) {
            -14
        } else {
            ((a.to_bits() >> 52) & 0x7ff) as i32 - 1023
        };
        let quantum = 2f64.powi(e - 10);
        let scaled = a / quantum;
        let mut n = scaled.floor();
        let frac = scaled - n;
        let up = match mode {
            0 => frac > 0.5 || (frac == 0.5 && n % 2.0 == 1.0),
            1 => !negative && frac > 0.0,
            2 => negative && frac > 0.0,
            _ => false,
        };
        if up {
            n += 1.0;
        }
        let inexact = frac != 0.0;
        let (mut e, mut n) = (e, n as u32);
        if n == 2048 {
            n = 1024;
            e += 1;
        }
        // Débordement : au-delà de 65504.
        if e > 15 {
            self.raise(FPSCR_OFC | FPSCR_IXC);
            let to_infinity = match mode {
                0 => true,
                1 => !negative,
                2 => negative,
                _ => false,
            };
            return sign | if to_infinity { 0x7c00 } else { 0x7bff };
        }
        if inexact {
            if a < 2f64.powi(-14) {
                self.raise(FPSCR_UFC);
            }
            self.raise(FPSCR_IXC);
        }
        if n < 1024 {
            // Sous-normal (ou zéro après arrondi).
            return sign | n as u16;
        }
        sign | (((e + 15) as u16) << 10) | (n - 1024) as u16
    }
}

/// Demi-précision → simple (exact).
fn half_to_single(env: &mut Env, h: u16) -> f32 {
    let d = env.half_to_double(h);
    if d.is_nan() {
        let payload = (u32::from(h) & 0x3ff) << 13;
        if *env.fpscr & FPSCR_DN != 0 {
            return f32::from_bits(0x7fc0_0000);
        }
        let sign = u32::from(h >> 15) << 31;
        return f32::from_bits(sign | 0x7f80_0000 | 0x0040_0000 | payload);
    }
    d as f32
}

impl Cpu {
    fn double(&self, i: u8) -> f64 {
        let (lo, hi) = (self.regs.s[i as usize], self.regs.s[i as usize + 1]);
        f64::from_bits(u64::from(lo) | (u64::from(hi) << 32))
    }

    fn set_double(&mut self, i: u8, x: f64) {
        let bits = x.to_bits();
        self.regs.s[i as usize] = bits as u32;
        self.regs.s[i as usize + 1] = (bits >> 32) as u32;
    }

    /// Exécute une instruction en double ou demi-précision.
    pub(super) fn exec_double(&mut self, instr: DInstr) {
        match instr {
            DInstr::Arith { op, d, n, m } => {
                let (dv, nv, mv) = (self.double(d), self.double(n), self.double(m));
                let mut env = Env {
                    fpscr: &mut self.regs.fpscr,
                };
                let result = match op {
                    FpArith::Vadd => env.add(nv, mv, false),
                    FpArith::Vsub => env.add(nv, mv, true),
                    FpArith::Vmul => env.mul(nv, mv),
                    FpArith::Vnmul => -env.mul(nv, mv),
                    FpArith::Vdiv => env.div(nv, mv),
                    FpArith::Vmla => {
                        let p = env.mul(nv, mv);
                        env.add(dv, p, false)
                    }
                    FpArith::Vmls => {
                        let p = env.mul(nv, mv);
                        env.add(dv, -p, false)
                    }
                    FpArith::Vnmla => {
                        let p = env.mul(nv, mv);
                        env.add(-dv, -p, false)
                    }
                    FpArith::Vnmls => {
                        let p = env.mul(nv, mv);
                        env.add(-dv, p, false)
                    }
                    FpArith::Vfma => env.fused(dv, nv, mv),
                    FpArith::Vfms => env.fused(dv, -nv, mv),
                    FpArith::Vfnma => env.fused(-dv, -nv, mv),
                    FpArith::Vfnms => env.fused(-dv, nv, mv),
                };
                self.set_double(d, result);
            }
            DInstr::Unary { op, d, m } => {
                let x = self.double(m);
                let result = match op {
                    FpUnary::Vmov => x,
                    FpUnary::Vabs => f64::from_bits(x.to_bits() & !(1 << 63)),
                    FpUnary::Vneg => f64::from_bits(x.to_bits() ^ (1 << 63)),
                    FpUnary::Vsqrt => Env {
                        fpscr: &mut self.regs.fpscr,
                    }
                    .sqrt(x),
                };
                self.set_double(d, result);
            }
            DInstr::MovImm { d, bits } => self.set_double(d, f64::from_bits(bits)),
            DInstr::Cmp { d, m, signal_nan } => {
                let (a, b) = (self.double(d), m.map_or(0.0, |m| self.double(m)));
                let flags = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .compare(a, b, signal_nan);
                self.regs.fpscr = (self.regs.fpscr & 0x0fff_ffff) | (flags << 28);
            }
            DInstr::ToInt {
                signed,
                round_zero,
                d,
                m,
            } => {
                let rounding = if round_zero { 3 } else { ROUND_FPSCR };
                let x = self.double(m);
                let value = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .convert_to_int(x, signed, rounding);
                self.regs.s[d as usize] = value;
            }
            DInstr::ToIntRounded {
                signed,
                rounding,
                d,
                m,
            } => {
                let x = self.double(m);
                let value = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .convert_to_int(x, signed, rounding);
                self.regs.s[d as usize] = value;
            }
            DInstr::FromInt { signed, d, m } => {
                let raw = self.regs.s[m as usize];
                let value = if signed {
                    f64::from(raw as i32)
                } else {
                    f64::from(raw)
                };
                self.set_double(d, value);
            }
            DInstr::ToSingle { d, m } => {
                let x = self.double(m);
                let result = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .double_to_single(x);
                self.regs.s[d as usize] = result.to_bits();
            }
            DInstr::FromSingle { d, m } => {
                let x = f32::from_bits(self.regs.s[m as usize]);
                let result = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .single_to_double(x);
                self.set_double(d, result);
            }
            DInstr::ToFixed {
                signed,
                wide,
                fbits,
                d,
            } => {
                let x = self.double(d);
                let value = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .convert_to_fixed(x, signed, wide, fbits);
                self.regs.s[d as usize] = value;
            }
            DInstr::FromFixed {
                signed,
                wide,
                fbits,
                d,
            } => {
                let raw = self.regs.s[d as usize];
                let value = match (signed, wide) {
                    (true, true) => f64::from(raw as i32),
                    (true, false) => f64::from(raw as u16 as i16),
                    (false, true) => f64::from(raw),
                    (false, false) => f64::from(raw as u16),
                };
                self.set_double(d, value * 2f64.powi(-(fbits as i32)));
            }
            DInstr::Rint {
                rounding,
                exact,
                d,
                m,
            } => {
                let x = self.double(m);
                let result = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .round_to_integral(x, rounding, exact);
                self.set_double(d, result);
            }
            DInstr::Vsel { cond, d, n, m } => {
                let regs = &self.regs;
                let taken = match cond {
                    0 => regs.z(),
                    1 => regs.v(),
                    2 => regs.n() == regs.v(),
                    _ => !regs.z() && regs.n() == regs.v(),
                };
                let source = if taken { n } else { m };
                let value = self.double(source);
                self.set_double(d, value);
            }
            DInstr::MaxMinNum { max, d, n, m } => {
                let (nv, mv) = (self.double(n), self.double(m));
                let result = Env {
                    fpscr: &mut self.regs.fpscr,
                }
                .max_min_num(nv, mv, max);
                self.set_double(d, result);
            }
            DInstr::Half {
                top,
                to_half,
                double,
                d,
                m,
            } => {
                let shift = if top { 16 } else { 0 };
                let mut env = Env {
                    fpscr: &mut self.regs.fpscr,
                };
                if to_half {
                    let half = if double {
                        let x = f64::from_bits(
                            u64::from(self.regs.s[m as usize])
                                | (u64::from(self.regs.s[m as usize + 1]) << 32),
                        );
                        env.double_to_half(x)
                    } else {
                        let x = f32::from_bits(self.regs.s[m as usize]);
                        let wide = env.single_to_double(x);
                        env.double_to_half(wide)
                    };
                    let old = self.regs.s[d as usize];
                    self.regs.s[d as usize] =
                        (old & !(0xffff << shift)) | (u32::from(half) << shift);
                } else {
                    let half = (self.regs.s[m as usize] >> shift) as u16;
                    if double {
                        let x = env.half_to_double(half);
                        self.set_double(d, x);
                    } else {
                        let x = half_to_single(&mut env, half);
                        self.regs.s[d as usize] = x.to_bits();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(fpscr: &mut u32) -> Env<'_> {
        Env { fpscr }
    }

    #[test]
    fn double_arithmetic_flags() {
        let mut fpscr = 0;
        assert_eq!(env(&mut fpscr).add(1.0, 2.0, false), 3.0);
        assert_eq!(env(&mut fpscr).mul(1.5, 2.0), 3.0);
        assert_eq!(fpscr, 0);
        assert_eq!(env(&mut fpscr).div(1.0, 3.0), 1.0 / 3.0);
        assert_ne!(fpscr & FPSCR_IXC, 0);
        fpscr = 0;
        assert_eq!(env(&mut fpscr).div(1.0, 0.0), f64::INFINITY);
        assert_ne!(fpscr & FPSCR_DZC, 0);
        fpscr = 0;
        assert_eq!(env(&mut fpscr).sqrt(4.0), 2.0);
        assert_eq!(fpscr, 0);
        assert!(env(&mut fpscr).sqrt(-1.0).is_nan());
        assert_ne!(fpscr & FPSCR_IOC, 0);
        fpscr = 0;
        assert_eq!(env(&mut fpscr).fused(1.0, 2.0, 3.0), 7.0);
        assert_eq!(fpscr, 0);
    }

    #[test]
    fn half_precision_round_trips() {
        let mut fpscr = 0;
        for h in [
            0x0000u16, 0x8000, 0x3c00, 0xc000, 0x7bff, 0x0001, 0x03ff, 0x0400, 0x7c00,
        ] {
            let x = env(&mut fpscr).half_to_double(h);
            assert_eq!(env(&mut fpscr).double_to_half(x), h, "{h:#06x}");
        }
        assert_eq!(fpscr, 0);
        assert_eq!(env(&mut fpscr).half_to_double(0x3c00), 1.0);
        assert_eq!(env(&mut fpscr).half_to_double(0x7bff), 65504.0);
        assert_eq!(env(&mut fpscr).double_to_half(0.1), 0x2e66);
        assert_ne!(fpscr & FPSCR_IXC, 0);
        fpscr = 0;
        assert_eq!(env(&mut fpscr).double_to_half(70000.0), 0x7c00);
        assert_ne!(fpscr & FPSCR_OFC, 0);
        // 65519 s'arrondit à 65504, 65520 déborde.
        fpscr = 0;
        assert_eq!(env(&mut fpscr).double_to_half(65519.0), 0x7bff);
        assert_eq!(env(&mut fpscr).double_to_half(65520.0), 0x7c00);
    }

    #[test]
    fn decodes_double_instructions() {
        // vadd.f64 d0, d1, d2 : 0xee31 0x0b02.
        assert_eq!(
            decode_data(0xee31, 0x0b02),
            Some(DInstr::Arith {
                op: FpArith::Vadd,
                d: 0,
                n: 2,
                m: 4
            })
        );
        // vcvtb.f16.f32 s0, s1 : 0xeeb3 0x0a60 ; vcvtt.f32.f16 s0, s1 : 0xeeb2 0x0ae0.
        assert_eq!(
            decode_half(0xeeb3, 0x0a60),
            Some(DInstr::Half {
                top: false,
                to_half: true,
                double: false,
                d: 0,
                m: 1
            })
        );
        assert_eq!(
            decode_half(0xeeb2, 0x0ae0),
            Some(DInstr::Half {
                top: true,
                to_half: false,
                double: false,
                d: 0,
                m: 1
            })
        );
        // vmov.f64 d0, #1.0 : 0xeeb7 0x0b00.
        assert_eq!(
            decode_data(0xeeb7, 0x0b00),
            Some(DInstr::MovImm {
                d: 0,
                bits: 0x3ff0_0000_0000_0000
            })
        );
    }
}
