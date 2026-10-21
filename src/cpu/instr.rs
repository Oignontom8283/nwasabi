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

    Mul {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        flags: FlagMode,
    },
    Mla {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Reg,
    },
    Mls {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Reg,
    },
    /// Multiplication 32×32 → 64 bits, éventuellement avec accumulation.
    MulLong {
        signed: bool,
        accumulate: bool,
        rdlo: Reg,
        rdhi: Reg,
        rn: Reg,
        rm: Reg,
    },
    Umaal {
        rdlo: Reg,
        rdhi: Reg,
        rn: Reg,
        rm: Reg,
    },
    Div {
        signed: bool,
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    /// `smulxy` / `smlaxy` : produit de deux demi-mots signés.
    Smul16 {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Option<Reg>,
        n_high: bool,
        m_high: bool,
    },
    /// `smmul` / `smmla` : mot de poids fort du produit signé 32×32.
    Smmul {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Option<Reg>,
        round: bool,
    },

    /// `bfi` (insertion) ; `rn` à `None` : `bfc` (mise à zéro).
    Bfi {
        rd: Reg,
        rn: Option<Reg>,
        lsb: u8,
        width: u8,
    },
    /// `ubfx` / `sbfx`.
    Bfx {
        signed: bool,
        rd: Reg,
        rn: Reg,
        lsb: u8,
        width: u8,
    },
    /// `ssat` / `usat` : saturation sur `bits` bits après un décalage.
    Sat {
        signed: bool,
        rd: Reg,
        rn: Reg,
        bits: u8,
        kind: ShiftKind,
        amount: u32,
    },
    Clz {
        rd: Reg,
        rm: Reg,
    },
    Rbit {
        rd: Reg,
        rm: Reg,
    },
    Rev {
        kind: RevKind,
        rd: Reg,
        rm: Reg,
    },
    /// `uxtb`, `sxth`, `uxtab`… ; `rn` présent pour les formes avec addition.
    Extend {
        kind: ExtKind,
        rd: Reg,
        rm: Reg,
        rotate: u8,
        rn: Option<Reg>,
    },
    Parallel {
        op: ParallelOp,
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    Sel {
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    /// `pkhbt` / `pkhtb`.
    Pkh {
        top_bottom: bool,
        rd: Reg,
        rn: Reg,
        rm: Reg,
        amount: u32,
    },

    B {
        offset: i32,
    },
    BCond {
        cond: u8,
        offset: i32,
    },
    Bl {
        offset: i32,
    },
    Bx {
        rm: Reg,
    },
    Blx {
        rm: Reg,
    },
    Cbz {
        rn: Reg,
        offset: u32,
        nonzero: bool,
    },
    /// `tbb` / `tbh` : branchement par table.
    TableBranch {
        half: bool,
        rn: Reg,
        rm: Reg,
    },

    Load {
        size: MemSize,
        signed: bool,
        rt: Reg,
        addr: Addr,
    },
    Store {
        size: MemSize,
        rt: Reg,
        addr: Addr,
    },
    Ldrd {
        rt: Reg,
        rt2: Reg,
        rn: Reg,
        offset: i32,
        mode: IndexMode,
    },
    Strd {
        rt: Reg,
        rt2: Reg,
        rn: Reg,
        offset: i32,
        mode: IndexMode,
    },
    /// `ldm`, `ldmia`, `ldmdb`, `pop`.
    Ldm {
        rn: Reg,
        regs: u16,
        writeback: bool,
        decrement_before: bool,
    },
    /// `stm`, `stmia`, `stmdb`, `push`.
    Stm {
        rn: Reg,
        regs: u16,
        writeback: bool,
        decrement_before: bool,
    },

    /// `it` : débute un bloc conditionnel.
    It {
        firstcond: u8,
        mask: u8,
    },
    Nop,
    Wfi,
    Wfe,
    Svc {
        imm: u8,
    },
    Bkpt {
        imm: u8,
    },
    Udf {
        imm: u16,
    },
    Mrs {
        rd: Reg,
        sysreg: SysReg,
    },
    /// `mask` : bits 11-10 du second demi-mot (pour APSR : bit 1 = N Z C V Q, bit 0 = GE).
    Msr {
        rn: Reg,
        sysreg: SysReg,
        mask: u8,
    },
    /// `cpsid` / `cpsie`.
    Cps {
        disable: bool,
        i: bool,
        f: bool,
    },
    /// `dmb`, `dsb`, `isb` : sans effet sur un processeur à un seul cœur émulé.
    Barrier,
    /// Instruction à virgule flottante.
    Fp(FpInstr),
}
