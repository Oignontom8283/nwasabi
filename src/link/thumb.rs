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

