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
        // ldrh / strh avec constante de 5 bits.
        0b10000 | 0b10001 => {
            let addr = Addr::Imm {
                rn: low3(3),
                offset: (bits(h, 10, 6) << 1) as i32,
                mode: IndexMode::Offset,
            };
            if bit(h, 11) {
                Instr::Load {
                    size: MemSize::Half,
                    signed: false,
                    rt: low3(0),
                    addr,
                }
            } else {
                Instr::Store {
                    size: MemSize::Half,
                    rt: low3(0),
                    addr,
                }
            }
        }
        // ldr / str relatifs à SP.
        0b10010 | 0b10011 => {
            let addr = Addr::Imm {
                rn: 13,
                offset: (bits(h, 7, 0) << 2) as i32,
                mode: IndexMode::Offset,
            };
            if bit(h, 11) {
                Instr::Load {
                    size: MemSize::Word,
                    signed: false,
                    rt: low3(8),
                    addr,
                }
            } else {
                Instr::Store {
                    size: MemSize::Word,
                    rt: low3(8),
                    addr,
                }
            }
        }
        // adr / add rd, sp, #imm.
        0b10100 => Instr::Adr {
            rd: low3(8),
            offset: (bits(h, 7, 0) << 2) as i32,
        },
        0b10101 => Instr::AddSubImm12 {
            rd: low3(8),
            rn: 13,
            imm: bits(h, 7, 0) << 2,
            sub: false,
        },
        0b10110 | 0b10111 => return decode16_misc(h),
        // stm / ldm.
        0b11000 | 0b11001 => {
            let rn = low3(8);
            let regs = bits(h, 7, 0) as u16;
            if bit(h, 11) {
                let writeback = regs & (1 << rn) == 0;
                Instr::Ldm {
                    rn,
                    regs,
                    writeback,
                    decrement_before: false,
                }
            } else {
                Instr::Stm {
                    rn,
                    regs,
                    writeback: true,
                    decrement_before: false,
                }
            }
        }
        // Branchement conditionnel, svc, udf.
        0b11010 | 0b11011 => match bits(h, 11, 8) {
            0xe => Instr::Udf {
                imm: bits(h, 7, 0) as u16,
            },
            0xf => Instr::Svc {
                imm: bits(h, 7, 0) as u8,
            },
            cond => Instr::BCond {
                cond: cond as u8,
                offset: sign_extend(bits(h, 7, 0) << 1, 9),
            },
        },
        // Branchement inconditionnel.
        0b11100 => Instr::B {
            offset: sign_extend(bits(h, 10, 0) << 1, 12),
        },
        _ => return None,
    })
}

/// Instructions diverses de 16 bits (`1011 xxxx …`).
fn decode16_misc(h: u32) -> Option<Instr> {
    let low3 = |shift: u32| bits(h, shift + 2, shift) as Reg;
    Some(match bits(h, 11, 8) {
        0b0000 => Instr::AddSubImm12 {
            rd: 13,
            rn: 13,
            imm: bits(h, 6, 0) << 2,
            sub: bit(h, 7),
        },
        0b0010 => {
            let kind = match bits(h, 7, 6) {
                0 => ExtKind::Sxth,
                1 => ExtKind::Sxtb,
                2 => ExtKind::Uxth,
                _ => ExtKind::Uxtb,
            };
            Instr::Extend {
                kind,
                rd: low3(0),
                rm: low3(3),
                rotate: 0,
                rn: None,
            }
        }
        0b0110 if bits(h, 7, 5) == 0b011 => Instr::Cps {
            disable: bit(h, 4),
            i: bit(h, 1),
            f: bit(h, 0),
        },
        // cbz / cbnz.
        0b0001 | 0b0011 | 0b1001 | 0b1011 => Instr::Cbz {
            rn: low3(0),
            offset: (bits(h, 9, 9) << 6) | (bits(h, 7, 3) << 1),
            nonzero: bit(h, 11),
        },
        0b0100 | 0b0101 => Instr::Stm {
            rn: 13,
            regs: (bits(h, 7, 0) | (bits(h, 8, 8) << 14)) as u16,
            writeback: true,
            decrement_before: true,
        },
        0b1100 | 0b1101 => Instr::Ldm {
            rn: 13,
            regs: (bits(h, 7, 0) | (bits(h, 8, 8) << 15)) as u16,
            writeback: true,
            decrement_before: false,
        },
        0b1010 => {
            let kind = match bits(h, 7, 6) {
                0 => RevKind::Rev,
                1 => RevKind::Rev16,
                3 => RevKind::Revsh,
                _ => return None,
            };
            Instr::Rev {
                kind,
                rd: low3(0),
                rm: low3(3),
            }
        }
        0b1110 => Instr::Bkpt {
            imm: bits(h, 7, 0) as u8,
        },
        0b1111 => {
            if bits(h, 3, 0) == 0 {
                match bits(h, 7, 4) {
                    0 | 1 | 4 => Instr::Nop, // nop, yield, sev
                    2 => Instr::Wfe,
                    3 => Instr::Wfi,
                    _ => return None,
                }
            } else {
                Instr::It {
                    firstcond: bits(h, 7, 4) as u8,
                    mask: bits(h, 3, 0) as u8,
                }
            }
        }
        _ => return None,
    })
}

