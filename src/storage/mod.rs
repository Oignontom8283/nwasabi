//! Le système de fichiers de la calculatrice, vu depuis l'hôte.
//!
//! Sur l'appareil, les fichiers vivent dans un tampon de RAM (voir [`records`]) que le système et
//! les applications lisent et modifient directement. nwasabi n'en garde donc **aucune copie** :
//! chaque opération relit le tampon dans la mémoire émulée, et les écritures de l'hôte le
//! modifient en place. L'hôte n'agit qu'entre deux appels à `Emulator::run`, donc jamais au
//! milieu d'une instruction : ses modifications sont atomiques pour l'application.

pub mod records;

use thiserror::Error;

use crate::memory::{Memory, MemoryError};

use records::Record;

/// Ce qui peut échouer en manipulant les fichiers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FsError {
    #[error("fichier introuvable : {0}")]
    NotFound(String),
    #[error("le fichier existe déjà : {0}")]
    Exists(String),
    /// Le nom doit être de la forme `nom.extension`, sans zéro.
    #[error("nom invalide (attendu nom.extension) : {0:?}")]
    InvalidName(String),
    /// Le tampon est abîmé à cette position (souvent par l'application) : lecture de ce qui
    /// précède possible, écriture refusée.
    #[error("système de fichiers abîmé à l'octet {offset}")]
    Corrupt { offset: usize },
    /// Plus assez de place : `needed` octets demandés pour l'enregistrement.
    #[error("plus de place : {needed} octets demandés, {available} libres")]
    NoSpace { needed: usize, available: usize },
    /// Un enregistrement ne peut pas dépasser 65 535 octets.
    #[error("fichier trop grand pour un enregistrement")]
    TooLarge,
    #[error(transparent)]
    Memory(#[from] MemoryError),
}

/// Sorte de fichier, d'après son extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    /// Script Python (`.py`) : le corps est un octet « importé automatiquement » puis le texte.
    Python,
    /// Réglages du système (`.sys`) : à ne pas supprimer.
    System,
    /// Tout le reste (formats instables d'une version à l'autre) : traité comme des octets.
    Other(String),
}

impl FileKind {
    fn of(name: &str) -> Self {
        match name.rsplit_once('.').map(|(_, ext)| ext) {
            Some("py") => Self::Python,
            Some("sys") => Self::System,
            Some(ext) => Self::Other(ext.to_owned()),
            None => Self::Other(String::new()),
        }
    }
}

/// Un fichier de la liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub name: String,
    pub kind: FileKind,
    /// Taille du corps, en octets.
    pub size: usize,
    /// Position de l'enregistrement dans le tampon.
    pub offset: usize,
}

impl FileInfo {
    fn of(record: &Record) -> Self {
        Self {
            kind: FileKind::of(&record.name),
            name: record.name.clone(),
            size: record.body_len,
            offset: record.offset,
        }
    }
}

/// La liste des fichiers et l'état du tampon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub files: Vec<FileInfo>,
    /// Position où le tampon est abîmé, s'il l'est.
    pub corrupt_at: Option<usize>,
}

/// Occupation du tampon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Taille du tampon.
    pub total: usize,
    /// Octets utilisés, marque de fin comprise.
    pub used: usize,
    pub free: usize,
    pub records: usize,
}

/// Script Python tel que le voit l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonSource {
    /// Le script est importé automatiquement au démarrage de l'interpréteur.
    pub auto_import: bool,
    pub text: String,
}

/// Où est le système de fichiers, et de quoi se souvenir d'un appel à l'autre.
#[derive(Debug, Clone)]
pub struct Storage {
    /// Adresse du tampon (juste après le magic de tête).
    buffer_address: u32,
    size: u32,
    last_hash: u64,
    generation: u64,
}

impl Storage {
    /// `magic_address` : adresse du magic de tête ; `size` : taille du tampon qui le suit.
    pub fn new(magic_address: u32, size: u32) -> Self {
        Self {
            buffer_address: magic_address + 4,
            size,
            last_hash: 0,
            generation: 0,
        }
    }

    /// Plage `[début, fin)` du tampon, magic de tête compris.
    pub fn range(&self) -> (u32, u32) {
        (self.buffer_address - 4, self.buffer_address + self.size)
    }

