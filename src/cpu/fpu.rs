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

const FPSCR_IOC: u32 = 1 << 0;
const FPSCR_DZC: u32 = 1 << 1;
const FPSCR_OFC: u32 = 1 << 2;
const FPSCR_UFC: u32 = 1 << 3;
const FPSCR_IXC: u32 = 1 << 4;
const FPSCR_IDC: u32 = 1 << 7;
/// Mise à zéro des dénormalisés.
const FPSCR_FZ: u32 = 1 << 24;
/// Mode « NaN par défaut ».
const FPSCR_DN: u32 = 1 << 25;
/// Mode d'arrondi (bits 23-22).
const FPSCR_RMODE_SHIFT: u32 = 22;
const DEFAULT_NAN: u32 = 0x7fc0_0000;

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
            // Somme exactement nulle : signe selon le mode d'arrondi (+0 en arrondi au plus proche).
            return if a == -b { 0.0 } else { result };
        }
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
        self.finish(result, finite && f64::from(result) != exact, finite)
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
        self.finish(result, inexact, true)
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
}
