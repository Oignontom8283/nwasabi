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

/// Ce qui est fait de chaque voie d'une opération parallèle : arithmétique signée (`s`), signée saturée (`q`), signée
/// divisée par deux (`sh`), non signée (`u`), non signée saturée (`uq`), non signée divisée par deux (`uh`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallelPrefix {
    S,
    Q,
    Sh,
    U,
    Uq,
    Uh,
}

/// Les voies d'une opération parallèle : deux demi-mots (`add16`, `sub16`, `asx`, `sax`) ou quatre octets (`add8`, `sub8`).
/// `asx` soustrait dans la voie basse et additionne dans la haute, avec les demi-mots du second opérande échangés ; `sax` l'inverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallelKind {
    Add16,
    Asx,
    Sax,
    Sub16,
    Add8,
    Sub8,
}

/// Opérations arithmétiques sur plusieurs petites valeurs à la fois (SIMD sur 32 bits) : `sadd16`, `uqsub8`, `shasx`…
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParallelOp {
    pub prefix: ParallelPrefix,
    pub kind: ParallelKind,
}

/// Additions et soustractions saturées sur 32 bits : `qadd`, `qsub`, `qdadd`, `qdsub`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturatingKind {
    Add,
    Sub,
    DoubleAdd,
    DoubleSub,
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

/// Les instructions d'accès exclusif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusiveOp {
    Ldrex,
    Ldrexb,
    Ldrexh,
    Strex,
    Strexb,
    Strexh,
}

impl ExclusiveOp {
    /// Vrai pour les écritures conditionnelles.
    pub fn is_store(self) -> bool {
        matches!(self, Self::Strex | Self::Strexb | Self::Strexh)
    }

    /// Nombre d'octets accédés.
    pub fn bytes(self) -> u32 {
        match self {
            Self::Ldrex | Self::Strex => 4,
            Self::Ldrexh | Self::Strexh => 2,
            Self::Ldrexb | Self::Strexb => 1,
        }
    }
}

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
    /// `smmul` / `smmla` / `smmls` : mot de poids fort du produit signé 32×32 (`subtract` : `ra` moins le produit).
    Smmul {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Option<Reg>,
        round: bool,
        subtract: bool,
    },
    /// `qadd`, `qsub`, `qdadd`, `qdsub` : `rd = sat(rm ± rn)`, avec `rn` doublé (et saturé) pour les formes `d`.
    Saturating {
        kind: SaturatingKind,
        rd: Reg,
        rm: Reg,
        rn: Reg,
    },
    /// `smuad`, `smusd`, `smlad`, `smlsd` (et leurs formes `x`) : somme ou différence des deux produits de demi-mots.
    Dual16 {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Option<Reg>,
        subtract: bool,
        exchange: bool,
    },
    /// `smulwb`, `smulwt`, `smlawb`, `smlawt` : 32 bits × demi-mot, 16 bits de poids fort du produit sur 48.
    Smulw {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Option<Reg>,
        m_high: bool,
    },
    /// `smlalbb`, `smlalbt`, `smlaltb`, `smlaltt` : produit de demi-mots ajouté à un total de 64 bits.
    Smlalxy {
        rdlo: Reg,
        rdhi: Reg,
        rn: Reg,
        rm: Reg,
        n_high: bool,
        m_high: bool,
    },
    /// `smlald`, `smlsld` (et leurs formes `x`) : comme `Dual16`, avec un total de 64 bits.
    Dual16Long {
        rdlo: Reg,
        rdhi: Reg,
        rn: Reg,
        rm: Reg,
        subtract: bool,
        exchange: bool,
    },
    /// `usad8`, `usada8` : somme des différences absolues des quatre octets.
    Usad8 {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Option<Reg>,
    },
    /// `ssat16`, `usat16` : saturation de chacun des deux demi-mots sur `bits` bits.
    Sat16 {
        signed: bool,
        rd: Reg,
        rn: Reg,
        bits: u8,
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
    /// `clrex` : efface le moniteur d'exclusivité local.
    Clrex,
    /// `ldrex`, `ldrexb`, `ldrexh`, `strex`, `strexb`, `strexh`.
    Exclusive {
        op: ExclusiveOp,
        /// Registre transféré.
        rt: Reg,
        /// Registre qui reçoit l'état d'une écriture (0 : faite, 1 : refusée) ; inutilisé pour un chargement.
        rd: Reg,
        rn: Reg,
        /// Décalage, en octets, ajouté à `rn`.
        offset: u32,
    },
    /// Instruction à virgule flottante.
    Fp(FpInstr),
}
