//! Relocations (`SHT_REL`, addend implicite stocké dans le contenu de la section).

/// Types de relocation ARM (ELF for the Arm Architecture, `R_ARM_*`) utilisés par le code Thumb-2.
pub const R_ARM_NONE: u8 = 0;
pub const R_ARM_ABS32: u8 = 2;
pub const R_ARM_REL32: u8 = 3;
pub const R_ARM_THM_CALL: u8 = 10;
pub const R_ARM_THM_JUMP24: u8 = 30;
pub const R_ARM_TARGET1: u8 = 38;
pub const R_ARM_PREL31: u8 = 42;
pub const R_ARM_THM_MOVW_ABS_NC: u8 = 47;
pub const R_ARM_THM_MOVT_ABS: u8 = 48;

/// Une entrée de relocation : « à `offset` dans la section cible, patcher en fonction du symbole ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relocation {
    /// Position à patcher, relative au début de la section cible.
    pub offset: u32,
    /// Indice du symbole dans la table des symboles.
    pub symbol: u32,
    /// Type `R_ARM_*`.
    pub kind: u8,
}