/// Décalage de branchement de 25 bits des instructions `b.w` et `bl` (formes T4 et T1).
fn branch_offset25(hw1: u32, hw2: u32) -> i32 {
    let s = bits(hw1, 10, 10);
    let j1 = bits(hw2, 13, 13);
    let j2 = bits(hw2, 11, 11);
    let i1 = !(j1 ^ s) & 1;
    let i2 = !(j2 ^ s) & 1;
    let imm =
        (s << 24) | (i1 << 23) | (i2 << 22) | (bits(hw1, 9, 0) << 12) | (bits(hw2, 10, 0) << 1);
    sign_extend(imm, 25)
}

/// Décode une instruction de 32 bits.
pub fn decode32(hw1: u16, hw2: u16) -> Option<Instr> {
    let (a, b) = (u32::from(hw1), u32::from(hw2));
    match bits(a, 12, 11) {
        0b01 => match bits(a, 10, 9) {
            0b00 if !bit(a, 6) => decode_ldm_stm(a, b),
            0b00 => decode_dual_and_table(a, b),
            0b01 => decode_dp_shifted_register(a, b),
            _ => fpu::decode(hw1, hw2).map(Instr::Fp),
        },
        0b10 => {
            if !bit(b, 15) {
                if bit(a, 9) {
                    decode_dp_plain_immediate(a, b)
                } else {
                    decode_dp_modified_immediate(a, b)
                }
            } else {
                decode_branches_and_misc(a, b)
            }
        }
        _ => match bits(a, 10, 9) {
            // Chargements et rangements d'un seul élément (0xF8xx, 0xF9xx).
            0b00 => decode_load_store_single(a, b),
            // Traitement de données entre registres (0xFAxx) et multiplications (0xFBxx).
            0b01 => decode_register_and_multiply(a, b),
            // Coprocesseur : virgule flottante.
            _ => fpu::decode(hw1, hw2).map(Instr::Fp),
        },
    }
}

/// `stm.w`, `ldm.w`, `push.w`, `pop.w`, `stmdb`, `ldmdb`.
fn decode_ldm_stm(a: u32, b: u32) -> Option<Instr> {
    let decrement_before = match bits(a, 8, 7) {
        0b01 => false,
        0b10 => true,
        _ => return None,
    };
    let (rn, regs, writeback) = (bits(a, 3, 0) as Reg, b as u16, bit(a, 5));
    Some(if bit(a, 4) {
        Instr::Ldm {
            rn,
            regs,
            writeback,
            decrement_before,
        }
    } else {
        Instr::Stm {
            rn,
            regs,
            writeback,
            decrement_before,
        }
    })
}

/// `ldrd`, `strd`, `tbb`, `tbh` (les accès exclusifs ne sont pas gérés).
fn decode_dual_and_table(a: u32, b: u32) -> Option<Instr> {
    let (p, u, w, l) = (bit(a, 8), bit(a, 7), bit(a, 5), bit(a, 4));
    let rn = bits(a, 3, 0) as Reg;
    if p || w {
        let offset = (bits(b, 7, 0) << 2) as i32;
        let offset = if u { offset } else { -offset };
        let mode = match (p, w) {
            (true, false) => IndexMode::Offset,
            (true, true) => IndexMode::PreIndex,
            _ => IndexMode::PostIndex,
        };
        let (rt, rt2) = (bits(b, 15, 12) as Reg, bits(b, 11, 8) as Reg);
        return Some(if l {
            Instr::Ldrd {
                rt,
                rt2,
                rn,
                offset,
                mode,
            }
        } else {
            Instr::Strd {
                rt,
                rt2,
                rn,
                offset,
                mode,
            }
        });
    }
    if bits(a, 8, 4) == 0b01101 && bits(b, 15, 5) == 0b111_1000_0000 {
        return Some(Instr::TableBranch {
            half: bit(b, 4),
            rn,
            rm: bits(b, 3, 0) as Reg,
        });
    }
    None
}

