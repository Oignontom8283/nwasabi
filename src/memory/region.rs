//! Zones de mémoire : description publique et stockage interne.

/// Taille d'une page de stockage : une zone n'alloue de mémoire que pour les pages touchées.
pub const PAGE_SIZE: u32 = 4096;
/// Taille d'une ligne de suivi : le suivi des accès se fait par blocs de 32 octets.
pub const LINE_SIZE: u32 = 32;

/// Ce que le programme émulé a le droit de faire dans une zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Perms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Perms {
    /// Lecture et exécution : la flash vue du programme.
    pub const READ_EXECUTE: Self = Self {
        read: true,
        write: false,
        execute: true,
    };
    /// Lecture seule.
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        execute: false,
    };
    /// Lecture, écriture et exécution : la RAM.
    pub const READ_WRITE_EXECUTE: Self = Self {
        read: true,
        write: true,
        execute: true,
    };
    /// Aucun accès (mémoire réservée au noyau).
    pub const NONE: Self = Self {
        read: false,
        write: false,
        execute: false,
    };
}

/// Nature d'une zone, pour l'affichage et l'analyse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    Flash,
    Ram,
    /// Mémoire qui existe mais que le programme ne peut pas toucher (noyau).
    Reserved,
}

/// Description d'une zone de mémoire à créer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSpec {
    pub name: String,
    pub base: u32,
    pub size: u32,
    pub perms: Perms,
    pub kind: RegionKind,
    /// Valeur des octets jamais écrits (0xFF pour une flash effacée).
    pub fill: u8,
}

/// Une zone telle que la voit l'extérieur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionInfo {
    pub name: String,
    pub base: u32,
    pub size: u32,
    pub perms: Perms,
    pub kind: RegionKind,
}

/// Indicateurs d'accès par ligne de 32 octets.
pub mod access {
    pub const READ: u8 = 1;
    pub const WRITE: u8 = 2;
    pub const EXECUTE: u8 = 4;
}

/// Une plage d'adresses qui a changé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangedRange {
    pub start: u32,
    pub len: u32,
}

/// Stockage d'une zone : pages allouées à la demande, indicateurs d'accès, dates des écritures.
pub(super) struct Region {
    pub spec: RegionSpec,
    pages: Vec<Option<Box<[u8; PAGE_SIZE as usize]>>>,
    /// Un octet par ligne : combinaison de [`access`].
    pub access: Vec<u8>,
    /// Date (compteur d'écritures) de la dernière écriture de chaque ligne ; créé à la première écriture.
    pub stamps: Option<Vec<u64>>,
}

impl Region {
    pub fn new(spec: RegionSpec) -> Self {
        let pages = (spec.size / PAGE_SIZE) as usize;
        let lines = (spec.size / LINE_SIZE) as usize;
        Self {
            spec,
            pages: vec![None; pages],
            access: vec![0; lines],
            stamps: None,
        }
    }

    pub fn contains(&self, address: u32, len: u32) -> bool {
        address >= self.spec.base
            && u64::from(address) + u64::from(len)
                <= u64::from(self.spec.base) + u64::from(self.spec.size)
    }

    /// Copie `out.len()` octets à partir de `offset` (déjà vérifié dans la zone).
    pub fn copy_out(&self, offset: u32, out: &mut [u8]) {
        let mut done = 0usize;
        while done < out.len() {
            let at = offset as usize + done;
            let (page, inner) = (at / PAGE_SIZE as usize, at % PAGE_SIZE as usize);
            let n = (out.len() - done).min(PAGE_SIZE as usize - inner);
            match &self.pages[page] {
                Some(p) => out[done..done + n].copy_from_slice(&p[inner..inner + n]),
                None => out[done..done + n].fill(self.spec.fill),
            }
            done += n;
        }
    }

    /// Écrit `data` à partir de `offset` (déjà vérifié dans la zone).
    pub fn copy_in(&mut self, offset: u32, data: &[u8]) {
        let fill = self.spec.fill;
        let mut done = 0usize;
        while done < data.len() {
            let at = offset as usize + done;
            let (page, inner) = (at / PAGE_SIZE as usize, at % PAGE_SIZE as usize);
            let n = (data.len() - done).min(PAGE_SIZE as usize - inner);
            let p = self.pages[page].get_or_insert_with(|| Box::new([fill; PAGE_SIZE as usize]));
            p[inner..inner + n].copy_from_slice(&data[done..done + n]);
            done += n;
        }
    }

