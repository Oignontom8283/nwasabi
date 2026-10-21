//! Le contenu du tampon du système de fichiers : une suite d'enregistrements.
//!
//! ```text
//! enregistrement = [taille u16][nom complet, terminé par un zéro][corps]
//! taille         = 2 + longueur du nom + 1 + longueur du corps   (elle compte ses propres 2 octets)
//! fin            = une taille égale à 0
//! ```
//!
//! Ces fonctions travaillent sur une copie du tampon (un `&[u8]`) : elles n'ont aucun accès à la
//! mémoire de l'appareil, ce qui les rend faciles à tester. Elles ne s'appuient sur aucun index
//! gardé en mémoire : le tampon est relu à chaque opération, car l'application émulée peut l'avoir
//! modifié, voire abîmé.

use super::FsError;

/// Taille minimale d'un enregistrement : sa taille (2), un nom d'au moins un caractère et son zéro.
const MIN_RECORD: usize = 4;
/// Octets réservés à la marque de fin.
const END_MARK: usize = 2;

/// Un enregistrement trouvé dans le tampon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Position de l'enregistrement dans le tampon.
    pub offset: usize,
    /// Taille de l'enregistrement entier, en-tête compris.
    pub size: usize,
    pub name: String,
    /// Position du corps dans le tampon.
    pub body_offset: usize,
    pub body_len: usize,
}

/// Résultat de la lecture d'un tampon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub records: Vec<Record>,
    /// Octets occupés, marque de fin comprise (ou jusqu'à l'endroit où le tampon est abîmé).
    pub used: usize,
    /// Position où le tampon cesse d'être lisible, s'il l'est.
    pub corrupt_at: Option<usize>,
}

/// Lit tous les enregistrements. Une structure abîmée n'est pas une erreur : on rend ce qui a pu
/// être lu avant, et la position du problème.
pub fn parse(buffer: &[u8]) -> Parsed {
    let mut records = Vec::new();
    let mut offset = 0;
    let corrupt = |records, offset| Parsed {
        records,
        used: offset,
        corrupt_at: Some(offset),
    };
    loop {
        let Some(head) = buffer.get(offset..offset + 2) else {
            return corrupt(records, offset);
        };
        let size = usize::from(u16::from_le_bytes([head[0], head[1]]));
        if size == 0 {
            return Parsed {
                records,
                used: offset + END_MARK,
                corrupt_at: None,
            };
        }
        let Some(record) = buffer
            .get(offset..offset + size)
            .filter(|_| size >= MIN_RECORD)
        else {
            return corrupt(records, offset);
        };
        let Some(nul) = record[2..].iter().position(|&b| b == 0) else {
            return corrupt(records, offset);
        };
        records.push(Record {
            offset,
            size,
            name: String::from_utf8_lossy(&record[2..2 + nul]).into_owned(),
            body_offset: offset + 2 + nul + 1,
            body_len: size - 2 - nul - 1,
        });
        offset += size;
    }
}

/// Vérifie qu'un nom complet (`nom.extension`) est acceptable.
pub fn check_name(name: &str) -> Result<(), FsError> {
    let dot = name.rfind('.');
    let ok = !name.is_empty()
        && !name.contains('\0')
        && name.len() < 256
        && dot.is_some_and(|i| i > 0 && i + 1 < name.len());
    if ok {
        Ok(())
    } else {
        Err(FsError::InvalidName(name.to_owned()))
    }
}

