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
