//! Ce qui arrête l'application : les fautes, les conditions des points d'arrêt et la raison pour laquelle l'exécution rend la main.

use crate::cpu::{BusFault, UsageFault};
use crate::memory::WatchHit;

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

/// Comparaison d'une condition de point d'arrêt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Au moins un bit en commun (`registre & valeur != 0`).
    And,
}

impl Compare {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "==" => Self::Eq,
            "!=" => Self::Ne,
            "<" => Self::Lt,
            "<=" => Self::Le,
            ">" => Self::Gt,
            ">=" => Self::Ge,
            "&" => Self::And,
            _ => return None,
        })
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::And => "&",
        }
    }

    pub(super) fn test(self, left: u32, right: u32) -> bool {
        match self {
            Self::Eq => left == right,
            Self::Ne => left != right,
            Self::Lt => left < right,
            Self::Le => left <= right,
            Self::Gt => left > right,
            Self::Ge => left >= right,
            Self::And => left & right != 0,
        }
    }
}

/// Le point d'arrêt ne s'applique que si un registre vérifie cette comparaison (valeurs non signées).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition {
    /// Numéro de registre : 0 à 12, 13 (sp), 14 (lr), 15 (pc).
    pub register: u8,
    pub compare: Compare,
    pub value: u32,
}

/// Un point d'arrêt, éventuellement conditionnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breakpoint {
    pub address: u32,
    pub condition: Option<Condition>,
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
    /// Un point d'observation « avec arrêt » a été touché (voir `Memory::watch`).
    Watchpoint(WatchHit),
    /// L'exécution est arrivée à une instruction où l'hôte a posé un point d'arrêt (pas encore exécutée).
    Breakpoint {
        pc: u32,
    },
    Fault(Fault),
}
