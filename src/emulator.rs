//! L'émulateur : un appareil, un processeur et le noyau simulé, réunis pour exécuter une
//! application `.nwa`.
//!
//! L'exécution est coopérative : [`Emulator::run`] avance l'application jusqu'à un point où
//! l'hôte doit intervenir (image prête, attente d'un événement, fin…) et dit pourquoi elle
//! s'est arrêtée. Le cœur ne dort jamais et ne lit aucune horloge : c'est à l'hôte de faire
//! passer le temps ([`Kernel::advance_time`]).

use std::fmt;

use crate::cpu::{BusFault, Cpu, Stop, UsageFault};
use crate::device::{DeviceError, DeviceImage, EXIT_STUB_OFFSET};
use crate::kernel::{DeviceStatus, Kernel, KernelConfig, KernelStop};
use crate::link::{LinkError, LinkOptions, LinkedApp, link};
use crate::profile::{FirmwareProfile, Model};

/// Une faute qui arrête l'application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    Bus(BusFault),
    /// Instruction que l'émulateur ne sait pas (encore) exécuter.
    Unimplemented {
        pc: u32,
        halfword: u16,
    },
    UsageFault {
        pc: u32,
        kind: UsageFault,
    },
    /// Instruction `bkpt`.
    Breakpoint {
        imm8: u8,
    },
    /// `wfi` / `wfe` : rien ne pourrait réveiller le processeur.
    WaitForInterrupt,
    /// Appel système inconnu du noyau simulé.
    UnknownSvc(u8),
}

/// Pourquoi [`Emulator::run`] a rendu la main.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStop {
    /// L'image est prête : l'écran peut être affiché.
    Frame,
    /// L'application dort (le temps virtuel a déjà avancé).
    Sleep {
        ms: u32,
    },
    /// L'application attend un événement du clavier ; l'hôte doit faire passer du temps ou
    /// appuyer sur une touche avant de relancer.
    WaitEvent {
        remaining_ms: Option<u32>,
    },
    /// `main` est revenue avec ce code.
    Exited(i32),
    /// L'application a demandé un redémarrage.
    Reset,
    /// L'utilisateur a quitté l'application avec `Home`.
    HomePressed,
    /// Le budget d'instructions est épuisé.
    Budget,
    Fault(Fault),
}

