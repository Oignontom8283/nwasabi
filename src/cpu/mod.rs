//! Processeur ARM Cortex-M7 (ARMv7E-M, Thumb-2, virgule flottante FPv5-D16 simple précision).
//!
//! Ce module ne doit jamais dépendre du reste de nwasabi (aucun chemin `crate::`, vérifié par
//! `tests/cpu_independence.rs`) ni de crates externes, afin de pouvoir devenir un crate
//! autonome. Il ne contient rien de spécifique à NumWorks : ni carte mémoire, ni appels système.
//! Tout ce dont il a besoin de l'extérieur passe par les traits [`Bus`] et [`Hooks`].

pub mod alu;
mod bus;
mod cache;
mod decode;
mod disasm;
mod exception;
mod exec;
mod fpu;
mod fpu_double;
mod hooks;
mod instr;
mod regs;

pub use bus::{Bus, BusFault, FaultKind, Width};
pub use decode::{decode16, decode32, is_32bit};
pub use disasm::{Line, disassemble};
pub use hooks::{Hooks, SvcAction};
pub use instr::Instr;
pub use regs::Registers;

/// Pourquoi le processeur lève une faute d'utilisation (`UsageFault`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageFault {
    /// Instruction que le processeur ne peut pas exécuter (`udf`).
    UndefinedInstruction,
    /// Branchement vers une adresse en mode ARM (bit 0 à zéro) : un Cortex-M n'exécute que du Thumb.
    InvalidState,
    /// Accès non aligné interdit (`ldm`, `stm`, `ldrd`, `strd`).
    Unaligned,
}

/// Raison pour laquelle l'exécution a rendu la main à l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// Le budget d'instructions ou de cycles donné par l'hôte est épuisé.
    Budget,
    /// Un hook a demandé l'arrêt (par exemple le programme est terminé ou attend une entrée).
    Requested,
    /// Un accès mémoire invalide.
    BusFault(BusFault),
    /// Une instruction que l'émulateur ne gère pas (encore) : son adresse et son premier demi-mot.
    Unimplemented { pc: u32, halfword: u16 },
    /// Faute d'utilisation à l'adresse `pc`.
    UsageFault { pc: u32, kind: UsageFault },
    /// Instruction `bkpt`.
    Breakpoint { imm8: u8 },
    /// Instruction `wfi` ou `wfe` : le processeur attend un événement.
    WaitForInterrupt,
}

/// L'état complet du processeur.
#[derive(Debug, Clone, Default)]
pub struct Cpu {
    pub regs: Registers,
    /// Cycles écoulés depuis la création, selon un modèle de durée simple (un par instruction).
    pub cycles: u64,
    /// Pile principale et pile de processus, quand elles ne sont pas la pile active (`regs.sp`).
    pub msp: u32,
    pub psp: u32,
    /// Registre `CONTROL` : bit 0 `nPRIV`, bit 1 `SPSEL` (pile de processus), bit 2 `FPCA`.
    pub control: u32,
    /// Vrai pendant le traitement d'une exception.
    pub handler_mode: bool,
    /// Adresse de la table des vecteurs d'exceptions.
    pub vtor: u32,
    pub primask: bool,
    pub faultmask: bool,
    pub basepri: u8,
    /// Adresse suivie par le moniteur d'exclusivité local (posée par `ldrex`, lue par `strex`).
    pub exclusive: Option<u32>,
    /// Instructions déjà décodées (voir `cache`).
    cache: cache::DecodeCache,
}

impl Cpu {
    pub fn new() -> Self {
        let mut cpu = Self::default();
        // Le bit « Thumb » de xPSR vaut toujours 1 sur un Cortex-M.
        cpu.regs.xpsr = regs::XPSR_T;
        cpu
    }
}

#[cfg(test)]
mod tests;
