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

/// Applique une relocation de type `kind` à l'emplacement `offset` de `section`.
///
/// `place` est l'adresse finale de cet emplacement (`P`).
pub fn apply(
    kind: u8,
    section: &mut [u8],
    offset: u32,
    place: u32,
    target: Target,
) -> Result<(), RelocError> {
    let s = target.address;
    let t = u32::from(target.thumb);
    match kind {
        R_ARM_NONE => Ok(()),

        // Adresse absolue sur 32 bits. TARGET1 est traité comme ABS32 (convention « bare metal »).
        R_ARM_ABS32 | R_ARM_TARGET1 => {
            let a = read_u32(section, offset)?;
            write_u32(section, offset, s.wrapping_add(a) | t)
        }

        // Distance relative sur 32 bits.
        R_ARM_REL32 => {
            let a = read_u32(section, offset)?;
            write_u32(section, offset, (s.wrapping_add(a) | t).wrapping_sub(place))
        }

        // Distance relative sur 31 bits (tables d'exceptions) ; le bit 31 de l'emplacement est conservé.
        R_ARM_PREL31 => {
            let word = read_u32(section, offset)?;
            let addend = (((word & 0x7fff_ffff) << 1) as i32) >> 1; // extension de signe 31 bits
            let value = i64::from((s.wrapping_add(addend as u32) | t).wrapping_sub(place) as i32);
            if !(-(1 << 30)..(1 << 30)).contains(&value) {
                return Err(RelocError::Overflow { kind, value });
            }
            write_u32(
                section,
                offset,
                (word & 0x8000_0000) | (value as u32 & 0x7fff_ffff),
            )
        }

        // Appel (`bl`) ou saut long (`b.w`) : seuls les 24 bits de décalage changent.
        R_ARM_THM_CALL | R_ARM_THM_JUMP24 => {
            let (hw1, hw2) = read_pair(section, offset)?;
            let branch = thumb::classify_branch(hw1, hw2).ok_or(RelocError::BadInstruction {
                kind,
                hw1,
                hw2,
            })?;
            match (kind, branch) {
                (R_ARM_THM_CALL, Branch::Bl | Branch::BW) | (R_ARM_THM_JUMP24, Branch::BW) => {}
                (_, Branch::Blx) => return Err(RelocError::Interworking { kind }),
                _ => return Err(RelocError::BadInstruction { kind, hw1, hw2 }),
            }
            // Appeler une fonction faible absente ne fait rien (comportement de l'éditeur de liens GNU).
            if target.weak_undefined {
                return write_pair(section, offset, thumb::NOP_W);
            }
            let addend = thumb::decode_branch_offset(hw1, hw2);
            let value = i64::from(s & !1) + i64::from(addend) - i64::from(place);
            let encoded = i32::try_from(value)
                .ok()
                .and_then(|v| thumb::encode_branch_offset(hw1, hw2, v))
                .ok_or(RelocError::Overflow { kind, value })?;
            write_pair(section, offset, encoded)
        }

        // `movw` : 16 bits de poids faible de (S + A) | T.
        // `movt` : 16 bits de poids fort de S + A.
        R_ARM_THM_MOVW_ABS_NC | R_ARM_THM_MOVT_ABS => {
            let (hw1, hw2) = read_pair(section, offset)?;
            let is_movt = thumb::classify_mov16(hw1, hw2).ok_or(RelocError::BadInstruction {
                kind,
                hw1,
                hw2,
            })?;
            if is_movt != (kind == R_ARM_THM_MOVT_ABS) {
                return Err(RelocError::BadInstruction { kind, hw1, hw2 });
            }
            let addend = i32::from(thumb::decode_mov16(hw1, hw2) as i16) as u32;
            let value = s.wrapping_add(addend);
            let imm = if is_movt {
                (value >> 16) as u16
            } else {
                ((value | t) & 0xffff) as u16
            };
            write_pair(section, offset, thumb::encode_mov16(hw1, hw2, imm))
        }

        other => Err(RelocError::Unsupported(other)),
    }
}

