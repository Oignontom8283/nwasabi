//! Décodage des instructions Thumb et Thumb-2 (ARMv7E-M).
//!
//! `decode16` traduit une instruction de 16 bits, `decode32` une instruction de 32 bits (deux
//! demi-mots). Une instruction valide mais que nwasabi ne gère pas, ou invalide, donne `None`.

use super::alu::{ShiftKind, decode_imm_shift, sign_extend, thumb_expand_imm};
use super::fpu;
use super::instr::{
    Addr, DpOp, ExtKind, FlagMode, IndexMode, Instr, MemSize, ParallelOp, Reg, RevKind, Rhs,
    ShiftAmount, SysReg,
};

/// Une instruction de 32 bits commence par un demi-mot dont les 5 premiers bits valent 0b11101,
/// 0b11110 ou 0b11111.
pub fn is_32bit(hw1: u16) -> bool {
    hw1 >> 11 >= 0b11101
}

/// Bits `[hi:lo]` de `v`.
fn bits(v: u32, hi: u32, lo: u32) -> u32 {
    (v >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

fn bit(v: u32, n: u32) -> bool {
    (v >> n) & 1 != 0
}

fn dp(op: DpOp, rd: Reg, rn: Reg, rhs: Rhs, flags: FlagMode) -> Instr {
    Instr::Dp {
        op,
        rd,
        rn,
        rhs,
        flags,
    }
}

fn reg_rhs(rm: u32) -> Rhs {
    Rhs::Reg {
        rm: rm as Reg,
        kind: ShiftKind::Lsl,
        amount: 0,
    }
}

fn imm_rhs(value: u32) -> Rhs {
    Rhs::Imm { value, carry: None }
}

/// Décode une instruction de 16 bits.
pub fn decode16(hw: u16) -> Option<Instr> {
    let h = u32::from(hw);
    let low3 = |shift: u32| bits(h, shift + 2, shift) as Reg;
    Some(match h >> 11 {
        // Décalage par une constante : lsl, lsr, asr.
        0b00000..=0b00010 => {
            let (kind, amount) = decode_imm_shift(bits(h, 12, 11), bits(h, 10, 6));
            Instr::Shift {
                kind,
                rd: low3(0),
                value: low3(3),
                amount: ShiftAmount::Imm(amount),
                flags: FlagMode::OutsideIt,
            }
        }
        // add / sub sur trois registres ou avec une constante de 3 bits.
        0b00011 => {
            let op = if bit(h, 9) { DpOp::Sub } else { DpOp::Add };
            let third = bits(h, 8, 6);
            let rhs = if bit(h, 10) {
                imm_rhs(third)
            } else {
                reg_rhs(third)
            };
            dp(op, low3(0), low3(3), rhs, FlagMode::OutsideIt)
        }
        // mov / cmp / add / sub avec une constante de 8 bits.
        0b00100..=0b00111 => {
            let rd = low3(8);
            let imm = imm_rhs(bits(h, 7, 0));
            match bits(h, 12, 11) {
                0 => dp(DpOp::Mov, rd, 0, imm, FlagMode::OutsideIt),
                1 => dp(DpOp::Cmp, 0, rd, imm, FlagMode::Always),
                2 => dp(DpOp::Add, rd, rd, imm, FlagMode::OutsideIt),
                _ => dp(DpOp::Sub, rd, rd, imm, FlagMode::OutsideIt),
            }
        }
        0b01000 => match bits(h, 10, 8) {
            0..=3 => {
                // Opérations entre registres bas : and, eor, lsl, lsr, asr, adc, sbc, ror, tst, rsb, cmp,
                // cmn, orr, mul, bic, mvn.
                let (rdn, rm) = (low3(0), low3(3));
                let reg = Rhs::Reg {
                    rm,
                    kind: ShiftKind::Lsl,
                    amount: 0,
                };
                let shift = |kind| Instr::Shift {
                    kind,
                    rd: rdn,
                    value: rdn,
                    amount: ShiftAmount::Reg(rm),
                    flags: FlagMode::OutsideIt,
                };
                let alu = |op| dp(op, rdn, rdn, reg, FlagMode::OutsideIt);
                let test = |op| dp(op, 0, rdn, reg, FlagMode::Always);
                match bits(h, 9, 6) {
                    0 => alu(DpOp::And),
                    1 => alu(DpOp::Eor),
                    2 => shift(ShiftKind::Lsl),
                    3 => shift(ShiftKind::Lsr),
                    4 => shift(ShiftKind::Asr),
                    5 => alu(DpOp::Adc),
                    6 => alu(DpOp::Sbc),
                    7 => shift(ShiftKind::Ror),
                    8 => test(DpOp::Tst),
                    9 => dp(DpOp::Rsb, rdn, rm, imm_rhs(0), FlagMode::OutsideIt),
                    10 => test(DpOp::Cmp),
                    11 => test(DpOp::Cmn),
                    12 => alu(DpOp::Orr),
                    13 => Instr::Mul {
                        rd: rdn,
                        rn: rm,
                        rm: rdn,
                        flags: FlagMode::OutsideIt,
                    },
                    14 => alu(DpOp::Bic),
                    _ => dp(DpOp::Mvn, rdn, 0, reg, FlagMode::OutsideIt),
                }
            }
            // Opérations sur les registres hauts et branchements par registre.
            op => {
                let rdn = (bits(h, 7, 7) << 3 | bits(h, 2, 0)) as Reg;
                let rm = bits(h, 6, 3);
                match op {
                    4 => dp(DpOp::Add, rdn, rdn, reg_rhs(rm), FlagMode::Never),
                    5 => dp(DpOp::Cmp, 0, rdn, reg_rhs(rm), FlagMode::Always),
                    6 => dp(DpOp::Mov, rdn, 0, reg_rhs(rm), FlagMode::Never),
                    _ if !bit(h, 7) => Instr::Bx { rm: rm as Reg },
                    _ => Instr::Blx { rm: rm as Reg },
                }
            }
        },
        // ldr rt, [pc, #imm]
        0b01001 => Instr::Load {
            size: MemSize::Word,
            signed: false,
            rt: low3(8),
            addr: Addr::Literal {
                offset: (bits(h, 7, 0) << 2) as i32,
            },
        },
        // Accès mémoire avec registre de décalage.
        0b01010 | 0b01011 => {
            let addr = Addr::Reg {
                rn: low3(3),
                rm: low3(6),
                shift: 0,
            };
            let rt = low3(0);
            let load = |size, signed| Instr::Load {
                size,
                signed,
                rt,
                addr,
            };
            let store = |size| Instr::Store { size, rt, addr };
            match bits(h, 11, 9) {
                0 => store(MemSize::Word),
                1 => store(MemSize::Half),
                2 => store(MemSize::Byte),
                3 => load(MemSize::Byte, true),
                4 => load(MemSize::Word, false),
                5 => load(MemSize::Half, false),
                6 => load(MemSize::Byte, false),
                _ => load(MemSize::Half, true),
            }
        }
        // ldr / str / ldrb / strb avec constante de 5 bits.
        0b01100..=0b01111 => {
            let byte = bit(h, 12);
            let imm = bits(h, 10, 6);
            let (size, offset) = if byte {
                (MemSize::Byte, imm)
            } else {
                (MemSize::Word, imm << 2)
            };
            let addr = Addr::Imm {
                rn: low3(3),
                offset: offset as i32,
                mode: IndexMode::Offset,
            };
            if bit(h, 11) {
                Instr::Load {
                    size,
                    signed: false,
                    rt: low3(0),
                    addr,
                }
            } else {
                Instr::Store {
                    size,
                    rt: low3(0),
                    addr,
                }
            }
        }
