//! Encodage des instructions Thumb-2 que l'éditeur de liens doit corriger : les branchements longs
//! (`bl`, `b.w`) et le chargement de constantes 16 bits (`movw`, `movt`).
//!
//! Une instruction Thumb-2 de 32 bits est écrite sous la forme de deux demi-mots de 16 bits
//! (`hw1` puis `hw2`). Les fonctions de ce module manipulent uniquement les champs « immédiats »
//! et conservent tout le reste de l'instruction (registre, condition…).

/// Instruction `nop.w` (32 bits) : remplace un appel vers une fonction faible non définie.
pub const NOP_W: (u16, u16) = (0xf3af, 0x8000);

/// Le branchement de 24 bits atteint ±16 Mio.
const BRANCH_RANGE: i32 = 1 << 24;

/// Nature d'un branchement long Thumb-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    /// `bl` : appel avec retour, reste en Thumb.
    Bl,
    /// `blx` : appel qui bascule en mode ARM (inexistant sur Cortex-M).
    Blx,
    /// `b.w` : saut inconditionnel (encodage T4).
    BW,
}

/// Reconnaît `bl`, `blx` ou `b.w` d'après leurs deux demi-mots.
pub fn classify_branch(hw1: u16, hw2: u16) -> Option<Branch> {
    if hw1 & 0xf800 != 0xf000 {
        return None;
    }
    match hw2 & 0xd000 {
        0xd000 => Some(Branch::Bl),
        0xc000 => Some(Branch::Blx),
        0x9000 => Some(Branch::BW),
        _ => None,
    }
}

/// Décalage (en octets, signé) encodé dans un `bl`/`b.w` : le champ vaut « cible − (adresse + 4) ».
pub fn decode_branch_offset(hw1: u16, hw2: u16) -> i32 {
    let s = u32::from(hw1 >> 10) & 1;
    let imm10 = u32::from(hw1) & 0x3ff;
    let j1 = u32::from(hw2 >> 13) & 1;
    let j2 = u32::from(hw2 >> 11) & 1;
    let imm11 = u32::from(hw2) & 0x7ff;
    let i1 = !(j1 ^ s) & 1;
    let i2 = !(j2 ^ s) & 1;
    let imm25 = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
    // Extension de signe depuis 25 bits.
    ((imm25 << 7) as i32) >> 7
}

/// Écrit `offset` dans les champs immédiats de `bl`/`b.w`. `None` si l'offset sort de ±16 Mio
/// ou n'est pas pair.
pub fn encode_branch_offset(hw1: u16, hw2: u16, offset: i32) -> Option<(u16, u16)> {
    if offset & 1 != 0 || !(-BRANCH_RANGE..BRANCH_RANGE).contains(&offset) {
        return None;
    }
    let value = offset as u32;
    let s = (value >> 24) & 1;
    let i1 = (value >> 23) & 1;
    let i2 = (value >> 22) & 1;
    let j1 = !(i1 ^ s) & 1;
    let j2 = !(i2 ^ s) & 1;
    let imm10 = (value >> 12) & 0x3ff;
    let imm11 = (value >> 1) & 0x7ff;
    let new1 = (hw1 & 0xf800) | ((s << 10) | imm10) as u16;
    let new2 = (hw2 & 0xd000) | ((j1 << 13) | (j2 << 11) | imm11) as u16;
    Some((new1, new2))
}

/// Reconnaît `movw` (T3) ou `movt` (T1) : renvoie `Some(true)` pour `movt`, `Some(false)` pour `movw`.
pub fn classify_mov16(hw1: u16, hw2: u16) -> Option<bool> {
    if hw2 & 0x8000 != 0 {
        return None;
    }
    match hw1 & 0xfbf0 {
        0xf240 => Some(false),
        0xf2c0 => Some(true),
        _ => None,
    }
}

/// Constante 16 bits (`imm4:i:imm3:imm8`) d'un `movw`/`movt`.
pub fn decode_mov16(hw1: u16, hw2: u16) -> u16 {
    let imm4 = hw1 & 0xf;
    let i = (hw1 >> 10) & 1;
    let imm3 = (hw2 >> 12) & 7;
    let imm8 = hw2 & 0xff;
    (imm4 << 12) | (i << 11) | (imm3 << 8) | imm8
}

/// Remplace la constante 16 bits d'un `movw`/`movt` en gardant le registre et l'opcode.
pub fn encode_mov16(hw1: u16, hw2: u16, value: u16) -> (u16, u16) {
    let imm4 = value >> 12;
    let i = (value >> 11) & 1;
    let imm3 = (value >> 8) & 7;
    let imm8 = value & 0xff;
    (
        (hw1 & 0xfbf0) | (i << 10) | imm4,
        (hw2 & 0x8f00) | (imm3 << 12) | imm8,
    )
}

