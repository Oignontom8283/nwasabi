//! Représentation des instructions Thumb-2 après décodage.
//!
//! Le décodeur transforme les un ou deux demi-mots de l'instruction en une valeur [`Instr`] ; un
//! autre module l'exécute. Séparer les deux permet de tester le décodage seul et, plus tard, de
//! mémoriser les instructions décodées.

use super::alu::ShiftKind;

/// Numéro de registre (0 à 15 ; 13 = SP, 14 = LR, 15 = PC).
pub type Reg = u8;

/// L'instruction met-elle à jour les indicateurs N, Z, C, V ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagMode {
    /// Toujours (forme avec le suffixe `S` des instructions de 32 bits, `cmp`, `tst`…).
    Always,
    /// Jamais.
    Never,
    /// Seulement hors d'un bloc `IT` (formes de 16 bits comme `adds r0, r1, r2`).
    OutsideIt,
}

/// Opérations arithmétiques et logiques à deux opérandes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpOp {
    And,
    Eor,
    Sub,
    Rsb,
    Add,
    Adc,
    Sbc,
    Orr,
    Orn,
    Bic,
    Mov,
    Mvn,
    Tst,
    Teq,
    Cmp,
    Cmn,
}

impl DpOp {
    /// L'opération range-t-elle un résultat dans `rd` ? (non pour `tst`, `teq`, `cmp`, `cmn`)
    pub fn writes_result(self) -> bool {
        !matches!(self, Self::Tst | Self::Teq | Self::Cmp | Self::Cmn)
    }
}