    /// Donne accès aux opérations, sur la mémoire de l'appareil.
    pub fn with<'a>(&'a mut self, memory: &'a mut Memory) -> Fs<'a> {
        Fs {
            storage: self,
            memory,
        }
    }
}

/// Les opérations sur les fichiers.
pub struct Fs<'a> {
    storage: &'a mut Storage,
    memory: &'a mut Memory,
}

fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

impl Fs<'_> {
    /// Une copie du tampon tel qu'il est en mémoire.
    pub fn raw(&mut self) -> Result<Vec<u8>, FsError> {
        Ok(self
            .memory
            .read_bytes(self.storage.buffer_address, self.storage.size)?)
    }

    /// Remplace le tampon, en n'écrivant que la partie qui change.
    fn store(&mut self, old: &[u8], new: &[u8]) -> Result<(), FsError> {
        let Some(first) = old.iter().zip(new).position(|(a, b)| a != b) else {
            return Ok(());
        };
        let last = old
            .iter()
            .zip(new)
            .rposition(|(a, b)| a != b)
            .unwrap_or(first);
        let start = self.storage.buffer_address + first as u32;
        self.memory.write_bytes(start, &new[first..=last])?;
        Ok(())
    }

    /// Remet le tampon dans un état relevé avec [`Fs::raw`] (pour défaire une opération qui a échoué à moitié).
    pub fn restore_raw(&mut self, raw: &[u8]) -> Result<(), FsError> {
        let current = self.raw()?;
        if raw.len() != current.len() {
            return Err(FsError::Corrupt { offset: 0 });
        }
        self.store(&current, raw)
    }

    pub fn list(&mut self) -> Result<Listing, FsError> {
        let parsed = records::parse(&self.raw()?);
        Ok(Listing {
            files: parsed.records.iter().map(FileInfo::of).collect(),
            corrupt_at: parsed.corrupt_at,
        })
    }

    pub fn stats(&mut self) -> Result<Stats, FsError> {
        let buffer = self.raw()?;
        let parsed = records::parse(&buffer);
        Ok(Stats {
            total: buffer.len(),
            used: parsed.used,
            free: buffer.len() - parsed.used,
            records: parsed.records.len(),
        })
    }

    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, FsError> {
        records::read(&self.raw()?, name)
    }

    /// Crée ou remplace un fichier.
    pub fn write(&mut self, name: &str, body: &[u8]) -> Result<(), FsError> {
        let old = self.raw()?;
        let new = records::write(&old, name, body)?;
        self.store(&old, &new)
    }

    pub fn remove(&mut self, name: &str) -> Result<(), FsError> {
        let old = self.raw()?;
        let new = records::remove(&old, name)?;
        self.store(&old, &new)
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError> {
        let old = self.raw()?;
        let new = records::rename(&old, from, to)?;
        self.store(&old, &new)
    }

    /// Lit un script Python.
    pub fn read_python(&mut self, name: &str) -> Result<PythonSource, FsError> {
        let body = self.read(name)?;
        let (flag, text) = body.split_first().map_or((0, &[][..]), |(f, t)| (*f, t));
        let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
        Ok(PythonSource {
            auto_import: flag != 0,
            text: String::from_utf8_lossy(&text[..end]).into_owned(),
        })
    }

    /// Écrit un script Python (octet d'importation, texte, zéro final).
    pub fn write_python(&mut self, name: &str, source: &PythonSource) -> Result<(), FsError> {
        let mut body = vec![u8::from(source.auto_import)];
        body.extend_from_slice(source.text.as_bytes());
        body.push(0);
        self.write(name, &body)
    }

    /// Numéro qui augmente à chaque fois que le tampon a changé depuis le dernier appel, que ce
    /// soit par l'hôte ou par l'application. (Les changements sont détectés à l'appel : deux
    /// modifications entre deux appels ne comptent que pour une.)
    pub fn generation(&mut self) -> Result<u64, FsError> {
        let hash = fnv1a(&self.raw()?);
        if hash != self.storage.last_hash {
            self.storage.last_hash = hash;
            self.storage.generation += 1;
        }
        Ok(self.storage.generation)
    }
}
