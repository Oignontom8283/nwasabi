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

/// Deuxième opérande d'une opération de traitement de données.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rhs {
    /// Constante ; `carry` est la retenue produite par le codage de la constante (`None` : inchangée).
    Imm { value: u32, carry: Option<bool> },
    /// Registre décalé d'une quantité immédiate.
    Reg {
        rm: Reg,
        kind: ShiftKind,
        amount: u32,
    },
}

/// Quantité d'un décalage : immédiate ou donnée par un registre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftAmount {
    Imm(u32),
    Reg(Reg),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemSize {
    Byte,
    Half,
    Word,
}

/// Quand l'adresse de base est mise à jour dans un accès mémoire à base + décalage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexMode {
    /// Adresse = base + décalage, base inchangée.
    Offset,
    /// Adresse = base + décalage, puis base = adresse.
    PreIndex,
    /// Adresse = base, puis base += décalage.
    PostIndex,
}

/// Façon de calculer l'adresse d'un accès mémoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Addr {
    /// Base + constante signée.
    Imm {
        rn: Reg,
        offset: i32,
        mode: IndexMode,
    },
    /// Base + registre décalé à gauche de `shift` bits.
    Reg { rn: Reg, rm: Reg, shift: u8 },
    /// PC (aligné sur 4) + constante signée.
    Literal { offset: i32 },
}

/// Extensions d'octets ou de demi-mots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtKind {
    Sxtb,
    Sxth,
    Uxtb,
    Uxth,
    Sxtb16,
    Uxtb16,
}

/// Opérations arithmétiques sur plusieurs petites valeurs à la fois (SIMD sur 32 bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallelOp {
    Uadd8,
    Uqadd8,
    Uqsub8,
    Uqadd16,
    Uqsub16,
    Sadd8,
    Ssub8,
    Usub8,
    Uadd16,
    Usub16,
    Sadd16,
    Ssub16,
}

/// Réorganisations d'octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevKind {
    Rev,
    Rev16,
    Revsh,
}

/// Registres spéciaux accessibles par `mrs`/`msr` (numéro `SYSm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SysReg(pub u8);

/// Instructions de transfert de registres flottants (voir le module `fpu`).
pub use super::fpu::FpInstr;

/// Une instruction décodée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    /// Opération arithmétique ou logique.
    Dp {
        op: DpOp,
        rd: Reg,
        rn: Reg,
        rhs: Rhs,
        flags: FlagMode,
    },
    /// Décalage ou rotation d'un registre (`mov`, `lsl`, `lsr`, `asr`, `ror`, `rrx`).
    Shift {
        kind: ShiftKind,
        rd: Reg,
        value: Reg,
        amount: ShiftAmount,
        flags: FlagMode,
    },
    Movw {
        rd: Reg,
        imm: u16,
    },
    Movt {
        rd: Reg,
        imm: u16,
    },
    /// `addw`/`subw` : base + constante de 12 bits, sans modifier les indicateurs.
    AddSubImm12 {
        rd: Reg,
        rn: Reg,
        imm: u32,
        sub: bool,
    },
    /// `adr` : adresse relative au PC (aligné sur 4).
    Adr {
        rd: Reg,
        offset: i32,
    },

