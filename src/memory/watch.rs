//! Points d'observation : signaler (et au besoin interrompre) les accès du programme à une plage.

use std::collections::VecDeque;

/// Quels accès déclenchent un point d'observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchKind {
    Read,
    Write,
    ReadWrite,
}

impl WatchKind {
    fn matches(self, write: bool) -> bool {
        match self {
            Self::Read => !write,
            Self::Write => write,
            Self::ReadWrite => true,
        }
    }
}

/// Identifiant d'un point d'observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchId(pub u32);

/// Un accès qui a déclenché un point d'observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchHit {
    pub id: WatchId,
    pub address: u32,
    /// Largeur de l'accès en octets (1, 2 ou 4).
    pub width: u8,
    pub write: bool,
    /// Valeur écrite, ou valeur lue.
    pub value: u32,
    /// Adresse de l'instruction qui a fait l'accès (connue tant qu'un point d'observation existe).
    pub pc: u32,
}

#[derive(Debug, Clone)]
struct Watch {
    id: WatchId,
    start: u32,
    end: u64,
    kind: WatchKind,
    /// Interrompre l'exécution au premier accès.
    stop: bool,
}

/// Nombre maximal de déclenchements gardés en attente de lecture par l'hôte.
const MAX_HITS: usize = 4096;

/// Les points d'observation d'une mémoire.
#[derive(Debug, Clone, Default)]
pub(super) struct Watchpoints {
    watches: Vec<Watch>,
    next_id: u32,
    hits: VecDeque<WatchHit>,
    /// Déclenchements perdus faute de place.
    pub dropped: u64,
    stop_hit: Option<WatchHit>,
    pc: u32,
}

impl Watchpoints {
    pub fn is_empty(&self) -> bool {
        self.watches.is_empty()
    }

    pub fn add(&mut self, start: u32, len: u32, kind: WatchKind, stop: bool) -> WatchId {
        let id = WatchId(self.next_id);
        self.next_id += 1;
        self.watches.push(Watch {
            id,
            start,
            end: u64::from(start) + u64::from(len),
            kind,
            stop,
        });
        id
    }

    pub fn remove(&mut self, id: WatchId) -> bool {
        let before = self.watches.len();
        self.watches.retain(|w| w.id != id);
        self.watches.len() != before
    }

    pub fn set_pc(&mut self, pc: u32) {
        self.pc = pc;
    }

    pub fn has_stopping(&self) -> bool {
        self.watches.iter().any(|w| w.stop)
    }

    /// Note un accès du programme.
    pub fn check(&mut self, address: u32, width: u8, write: bool, value: u32) {
        let end = u64::from(address) + u64::from(width);
        for w in &self.watches {
            if !w.kind.matches(write) || u64::from(address) >= w.end || end <= u64::from(w.start) {
                continue;
            }
            let hit = WatchHit {
                id: w.id,
                address,
                width,
                write,
                value,
                pc: self.pc,
            };
            if self.hits.len() == MAX_HITS {
                self.hits.pop_front();
                self.dropped += 1;
            }
            self.hits.push_back(hit);
            if w.stop && self.stop_hit.is_none() {
                self.stop_hit = Some(hit);
            }
        }
    }

    pub fn take_hits(&mut self) -> Vec<WatchHit> {
        self.hits.drain(..).collect()
    }

    pub fn take_stop(&mut self) -> Option<WatchHit> {
        self.stop_hit.take()
    }
}
