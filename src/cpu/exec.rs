//! Exécution des instructions : le cycle « lire, décoder, exécuter » du processeur.

use super::alu::{
    ShiftKind, add_with_carry, shift_with_carry, sign_extend, signed_saturate, unsigned_saturate,
};
use super::decode::{decode16, decode32, is_32bit};
use super::instr::{
    Addr, DpOp, ExtKind, FlagMode, IndexMode, Instr, MemSize, ParallelOp, Reg, RevKind, Rhs,
    ShiftAmount, SysReg,
};
use super::{Bus, Cpu, Hooks, Stop, SvcAction, UsageFault};

/// Contexte de l'instruction en cours.
#[derive(Clone, Copy)]
struct Ctx {
    /// Adresse de l'instruction.
    addr: u32,
    /// Valeur lue quand l'instruction utilise PC comme opérande : adresse + 4.
    pc4: u32,
    /// L'instruction appartient à un bloc `IT` (les instructions de 16 bits ne modifient alors pas
    /// les indicateurs).
    in_it: bool,
}

impl Cpu {
