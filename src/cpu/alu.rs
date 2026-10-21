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

