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

#[cfg(test)]
mod tests {
    use super::*;

    // Vecteurs produits par l'assembleur GNU pour ARM (arm-none-eabi-as 2.45) puis liés à 0x1000.

    #[test]
    fn decodes_bl_and_bw_like_the_gnu_assembler() {
        // 0x1000: bl 0x3026 ; 0x1004: b.w 0x3026
        assert_eq!(decode_branch_offset(0xf002, 0xf811), 0x3026 - 0x1004);
        assert_eq!(decode_branch_offset(0xf002, 0xb80f), 0x3026 - 0x1008);
        // 0x101c: bl 0x1000 ; 0x1020: b.w 0x1000 (vers l'arrière)
        assert_eq!(decode_branch_offset(0xf7ff, 0xfff0), 0x1000 - 0x1020);
        assert_eq!(decode_branch_offset(0xf7ff, 0xbfee), 0x1000 - 0x1024);
        // La valeur laissée par l'assembleur dans un `bl` à relocaliser est -4.
        assert_eq!(decode_branch_offset(0xf7ff, 0xfffe), -4);
    }

    #[test]
    fn classifies_branch_kinds() {
        assert_eq!(classify_branch(0xf002, 0xf811), Some(Branch::Bl));
        assert_eq!(classify_branch(0xf002, 0xb80f), Some(Branch::BW));
        assert_eq!(classify_branch(0xf002, 0xe811), Some(Branch::Blx));
        assert_eq!(classify_branch(0xf240, 0x0000), None); // movw
        assert_eq!(classify_branch(0x4770, 0xf811), None); // bx lr
    }

    #[test]
    fn encodes_branches_like_the_gnu_assembler() {
        // On repart d'un bl/b.w quelconque : seuls les champs immédiats doivent changer.
        assert_eq!(
            encode_branch_offset(0xf000, 0xf800, 0x3026 - 0x1004),
            Some((0xf002, 0xf811))
        );
        assert_eq!(
            encode_branch_offset(0xf000, 0xb800, 0x3026 - 0x1008),
            Some((0xf002, 0xb80f))
        );
        assert_eq!(
            encode_branch_offset(0xf000, 0xf800, 0x1000 - 0x1020),
            Some((0xf7ff, 0xfff0))
        );
        assert_eq!(
            encode_branch_offset(0xf000, 0xb800, 0x1000 - 0x1024),
            Some((0xf7ff, 0xbfee))
        );
    }

    #[test]
    fn branch_offset_round_trips_over_the_whole_range() {
        let mut offset = -(1 << 24);
        while offset < (1 << 24) {
            let (hw1, hw2) = encode_branch_offset(0xf000, 0xf800, offset).unwrap();
            assert_eq!(decode_branch_offset(hw1, hw2), offset);
            assert_eq!(classify_branch(hw1, hw2), Some(Branch::Bl));
            offset += 2 * 4099; // pas premier avec la taille des champs, pour varier les bits
        }
        for edge in [-(1 << 24), (1 << 24) - 2, 0, 2, -2] {
            let (hw1, hw2) = encode_branch_offset(0xf000, 0xb800, edge).unwrap();
            assert_eq!(decode_branch_offset(hw1, hw2), edge);
        }
    }

    #[test]
    fn rejects_out_of_range_or_odd_branch_offsets() {
        assert_eq!(encode_branch_offset(0xf000, 0xf800, 1 << 24), None);
        assert_eq!(encode_branch_offset(0xf000, 0xf800, -(1 << 24) - 2), None);
        assert_eq!(encode_branch_offset(0xf000, 0xf800, 3), None);
    }

    #[test]
    fn decodes_and_encodes_movw_movt_like_the_gnu_assembler() {
        assert_eq!(classify_mov16(0xf64a, 0x33cd), Some(false)); // movw r3, #0xabcd
        assert_eq!(classify_mov16(0xf2c1, 0x2334), Some(true)); // movt r3, #0x1234
        assert_eq!(decode_mov16(0xf64a, 0x33cd), 0xabcd);
        assert_eq!(decode_mov16(0xf2c1, 0x2334), 0x1234);
        assert_eq!(decode_mov16(0xf240, 0x0000), 0);
        assert_eq!(decode_mov16(0xf64f, 0x7bff), 0xffff);
        assert_eq!(decode_mov16(0xf2c8, 0x0500), 0x8000);
        // Écriture : on part d'une instruction avec la constante 0 et on retrouve celle de l'assembleur.
        assert_eq!(encode_mov16(0xf240, 0x0300, 0xabcd), (0xf64a, 0x33cd));
        assert_eq!(encode_mov16(0xf2c0, 0x0300, 0x1234), (0xf2c1, 0x2334));
        assert_eq!(encode_mov16(0xf640, 0x0b00, 0xffff), (0xf64f, 0x7bff));
        assert_eq!(encode_mov16(0xf2c0, 0x0500, 0x8000), (0xf2c8, 0x0500));
    }

    #[test]
    fn mov16_round_trips_for_every_value() {
        for value in 0..=u16::MAX {
            let (hw1, hw2) = encode_mov16(0xf240, 0x0000, value);
            assert_eq!(decode_mov16(hw1, hw2), value);
            assert_eq!(classify_mov16(hw1, hw2), Some(false));
        }
    }

    #[test]
    fn mov16_leaves_the_register_untouched() {
        // movw r11, #0xffff -> on remplace la constante, le registre (bits 11:8 de hw2) reste.
        let (_, hw2) = encode_mov16(0xf64f, 0x7bff, 0x0000);
        assert_eq!(hw2 & 0x0f00, 0x0b00);
    }
}
