//! Cache des instructions décodées.
//!
//! Décoder une instruction coûte plus cher que l'exécuter : la plupart du temps, un programme
//! repasse en boucle sur les mêmes instructions. Le cache garde, pour une adresse, l'instruction
//! décodée et sa taille. Il est « à correspondance directe » : l'adresse désigne une seule case.
//!
//! Le contenu de la mémoire peut changer (code copié en RAM, flash reprogrammée par l'hôte) ; le
//! bus dit alors que son « époque du code » a changé (`Bus::code_epoch`) et tout le cache est
//! vidé. Un bus qui ne sait pas suivre ses modifications répond `None` : le cache est désactivé.

use super::instr::Instr;

/// Nombre de cases (puissance de deux). Une instruction occupe 2 ou 4 octets : 4096 cases
/// couvrent 8 Kio de code sans conflit.
const SLOTS: usize = 4096;

#[derive(Clone, Copy)]
struct Entry {
    address: u32,
    size: u8,
    instr: Instr,
}

const EMPTY: Entry = Entry {
    address: u32::MAX,
    size: 0,
    instr: Instr::Nop,
};

#[derive(Clone, Default)]
pub(super) struct DecodeCache {
    entries: Vec<Entry>,
    epoch: Option<u64>,
}

impl std::fmt::Debug for DecodeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DecodeCache {{ epoch: {:?} }}", self.epoch)
    }
}

impl DecodeCache {
    fn slot(address: u32) -> usize {
        (address >> 1) as usize & (SLOTS - 1)
    }

    /// Prend en compte l'époque du code annoncée par le bus ; vide le cache si elle a changé.
    #[inline]
    fn sync(&mut self, epoch: Option<u64>) {
        if self.epoch != epoch || self.entries.is_empty() {
            self.entries.clear();
            if epoch.is_some() {
                self.entries.resize(SLOTS, EMPTY);
            }
            self.epoch = epoch;
        }
    }

    /// L'instruction décodée à `address` et sa taille, si elle est en cache.
    #[inline]
    pub fn lookup(&mut self, address: u32, epoch: Option<u64>) -> Option<(Instr, u8)> {
        self.sync(epoch);
        let entry = self.entries.get(Self::slot(address))?;
        (entry.address == address).then_some((entry.instr, entry.size))
    }

    #[inline]
    pub fn insert(&mut self, address: u32, size: u8, instr: Instr) {
        if let Some(entry) = self.entries.get_mut(Self::slot(address)) {
            *entry = Entry {
                address,
                size,
                instr,
            };
        }
    }
}