/// Traitement de données avec registre décalé par une constante.
fn decode_dp_shifted_register(a: u32, b: u32) -> Option<Instr> {
    if bit(b, 15) {
        return None;
    }
    let (op, s) = (bits(a, 8, 5), bit(a, 4));
    let rn = bits(a, 3, 0) as Reg;
    let rd = bits(b, 11, 8) as Reg;
    let rm = bits(b, 3, 0) as Reg;
    let imm5 = (bits(b, 14, 12) << 2) | bits(b, 7, 6);
    let type_bits = bits(b, 5, 4);
    let (kind, amount) = decode_imm_shift(type_bits, imm5);
    let rhs = Rhs::Reg { rm, kind, amount };
    let flags = if s { FlagMode::Always } else { FlagMode::Never };
    let test = rd == 15 && s;
    Some(match op {
        0 if test => dp(DpOp::Tst, 0, rn, rhs, FlagMode::Always),
        0 => dp(DpOp::And, rd, rn, rhs, flags),
        1 => dp(DpOp::Bic, rd, rn, rhs, flags),
        // orr ; avec rn = pc : mov / décalages par une constante.
        2 if rn == 15 => Instr::Shift {
            kind,
            rd,
            value: rm,
            amount: ShiftAmount::Imm(amount),
            flags,
        },
        2 => dp(DpOp::Orr, rd, rn, rhs, flags),
        3 if rn == 15 => dp(DpOp::Mvn, rd, 0, rhs, flags),
        3 => dp(DpOp::Orn, rd, rn, rhs, flags),
        4 if test => dp(DpOp::Teq, 0, rn, rhs, FlagMode::Always),
        4 => dp(DpOp::Eor, rd, rn, rhs, flags),
        6 if !s => Instr::Pkh {
            top_bottom: bit(b, 5),
            rd,
            rn,
            rm,
            amount: imm5,
        },
        8 if test => dp(DpOp::Cmn, 0, rn, rhs, FlagMode::Always),
        8 => dp(DpOp::Add, rd, rn, rhs, flags),
        10 => dp(DpOp::Adc, rd, rn, rhs, flags),
        11 => dp(DpOp::Sbc, rd, rn, rhs, flags),
        13 if test => dp(DpOp::Cmp, 0, rn, rhs, FlagMode::Always),
        13 => dp(DpOp::Sub, rd, rn, rhs, flags),
        14 => dp(DpOp::Rsb, rd, rn, rhs, flags),
        _ => return None,
    })
}

/// Traitement de données avec constante « modifiée ».
fn decode_dp_modified_immediate(a: u32, b: u32) -> Option<Instr> {
    let (op, s) = (bits(a, 8, 5), bit(a, 4));
    let rn = bits(a, 3, 0) as Reg;
    let rd = bits(b, 11, 8) as Reg;
    let imm12 = (bits(a, 10, 10) << 11) | (bits(b, 14, 12) << 8) | bits(b, 7, 0);
    let (value, carry) = thumb_expand_imm(imm12);
    let rhs = Rhs::Imm { value, carry };
    let flags = if s { FlagMode::Always } else { FlagMode::Never };
    let test = rd == 15 && s;
    Some(match op {
        0 if test => dp(DpOp::Tst, 0, rn, rhs, FlagMode::Always),
        0 => dp(DpOp::And, rd, rn, rhs, flags),
        1 => dp(DpOp::Bic, rd, rn, rhs, flags),
        2 if rn == 15 => dp(DpOp::Mov, rd, 0, rhs, flags),
        2 => dp(DpOp::Orr, rd, rn, rhs, flags),
        3 if rn == 15 => dp(DpOp::Mvn, rd, 0, rhs, flags),
        3 => dp(DpOp::Orn, rd, rn, rhs, flags),
        4 if test => dp(DpOp::Teq, 0, rn, rhs, FlagMode::Always),
        4 => dp(DpOp::Eor, rd, rn, rhs, flags),
        8 if test => dp(DpOp::Cmn, 0, rn, rhs, FlagMode::Always),
        8 => dp(DpOp::Add, rd, rn, rhs, flags),
        10 => dp(DpOp::Adc, rd, rn, rhs, flags),
        11 => dp(DpOp::Sbc, rd, rn, rhs, flags),
        13 if test => dp(DpOp::Cmp, 0, rn, rhs, FlagMode::Always),
        13 => dp(DpOp::Sub, rd, rn, rhs, flags),
        14 => dp(DpOp::Rsb, rd, rn, rhs, flags),
        _ => return None,
    })
}

