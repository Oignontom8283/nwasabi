//! Opérations arithmétiques et logiques de base du processeur, sans état : additions avec retenue,
//! décalages, expansion des constantes immédiates, saturation. Ce sont des fonctions pures, faciles
//! à tester une par une.

/// Type de décalage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftKind {
    Lsl,
    Lsr,
    Asr,
    Ror,
    /// Rotation à droite d'un bit à travers la retenue.
    Rrx,
}

impl ShiftKind {
    /// Type de décalage codé sur 2 bits (`00` LSL, `01` LSR, `10` ASR, `11` ROR).
    pub fn from_bits(bits: u32) -> Self {
        match bits & 3 {
            0 => Self::Lsl,
            1 => Self::Lsr,
            2 => Self::Asr,
            _ => Self::Ror,
        }
    }
}

/// `x + y + carry_in` : résultat, retenue sortante et débordement signé.
pub fn add_with_carry(x: u32, y: u32, carry_in: bool) -> (u32, bool, bool) {
    let unsigned = u64::from(x) + u64::from(y) + u64::from(carry_in);
    let signed = i64::from(x as i32) + i64::from(y as i32) + i64::from(carry_in);
    let result = unsigned as u32;
    (
        result,
        u64::from(result) != unsigned,
        i64::from(result as i32) != signed,
    )
}

/// Décale `value` et renvoie le résultat et la retenue sortante. `amount` est le nombre de bits
/// (pour `Rrx`, il vaut 1 implicitement). Un décalage de 0 laisse la valeur et la retenue.
pub fn shift_with_carry(value: u32, kind: ShiftKind, amount: u32, carry_in: bool) -> (u32, bool) {
    if kind == ShiftKind::Rrx {
        return ((u32::from(carry_in) << 31) | (value >> 1), value & 1 != 0);
    }
    if amount == 0 {
        return (value, carry_in);
    }
    match kind {
        ShiftKind::Lsl => match amount {
            1..=31 => (value << amount, (value >> (32 - amount)) & 1 != 0),
            32 => (0, value & 1 != 0),
            _ => (0, false),
        },
        ShiftKind::Lsr => match amount {
            1..=31 => (value >> amount, (value >> (amount - 1)) & 1 != 0),
            32 => (0, value >> 31 != 0),
            _ => (0, false),
        },
        ShiftKind::Asr => {
            let sign = (value as i32) >> 31;
            match amount {
                1..=31 => (
                    ((value as i32) >> amount) as u32,
                    (value >> (amount - 1)) & 1 != 0,
                ),
                _ => (sign as u32, value >> 31 != 0),
            }
        }
        ShiftKind::Ror => {
            let n = amount % 32;
            let result = value.rotate_right(n);
            (result, result >> 31 != 0)
        }
        ShiftKind::Rrx => unreachable!("traité plus haut"),
    }
}

/// Décalage d'un champ immédiat à 5 bits : `LSR #0` et `ASR #0` signifient 32, `ROR #0` signifie RRX.
pub fn decode_imm_shift(kind_bits: u32, imm5: u32) -> (ShiftKind, u32) {
    match (ShiftKind::from_bits(kind_bits), imm5) {
        (ShiftKind::Lsl, n) => (ShiftKind::Lsl, n),
        (ShiftKind::Lsr, 0) => (ShiftKind::Lsr, 32),
        (ShiftKind::Asr, 0) => (ShiftKind::Asr, 32),
        (ShiftKind::Ror, 0) => (ShiftKind::Rrx, 1),
        (kind, n) => (kind, n),
    }
}

/// Expansion d'une constante immédiate « modifiée » de 12 bits (`ThumbExpandImm_C`) : valeur et,
/// pour les formes avec rotation, la retenue produite (`None` : la retenue ne change pas).
pub fn thumb_expand_imm(imm12: u32) -> (u32, Option<bool>) {
    let imm8 = imm12 & 0xff;
    if imm12 >> 10 == 0 {
        let value = match (imm12 >> 8) & 3 {
            0 => imm8,
            1 => (imm8 << 16) | imm8,
            2 => (imm8 << 24) | (imm8 << 8),
            _ => (imm8 << 24) | (imm8 << 16) | (imm8 << 8) | imm8,
        };
        (value, None)
    } else {
        let unrotated = 0x80 | (imm12 & 0x7f);
        let rotation = imm12 >> 7;
        let value = unrotated.rotate_right(rotation);
        (value, Some(value >> 31 != 0))
    }
}

/// Étend le signe d'une valeur de `bits` bits.
pub fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

/// Saturation signée de `value` sur `bits` bits : résultat et indicateur de saturation.
pub fn signed_saturate(value: i64, bits: u32) -> (i32, bool) {
    let max = (1i64 << (bits - 1)) - 1;
    let min = -(1i64 << (bits - 1));
    if value > max {
        (max as i32, true)
    } else if value < min {
        (min as i32, true)
    } else {
        (value as i32, false)
    }
}

