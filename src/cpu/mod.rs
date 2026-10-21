//! Processeur ARM Cortex-M7 (ARMv7E-M, Thumb-2, virgule flottante FPv5-D16 simple précision).
//!
//! Ce module ne doit jamais dépendre du reste de nwasabi (aucun chemin `crate::`, vérifié par
//! `tests/cpu_independence.rs`) ni de crates externes, afin de pouvoir devenir un crate
//! autonome. Il ne contient rien de spécifique à NumWorks : ni carte mémoire, ni appels système.
//! Tout ce dont il a besoin de l'extérieur passe par les traits [`Bus`] et [`Hooks`].

mod bus;
mod hooks;
mod regs;

pub use bus::{Bus, BusFault, FaultKind, Width};
pub use hooks::{Hooks, SvcAction};
pub use regs::Registers;

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
}

/// L'état complet du processeur.
#[derive(Debug, Clone, Default)]
pub struct Cpu {
    pub regs: Registers,
    /// Cycles écoulés depuis la création, selon un modèle de durée simple.
    pub cycles: u64,
}

impl Cpu {
    pub fn new() -> Self {
        Self::default()
    }
}
