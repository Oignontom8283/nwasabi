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

