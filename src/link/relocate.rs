//! Application d'une relocation ARM : calcule la valeur finale d'un emplacement du programme
//! (adresse d'une fonction, distance d'un branchement…) maintenant que toutes les adresses sont
//! connues.
//!
//! Les relocations des fichiers ARM sont de type « REL » : la valeur déjà présente à l'emplacement
//! sert d'*addend* (`A`) ; on y ajoute l'adresse du symbole (`S`) et on retranche, si la valeur
//! est relative, l'adresse de l'emplacement lui-même (`P`). `T` vaut 1 quand le symbole est une
//! fonction Thumb : le bit 0 de l'adresse signale alors le mode Thumb.

use super::thumb::{self, Branch};
use crate::elf::{
    R_ARM_ABS32, R_ARM_NONE, R_ARM_PREL31, R_ARM_REL32, R_ARM_TARGET1, R_ARM_THM_CALL,
    R_ARM_THM_JUMP24, R_ARM_THM_MOVT_ABS, R_ARM_THM_MOVW_ABS_NC,
};

/// Ce que la relocation vise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// `S` : adresse du symbole, avec le bit 0 à 1 pour une fonction Thumb.
    pub address: u32,
    /// `T` : le symbole est une fonction Thumb.
    pub thumb: bool,
    /// Symbole faible resté non défini (`S` vaut alors 0).
    pub weak_undefined: bool,
}

/// Pourquoi une relocation n'a pas pu être appliquée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocError {
    /// Type de relocation non géré.
    Unsupported(u8),
    /// L'emplacement dépasse la section.
    OutOfSection,
    /// L'instruction trouvée n'est pas celle attendue pour ce type de relocation.
    BadInstruction { kind: u8, hw1: u16, hw2: u16 },
    /// La valeur ne tient pas dans le champ (branchement trop long…).
    Overflow { kind: u8, value: i64 },
    /// Appel vers du code ARM (`blx`) : impossible sur Cortex-M, qui n'exécute que du Thumb.
    Interworking { kind: u8 },
}

fn slice_at(section: &mut [u8], offset: u32, len: usize) -> Result<&mut [u8], RelocError> {
    let start = offset as usize;
    section
        .get_mut(start..start.saturating_add(len))
        .ok_or(RelocError::OutOfSection)
}

fn read_u32(section: &mut [u8], offset: u32) -> Result<u32, RelocError> {
    let b = slice_at(section, offset, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn write_u32(section: &mut [u8], offset: u32, value: u32) -> Result<(), RelocError> {
    slice_at(section, offset, 4)?.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn read_pair(section: &mut [u8], offset: u32) -> Result<(u16, u16), RelocError> {
    let b = slice_at(section, offset, 4)?;
    Ok((
        u16::from_le_bytes([b[0], b[1]]),
        u16::from_le_bytes([b[2], b[3]]),
    ))
}

fn write_pair(section: &mut [u8], offset: u32, (hw1, hw2): (u16, u16)) -> Result<(), RelocError> {
    let b = slice_at(section, offset, 4)?;
    b[..2].copy_from_slice(&hw1.to_le_bytes());
    b[2..].copy_from_slice(&hw2.to_le_bytes());
    Ok(())
}

