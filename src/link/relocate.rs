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

#[cfg(test)]
mod tests {
    use super::*;

    fn target(address: u32, thumb: bool) -> Target {
        Target {
            address,
            thumb,
            weak_undefined: false,
        }
    }

    fn word(value: u32) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn pair(hw1: u16, hw2: u16) -> Vec<u8> {
        [hw1.to_le_bytes(), hw2.to_le_bytes()].concat()
    }

    #[test]
    fn abs32_adds_symbol_and_implicit_addend() {
        let mut data = word(0x10);
        apply(
            R_ARM_ABS32,
            &mut data,
            0,
            0x2000_0000,
            target(0x9020_0100, false),
        )
        .unwrap();
        assert_eq!(data, word(0x9020_0110));
    }

    #[test]
    fn abs32_to_a_thumb_function_keeps_the_thumb_bit() {
        // Adresse de fonction stockée dans une table : bit 0 à 1.
        let mut data = word(0);
        apply(R_ARM_ABS32, &mut data, 0, 0, target(0x9020_0101, true)).unwrap();
        assert_eq!(data, word(0x9020_0101));
        // Symbole de section + addend : le bit Thumb vient de T.
        let mut data = word(0x40);
        apply(R_ARM_ABS32, &mut data, 0, 0, target(0x9020_0000, true)).unwrap();
        assert_eq!(data, word(0x9020_0041));
    }

    #[test]
    fn target1_behaves_like_abs32() {
        let mut data = word(4);
        apply(R_ARM_TARGET1, &mut data, 0, 0, target(0x100, false)).unwrap();
        assert_eq!(data, word(0x104));
    }

    #[test]
    fn rel32_subtracts_the_place() {
        let mut data = word(0);
        apply(
            R_ARM_REL32,
            &mut data,
            0,
            0x9020_0010,
            target(0x9020_0100, false),
        )
        .unwrap();
        assert_eq!(data, word(0xf0));
        // Distance négative.
        let mut data = word(0);
        apply(
            R_ARM_REL32,
            &mut data,
            0,
            0x9020_0100,
            target(0x9020_0010, false),
        )
        .unwrap();
        assert_eq!(data, word((-0xf0i32) as u32));
    }

    #[test]
    fn prel31_keeps_bit_31_and_checks_range() {
        let mut data = word(0x8000_0000);
        apply(
            R_ARM_PREL31,
            &mut data,
            0,
            0x9020_0000,
            target(0x9020_0100, false),
        )
        .unwrap();
        assert_eq!(data, word(0x8000_0100));
        // Décalage négatif encodé sur 31 bits.
        let mut data = word(0);
        apply(
            R_ARM_PREL31,
            &mut data,
            0,
            0x9020_0100,
            target(0x9020_0000, false),
        )
        .unwrap();
        assert_eq!(data, word(0x7fff_ff00));
        // Trop loin pour 31 bits.
        let mut data = word(0);
        let err = apply(R_ARM_PREL31, &mut data, 0, 0, target(0x7000_0000, false)).unwrap_err();
        assert!(matches!(
            err,
            RelocError::Overflow {
                kind: R_ARM_PREL31,
                ..
            }
        ));
    }

    #[test]
    fn thm_call_matches_the_gnu_linker() {
        // `bl far_fwd` laissé à -4 par l'assembleur (f7ff fffe), placé à 0x1000, cible 0x3027 (Thumb).
        let mut data = pair(0xf7ff, 0xfffe);
        apply(R_ARM_THM_CALL, &mut data, 0, 0x1000, target(0x3027, true)).unwrap();
        assert_eq!(data, pair(0xf002, 0xf811));
        // Même chose vers l'arrière : 0x101c -> 0x1000.
        let mut data = pair(0xf7ff, 0xfffe);
        apply(R_ARM_THM_CALL, &mut data, 0, 0x101c, target(0x1001, true)).unwrap();
        assert_eq!(data, pair(0xf7ff, 0xfff0));
    }

