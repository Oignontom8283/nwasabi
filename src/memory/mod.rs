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
mod watch;

pub use error::MemoryError;
pub use region::{
    ChangedRange, LINE_SIZE, PAGE_SIZE, Perms, RegionInfo, RegionKind, RegionSpec, access,
};
pub use watch::{WatchHit, WatchId, WatchKind};

use region::Region;

/// L'espace d'adressage de l'appareil émulé.
pub struct Memory {
    regions: Vec<Region>,
    /// Indice de la dernière zone utilisée (la plupart des accès se suivent dans la même zone).
    last: usize,
    /// Compteur incrémenté à chaque écriture ; sert à dater les modifications.
    tick: u64,
    watches: watch::Watchpoints,
    /// Change quand du code déjà exécuté a pu être modifié (voir `Bus::code_epoch`).
    code_epoch: u64,
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
            watches: watch::Watchpoints::default(),
            code_epoch: 0,
        }
    }

    /// Ajoute une zone. Elle ne doit chevaucher aucune zone existante.
    pub fn add_region(&mut self, spec: RegionSpec) -> Result<(), MemoryError> {
        let bad = spec.size == 0
            || !spec.base.is_multiple_of(PAGE_SIZE)
            || !spec.size.is_multiple_of(PAGE_SIZE)
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

    /// Les zones déclarées, dans l'ordre de création.
    pub fn regions(&self) -> Vec<RegionInfo> {
        self.regions
            .iter()
            .map(|r| RegionInfo {
                name: r.spec.name.clone(),
                base: r.spec.base,
                size: r.spec.size,
                perms: r.spec.perms,
                kind: r.spec.kind,
            })
            .collect()
    }

    /// Indice de la zone qui contient `[address, address + len)`, si elle existe.
    fn find(&mut self, address: u32, len: u32) -> Option<usize> {
        if self
            .regions
            .get(self.last)
            .is_some_and(|r| r.contains(address, len))
        {
            return Some(self.last);
        }
        let found = self.regions.iter().position(|r| r.contains(address, len))?;
        self.last = found;
        Some(found)
    }

    /// Vrai si `address` est dans une zone.
    pub fn is_mapped(&mut self, address: u32) -> bool {
        self.find(address, 1).is_some()
    }

    // ---- accès directs de l'hôte (ignorent les droits) --------------------------------------

    /// Lit `out.len()` octets sans passer par les droits d'accès ni compter comme une lecture
    /// du programme.
    pub fn read_into(&mut self, address: u32, out: &mut [u8]) -> Result<(), MemoryError> {
        let len = out.len() as u32;
        let index = self
            .find(address, len)
            .ok_or(MemoryError::Unmapped { address })?;
        let region = &self.regions[index];
        region.copy_out(address - region.spec.base, out);
        Ok(())
    }

    /// Lit `len` octets (accès de l'hôte).
    pub fn read_bytes(&mut self, address: u32, len: u32) -> Result<Vec<u8>, MemoryError> {
        let mut out = vec![0; len as usize];
        self.read_into(address, &mut out)?;
        Ok(out)
    }

    /// Lit un mot de 32 bits little-endian (accès de l'hôte).
    pub fn read_u32(&mut self, address: u32) -> Result<u32, MemoryError> {
        let mut b = [0u8; 4];
        self.read_into(address, &mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    /// Écrit des octets, même dans une zone en lecture seule (accès de l'hôte, par exemple pour
    /// « flasher » une application). La modification est datée : voir [`Memory::changes_since`].
    pub fn write_bytes(&mut self, address: u32, data: &[u8]) -> Result<(), MemoryError> {
        let index = self
            .find(address, data.len() as u32)
            .ok_or(MemoryError::Unmapped { address })?;
        self.tick += 1;
        let tick = self.tick;
        let region = &mut self.regions[index];
        let offset = address - region.spec.base;
        if region.has_access(offset, data.len() as u32, access::EXECUTE) {
            self.code_epoch += 1;
        }
        let region = &mut self.regions[index];
        region.copy_in(offset, data);
        region.stamp(offset, data.len() as u32, tick);
        Ok(())
    }

    /// Écrit un mot de 32 bits little-endian (accès de l'hôte).
    pub fn write_u32(&mut self, address: u32, value: u32) -> Result<(), MemoryError> {
        self.write_bytes(address, &value.to_le_bytes())
    }

    /// Remplit `[address, address + len)` avec `value` (accès de l'hôte).
    pub fn fill(&mut self, address: u32, len: u32, value: u8) -> Result<(), MemoryError> {
        self.write_bytes(address, &vec![value; len as usize])
    }

    // ---- suivi ---------------------------------------------------------------------------------

    /// Indicateurs d'accès de la zone `name`, un octet par ligne de 32 octets
    /// (combinaison de [`access::READ`], [`access::WRITE`], [`access::EXECUTE`]).
    pub fn access_map(&self, name: &str) -> Option<&[u8]> {
        self.regions
            .iter()
            .find(|r| r.spec.name == name)
            .map(|r| r.access.as_slice())
    }

    /// Efface tous les indicateurs d'accès (par exemple au lancement d'une application).
    pub fn clear_access(&mut self) {
        // Les lignes ne portent plus « exécuté » : les modifications du code ne seraient plus vues.
        self.code_epoch += 1;
        for r in &mut self.regions {
            r.access.fill(0);
        }
    }

    /// Valeur actuelle du compteur de modifications ; à passer plus tard à [`Memory::changes_since`].
    pub fn change_cursor(&self) -> u64 {
        self.tick
    }

    /// Plages modifiées (par le programme ou par l'hôte) depuis `cursor`, fusionnées par zone et
    /// triées par adresse. La granularité est la ligne de 32 octets.
    pub fn changes_since(&self, cursor: u64) -> Vec<ChangedRange> {
        let mut ranges: Vec<ChangedRange> = Vec::new();
        let mut order: Vec<&Region> = self.regions.iter().collect();
        order.sort_by_key(|r| r.spec.base);
        for region in order {
            let Some(stamps) = &region.stamps else {
                continue;
            };
            let mut open: Option<(usize, usize)> = None;
            for (line, &stamp) in stamps.iter().enumerate() {
                match (stamp > cursor, open) {
                    (true, Some((start, _))) => open = Some((start, line)),
                    (true, None) => open = Some((line, line)),
                    (false, Some((start, end))) => {
                        ranges.push(line_span(region.spec.base, start, end));
                        open = None;
                    }
                    (false, None) => {}
                }
            }
            if let Some((start, end)) = open {
                ranges.push(line_span(region.spec.base, start, end));
            }
        }
        ranges
    }

    /// Observe les accès du programme à `[start, start + len)`. Les accès de l'hôte ne comptent
    /// pas. Avec `stop`, l'émulateur s'interrompt au premier accès (voir `Emulator::run`).
    pub fn watch(&mut self, start: u32, len: u32, kind: WatchKind, stop: bool) -> WatchId {
        self.watches.add(start, len, kind, stop)
    }

    /// Retire un point d'observation ; faux s'il n'existait pas.
    pub fn unwatch(&mut self, id: WatchId) -> bool {
        self.watches.remove(id)
    }

    /// Les accès observés depuis le dernier appel (les plus anciens sont perdus au-delà de 4096).
    pub fn take_watch_hits(&mut self) -> Vec<WatchHit> {
        self.watches.take_hits()
    }

    /// Premier accès à un point d'observation « avec arrêt », s'il y en a eu un.
    pub fn take_watch_stop(&mut self) -> Option<WatchHit> {
        self.watches.take_stop()
    }

    /// Vrai s'il existe un point d'observation qui interrompt l'exécution.
    pub fn has_stopping_watch(&self) -> bool {
        self.watches.has_stopping()
    }

    /// Plus petite et plus grande adresses des lignes de `[start, end)` qui portent l'indicateur
    /// `flag` (voir [`access`]) : mesure de l'étendue réellement utilisée d'une zone.
    pub fn access_extent(&self, start: u32, end: u32, flag: u8) -> Option<(u32, u32)> {
        let region = self.regions.iter().find(|r| r.contains(start, 1))?;
        let base = region.spec.base;
        let first = ((start - base) / LINE_SIZE) as usize;
        let last = ((end.min(base + region.spec.size) - base).div_ceil(LINE_SIZE)) as usize;
        let lines = region.access.get(first..last)?;
        let low = lines.iter().position(|&a| a & flag != 0)?;
        let high = lines.iter().rposition(|&a| a & flag != 0)?;
        Some((
            base + (first + low) as u32 * LINE_SIZE,
            base + (first + high + 1) as u32 * LINE_SIZE,
        ))
    }

    /// Nombre de pages de 4 Kio effectivement allouées dans la zone `name` (consommation réelle).
    pub fn allocated_pages(&self, name: &str) -> Option<usize> {
        self.regions
            .iter()
            .find(|r| r.spec.name == name)
            .map(Region::allocated_pages)
    }
}

fn line_span(base: u32, first: usize, last: usize) -> ChangedRange {
    let start = base + first as u32 * LINE_SIZE;
    ChangedRange {
        start,
        len: (last - first + 1) as u32 * LINE_SIZE,
    }
}

#[cfg(test)]
mod tests;