/// Saturation non signée de `value` sur `bits` bits : résultat et indicateur de saturation.
pub fn unsigned_saturate(value: i64, bits: u32) -> (u32, bool) {
    let max = (1i64 << bits) - 1;
    if value > max {
        (max as u32, true)
    } else if value < 0 {
        (0, true)
    } else {
        (value as u32, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_with_carry_reports_carry_and_overflow() {
        assert_eq!(add_with_carry(1, 2, false), (3, false, false));
        assert_eq!(add_with_carry(0xffff_ffff, 1, false), (0, true, false));
        assert_eq!(
            add_with_carry(0x7fff_ffff, 1, false),
            (0x8000_0000, false, true)
        );
        assert_eq!(
            add_with_carry(0x8000_0000, 0x8000_0000, false),
            (0, true, true)
        );
        // Soustraction a - b = a + !b + 1.
        assert_eq!(add_with_carry(5, !3, true), (2, true, false));
        assert_eq!(add_with_carry(3, !5, true), ((-2i32) as u32, false, false));
        assert_eq!(add_with_carry(0, 0, true), (1, false, false));
        assert_eq!(add_with_carry(0xffff_ffff, 0, true), (0, true, false));
    }

    #[test]
    fn logical_shifts_follow_the_arm_rules() {
        assert_eq!(
            shift_with_carry(0x8000_0001, ShiftKind::Lsl, 1, false),
            (2, true)
        );
        assert_eq!(shift_with_carry(1, ShiftKind::Lsl, 32, false), (0, true));
        assert_eq!(shift_with_carry(1, ShiftKind::Lsl, 33, true), (0, false));
        assert_eq!(
            shift_with_carry(0x8000_0000, ShiftKind::Lsr, 31, false),
            (1, false)
        );
        assert_eq!(
            shift_with_carry(0x8000_0000, ShiftKind::Lsr, 32, false),
            (0, true)
        );
        assert_eq!(
            shift_with_carry(0xffff_ffff, ShiftKind::Lsr, 40, true),
            (0, false)
        );
        // Un décalage de zéro ne touche ni la valeur ni la retenue.
        assert_eq!(
            shift_with_carry(0x1234, ShiftKind::Lsl, 0, true),
            (0x1234, true)
        );
    }

    #[test]
    fn arithmetic_shift_keeps_the_sign() {
        assert_eq!(
            shift_with_carry(0x8000_0000, ShiftKind::Asr, 4, false),
            (0xf800_0000, false)
        );
        assert_eq!(
            shift_with_carry(0x8000_0001, ShiftKind::Asr, 1, false),
            (0xc000_0000, true)
        );
        assert_eq!(
            shift_with_carry(0x8000_0000, ShiftKind::Asr, 32, false),
            (0xffff_ffff, true)
        );
        assert_eq!(
            shift_with_carry(0x7fff_ffff, ShiftKind::Asr, 99, true),
            (0, false)
        );
    }

    #[test]
    fn rotations() {
        assert_eq!(
            shift_with_carry(0x0000_0003, ShiftKind::Ror, 1, false),
            (0x8000_0001, true)
        );
        // Une rotation d'un multiple de 32 laisse la valeur mais la retenue est son bit 31.
        assert_eq!(
            shift_with_carry(0x8000_0000, ShiftKind::Ror, 32, false),
            (0x8000_0000, true)
        );
        assert_eq!(
            shift_with_carry(0x0000_0003, ShiftKind::Rrx, 1, true),
            (0x8000_0001, true)
        );
        assert_eq!(
            shift_with_carry(0x0000_0002, ShiftKind::Rrx, 1, false),
            (1, false)
        );
    }

    #[test]
    fn immediate_shift_encoding_special_cases() {
        assert_eq!(decode_imm_shift(0, 0), (ShiftKind::Lsl, 0));
        assert_eq!(decode_imm_shift(1, 0), (ShiftKind::Lsr, 32));
        assert_eq!(decode_imm_shift(2, 0), (ShiftKind::Asr, 32));
        assert_eq!(decode_imm_shift(3, 0), (ShiftKind::Rrx, 1));
        assert_eq!(decode_imm_shift(3, 5), (ShiftKind::Ror, 5));
    }

    #[test]
    fn modified_immediates_expand_like_the_manual() {
        // Formes sans rotation : la retenue n'est pas modifiée.
        assert_eq!(thumb_expand_imm(0x0ab), (0x0000_00ab, None));
        assert_eq!(thumb_expand_imm(0x1ab), (0x00ab_00ab, None));
        assert_eq!(thumb_expand_imm(0x2ab), (0xab00_ab00, None));
        assert_eq!(thumb_expand_imm(0x3ab), (0xabab_abab, None));
        // Formes avec rotation : (0x80 | imm7) tourné à droite de imm12[11:7] ; retenue = bit 31.
        assert_eq!(thumb_expand_imm(0x400), (0x8000_0000, Some(true))); // 0x80 tourné de 8
        assert_eq!(thumb_expand_imm(0x480), (0x4000_0000, Some(false))); // tourné de 9
        assert_eq!(thumb_expand_imm(0x4ff), (0x7f80_0000, Some(false))); // 0xff tourné de 9
        assert_eq!(thumb_expand_imm(0x800), (0x0080_0000, Some(false))); // tourné de 16
        assert_eq!(thumb_expand_imm(0xc00), (0x0000_8000, Some(false))); // tourné de 24
    }

    #[test]
    fn sign_extension_and_saturation() {
        assert_eq!(sign_extend(0xff, 8), -1);
        assert_eq!(sign_extend(0x7f, 8), 127);
        assert_eq!(sign_extend(0x800, 12), -2048);
        assert_eq!(sign_extend(0x00ff_ffff, 25), 0x00ff_ffff);
        assert_eq!(signed_saturate(200, 8), (127, true));
        assert_eq!(signed_saturate(-200, 8), (-128, true));
        assert_eq!(signed_saturate(-128, 8), (-128, false));
        assert_eq!(unsigned_saturate(300, 8), (255, true));
        assert_eq!(unsigned_saturate(-1, 8), (0, true));
        assert_eq!(unsigned_saturate(255, 8), (255, false));
    }
}
