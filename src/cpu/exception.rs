//! Entrée et retour d'exception (pile de contexte, `EXC_RETURN`).
//!
//! Utilisé quand un appel système `svc` n'est pas traité par l'hôte (`SvcAction::Raise`) : le
//! processeur empile son contexte, passe en mode « handler » et saute au vecteur d'exception.
//! C'est le mécanisme que le futur mode « firmware complet » utilisera.

use super::{Bus, Cpu, Stop, UsageFault};

/// Numéro d'exception de `SVCall`.
pub const EXC_SVCALL: u32 = 11;

/// Taille du contexte empilé : 8 mots entiers, plus 18 mots si le contexte flottant est actif.
const FRAME_INTEGER: u32 = 0x20;
const FRAME_FLOAT: u32 = 0x48;
const CONTROL_SPSEL: u32 = 1 << 1;
const CONTROL_FPCA: u32 = 1 << 2;
/// Bit de xPSR qui mémorise qu'un mot de remplissage a aligné la pile sur 8 octets.
const XPSR_ALIGN: u32 = 1 << 9;

impl Cpu {
