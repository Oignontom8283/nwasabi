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

/// Une page de mémoire, et la copie des pages allouées d'une zone (index de page, contenu).
pub type Page = Box<[u8; PAGE_SIZE as usize]>;
pub type SavedPages = Vec<(usize, Page)>;

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

    #[inline]
    pub fn contains(&self, address: u32, len: u32) -> bool {
        // Décalage depuis le début de la zone : un `address` plus petit que `base` donne un grand nombre et échoue.
        let offset = address.wrapping_sub(self.spec.base);
        u64::from(offset) + u64::from(len) <= u64::from(self.spec.size)
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

    /// Lit `N` octets (1 à 4) à `offset` : chemin rapide quand ils sont dans une même page.
    #[inline]
    pub fn read_small<const N: usize>(&self, offset: u32) -> [u8; N] {
        let at = offset as usize;
        let (page, inner) = (at / PAGE_SIZE as usize, at % PAGE_SIZE as usize);
        if inner + N <= PAGE_SIZE as usize {
            return match &self.pages[page] {
                Some(p) => {
                    let mut out = [0u8; N];
                    out.copy_from_slice(&p[inner..inner + N]);
                    out
                }
                None => [self.spec.fill; N],
            };
        }
        let mut out = [0u8; N];
        self.copy_out(offset, &mut out);
        out
    }

    /// Écrit `N` octets (1 à 4) à `offset` : chemin rapide quand ils sont dans une même page.
    #[inline]
    pub fn write_small<const N: usize>(&mut self, offset: u32, data: [u8; N]) {
        let at = offset as usize;
        let (page, inner) = (at / PAGE_SIZE as usize, at % PAGE_SIZE as usize);
        if inner + N <= PAGE_SIZE as usize {
            let fill = self.spec.fill;
            let p = self.pages[page].get_or_insert_with(|| Box::new([fill; PAGE_SIZE as usize]));
            p[inner..inner + N].copy_from_slice(&data);
        } else {
            self.copy_in(offset, &data);
        }
    }

    /// Comme `mark_access` pour un accès de 1 à 4 octets (au plus deux lignes).
    #[inline]
    pub fn mark_small(&mut self, offset: u32, len: u32, flag: u8) {
        let first = (offset / LINE_SIZE) as usize;
        let last = ((offset + len - 1) / LINE_SIZE) as usize;
        self.access[first] |= flag;
        if last != first {
            self.access[last] |= flag;
        }
    }

    /// Comme `stamp` pour un accès de 1 à 4 octets.
    #[inline]
    pub fn stamp_small(&mut self, offset: u32, len: u32, tick: u64) {
        let lines = (self.spec.size / LINE_SIZE) as usize;
        let stamps = self.stamps.get_or_insert_with(|| vec![0; lines]);
        let first = (offset / LINE_SIZE) as usize;
        let last = ((offset + len - 1) / LINE_SIZE) as usize;
        stamps[first] = tick;
        stamps[last] = tick;
    }

    /// Indique que les lignes touchées par `[offset, offset + len)` ont reçu l'indicateur `flag`.
    pub fn mark_access(&mut self, offset: u32, len: u32, flag: u8) {
        for line in &mut self.access[line_range(offset, len)] {
            *line |= flag;
        }
    }

    /// Vrai si une des lignes de `[offset, offset + len)` porte l'indicateur `flag`.
    pub fn has_access(&self, offset: u32, len: u32, flag: u8) -> bool {
        self.access[line_range(offset, len)]
            .iter()
            .any(|line| line & flag != 0)
    }

    /// Date l'écriture des lignes touchées par `[offset, offset + len)`.
    pub fn stamp(&mut self, offset: u32, len: u32, tick: u64) {
        let lines = (self.spec.size / LINE_SIZE) as usize;
        let stamps = self.stamps.get_or_insert_with(|| vec![0; lines]);
        stamps[line_range(offset, len)].fill(tick);
    }

    /// Copie des pages allouées (index de page, contenu).
    pub fn snapshot_pages(&self) -> SavedPages {
        self.pages
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.as_ref().map(|p| (i, p.clone())))
            .collect()
    }

    /// Remet les pages dans l'état d'une copie : celles qui n'y sont pas redeviennent vierges.
    pub fn restore_pages(&mut self, saved: &[(usize, Page)]) {
        for page in &mut self.pages {
            *page = None;
        }
        for (i, data) in saved {
            self.pages[*i] = Some(data.clone());
        }
    }

    /// Adresses de début des pages réellement allouées.
    pub fn allocated_page_addresses(&self) -> Vec<u32> {
        (0..self.pages.len())
            .filter(|&i| self.pages[i].is_some())
            .map(|i| self.spec.base + i as u32 * PAGE_SIZE)
            .collect()
    }

    /// Nombre de pages réellement allouées.
    pub fn allocated_pages(&self) -> usize {
        self.pages.iter().filter(|p| p.is_some()).count()
    }
}

/// Indices des lignes de 32 octets touchées par `[offset, offset + len)`.
fn line_range(offset: u32, len: u32) -> std::ops::Range<usize> {
    let first = (offset / LINE_SIZE) as usize;
    let last = ((offset + len.max(1) - 1) / LINE_SIZE) as usize;
    first..last + 1
}
