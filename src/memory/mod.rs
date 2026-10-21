//! Mémoire émulée : zones de flash et de RAM avec droits d'accès, allocation paresseuse et suivi
//! de ce que le programme lit, écrit et exécute.
//!
//! [`Memory`] implémente le trait `cpu::Bus` : c'est ce que le processeur voit. L'hôte, lui,
//! dispose des accès « directs » (`read_bytes`, `write_bytes`…) qui ignorent les droits : ils
//! servent à fabriquer l'image de l'appareil et à inspecter la mémoire, et ne comptent pas comme
//! une activité du programme (ils ne marquent pas les lignes comme lues ou écrites par lui).

mod bus;
mod error;
mod region;

pub use error::MemoryError;
pub use region::{
    ChangedRange, LINE_SIZE, PAGE_SIZE, Perms, RegionInfo, RegionKind, RegionSpec, access,
};

use region::Region;

/// L'espace d'adressage de l'appareil émulé.
pub struct Memory {
    regions: Vec<Region>,
    /// Indice de la dernière zone utilisée (la plupart des accès se suivent dans la même zone).
    last: usize,
    /// Compteur incrémenté à chaque écriture ; sert à dater les modifications.
    tick: u64,
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            last: 0,
            tick: 0,
        }
    }

    /// Ajoute une zone. Elle ne doit chevaucher aucune zone existante.
    pub fn add_region(&mut self, spec: RegionSpec) -> Result<(), MemoryError> {
        let bad = spec.size == 0
            || spec.base % PAGE_SIZE != 0
            || spec.size % PAGE_SIZE != 0
            || u64::from(spec.base) + u64::from(spec.size) > 1 << 32;
        if bad {
            return Err(MemoryError::BadRegion {
                name: spec.name,
                base: spec.base,
                size: spec.size,
            });
        }
        let end = u64::from(spec.base) + u64::from(spec.size);
        if let Some(other) = self.regions.iter().find(|r| {
            u64::from(r.spec.base) < end
                && u64::from(spec.base) < u64::from(r.spec.base) + u64::from(r.spec.size)
        }) {
            return Err(MemoryError::Overlap {
                name: spec.name,
                other: other.spec.name.clone(),
            });
        }
        self.regions.push(Region::new(spec));
        Ok(())
    }