    #[test]
    fn thm_jump24_matches_the_gnu_linker() {
        // `b.w far_fwd` à 0x1004 (f7ff bffe) -> 0x3026 : f002 b80f.
        let mut data = pair(0xf7ff, 0xbffe);
        apply(R_ARM_THM_JUMP24, &mut data, 0, 0x1004, target(0x3027, true)).unwrap();
        assert_eq!(data, pair(0xf002, 0xb80f));
        // Un `bl` n'est pas valable pour une relocation de saut.
        let mut data = pair(0xf7ff, 0xfffe);
        let err = apply(R_ARM_THM_JUMP24, &mut data, 0, 0x1004, target(0x3027, true)).unwrap_err();
        assert!(matches!(err, RelocError::BadInstruction { .. }));
    }

    #[test]
    fn thm_call_to_a_missing_weak_function_becomes_a_nop() {
        let mut data = pair(0xf7ff, 0xfffe);
        let weak = Target {
            address: 0,
            thumb: false,
            weak_undefined: true,
        };
        apply(R_ARM_THM_CALL, &mut data, 0, 0x1000, weak).unwrap();
        assert_eq!(data, pair(0xf3af, 0x8000));
    }

    #[test]
    fn thm_call_refuses_arm_targets_and_long_distances() {
        let mut blx = pair(0xf000, 0xe800);
        assert_eq!(
            apply(R_ARM_THM_CALL, &mut blx, 0, 0, target(0x100, false)),
            Err(RelocError::Interworking {
                kind: R_ARM_THM_CALL
            })
        );
        let mut far = pair(0xf7ff, 0xfffe);
        let err = apply(R_ARM_THM_CALL, &mut far, 0, 0, target(0x3000_0000, true)).unwrap_err();
        assert!(matches!(
            err,
            RelocError::Overflow {
                kind: R_ARM_THM_CALL,
                ..
            }
        ));
        let mut not_a_branch = pair(0x4770, 0x4770);
        let err = apply(R_ARM_THM_CALL, &mut not_a_branch, 0, 0, target(0x100, true)).unwrap_err();
        assert!(matches!(err, RelocError::BadInstruction { .. }));
    }

    #[test]
    fn movw_movt_load_the_two_halves_of_an_address() {
        // movw r3, #0 / movt r3, #0 vers l'adresse 0x12345679 (fonction Thumb).
        let mut movw = pair(0xf240, 0x0300);
        apply(
            R_ARM_THM_MOVW_ABS_NC,
            &mut movw,
            0,
            0,
            target(0x1234_5678, true),
        )
        .unwrap();
        assert_eq!(movw, pair(0xf245, 0x6379)); // 0x5679
        let mut movt = pair(0xf2c0, 0x0300);
        apply(
            R_ARM_THM_MOVT_ABS,
            &mut movt,
            0,
            0,
            target(0x1234_5678, true),
        )
        .unwrap();
        assert_eq!(movt, pair(0xf2c1, 0x2334)); // 0x1234 (movt ne reçoit pas le bit Thumb)
        // Utiliser movw avec la relocation movt est une erreur.
        let mut wrong = pair(0xf240, 0x0300);
        assert!(apply(R_ARM_THM_MOVT_ABS, &mut wrong, 0, 0, target(0, false)).is_err());
    }

    #[test]
    fn none_does_nothing_and_unknown_kinds_are_refused() {
        let mut data = word(0xdead_beef);
        apply(R_ARM_NONE, &mut data, 0, 0, target(1, false)).unwrap();
        assert_eq!(data, word(0xdead_beef));
        assert_eq!(
            apply(200, &mut data, 0, 0, target(1, false)),
            Err(RelocError::Unsupported(200))
        );
    }

    #[test]
    fn out_of_section_offsets_are_refused() {
        let mut data = word(0);
        assert_eq!(
            apply(R_ARM_ABS32, &mut data, 2, 0, target(0, false)),
            Err(RelocError::OutOfSection)
        );
        assert_eq!(
            apply(R_ARM_ABS32, &mut data, u32::MAX, 0, target(0, false)),
            Err(RelocError::OutOfSection)
        );
    }
}
