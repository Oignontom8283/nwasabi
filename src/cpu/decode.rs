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

