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

fn find<'a>(parsed: &'a Parsed, name: &str) -> Option<&'a Record> {
    parsed.records.iter().find(|r| r.name == name)
}

fn require_sound(parsed: &Parsed) -> Result<(), FsError> {
    match parsed.corrupt_at {
        Some(offset) => Err(FsError::Corrupt { offset }),
        None => Ok(()),
    }
}

fn encode(name: &str, body: &[u8]) -> Result<Vec<u8>, FsError> {
    let size = 2 + name.len() + 1 + body.len();
    let size16 = u16::try_from(size).map_err(|_| FsError::TooLarge)?;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&size16.to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out.extend_from_slice(body);
    Ok(out)
}

/// Reconstruit un tampon de `capacity` octets : `head`, `middle`, `tail`, marque de fin, zéros.
fn assemble(capacity: usize, parts: [&[u8]; 3]) -> Result<Vec<u8>, FsError> {
    let used: usize = parts.iter().map(|p| p.len()).sum::<usize>() + END_MARK;
    if used > capacity {
        let available = capacity.saturating_sub(used - parts[1].len() - END_MARK + END_MARK);
        return Err(FsError::NoSpace {
            needed: parts[1].len(),
            available,
        });
    }
    let mut out = Vec::with_capacity(capacity);
    for part in parts {
        out.extend_from_slice(part);
    }
    out.resize(capacity, 0);
    Ok(out)
}

/// Corps du fichier `name`.
pub fn read(buffer: &[u8], name: &str) -> Result<Vec<u8>, FsError> {
    let parsed = parse(buffer);
    let record = find(&parsed, name).ok_or_else(|| FsError::NotFound(name.to_owned()))?;
    Ok(buffer[record.body_offset..record.body_offset + record.body_len].to_vec())
}

/// Crée le fichier à la fin, ou remplace son corps en gardant sa place. Rend le nouveau tampon.
pub fn write(buffer: &[u8], name: &str, body: &[u8]) -> Result<Vec<u8>, FsError> {
    check_name(name)?;
    let parsed = parse(buffer);
    require_sound(&parsed)?;
    let record = encode(name, body)?;
    match find(&parsed, name) {
        Some(old) => assemble(
            buffer.len(),
            [
                &buffer[..old.offset],
                &record,
                &buffer[old.offset + old.size..parsed.used - END_MARK],
            ],
        ),
        None => assemble(
            buffer.len(),
            [&buffer[..parsed.used - END_MARK], &record, &[]],
        ),
    }
}

/// Supprime le fichier : les suivants remontent. Rend le nouveau tampon.
pub fn remove(buffer: &[u8], name: &str) -> Result<Vec<u8>, FsError> {
    let parsed = parse(buffer);
    require_sound(&parsed)?;
    let old = find(&parsed, name).ok_or_else(|| FsError::NotFound(name.to_owned()))?;
    assemble(
        buffer.len(),
        [
            &buffer[..old.offset],
            &[],
            &buffer[old.offset + old.size..parsed.used - END_MARK],
        ],
    )
}

/// Change le nom d'un fichier en gardant sa place et son corps.
pub fn rename(buffer: &[u8], from: &str, to: &str) -> Result<Vec<u8>, FsError> {
    check_name(to)?;
    let parsed = parse(buffer);
    require_sound(&parsed)?;
    let old = find(&parsed, from).ok_or_else(|| FsError::NotFound(from.to_owned()))?;
    if from != to && find(&parsed, to).is_some() {
        return Err(FsError::Exists(to.to_owned()));
    }
    let body = &buffer[old.body_offset..old.body_offset + old.body_len];
    let record = encode(to, body)?;
    assemble(
        buffer.len(),
        [
            &buffer[..old.offset],
            &record,
            &buffer[old.offset + old.size..parsed.used - END_MARK],
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: usize = 64;

    fn empty() -> Vec<u8> {
        vec![0; SIZE]
    }

    fn names(buffer: &[u8]) -> Vec<String> {
        parse(buffer).records.into_iter().map(|r| r.name).collect()
    }

    #[test]
    fn an_empty_buffer_has_no_records() {
        let p = parse(&empty());
        assert!(p.records.is_empty());
        assert_eq!(p.used, 2);
        assert_eq!(p.corrupt_at, None);
    }

    #[test]
    fn records_have_the_documented_layout() {
        let b = write(&empty(), "a.py", b"xy").unwrap();
        // taille = 2 + 4 + 1 + 2 = 9
        assert_eq!(&b[..9], [9, 0, b'a', b'.', b'p', b'y', 0, b'x', b'y']);
        assert_eq!(&b[9..11], [0, 0]);
        let p = parse(&b);
        assert_eq!(p.records[0].body_offset, 7);
        assert_eq!(p.records[0].body_len, 2);
        assert_eq!(p.used, 11);
    }

    #[test]
    fn write_read_replace_keep_order() {
        let mut b = write(&empty(), "one.py", b"1").unwrap();
        b = write(&b, "two.py", b"22").unwrap();
        b = write(&b, "three.py", b"333").unwrap();
        b = write(&b, "two.py", b"longer body").unwrap();
        assert_eq!(names(&b), ["one.py", "two.py", "three.py"]);
        assert_eq!(read(&b, "two.py").unwrap(), b"longer body");
        assert_eq!(read(&b, "three.py").unwrap(), b"333");
        b = write(&b, "two.py", b"").unwrap();
        assert_eq!(read(&b, "two.py").unwrap(), b"");
        assert_eq!(read(&b, "three.py").unwrap(), b"333");
    }

    #[test]
    fn remove_and_rename() {
        let mut b = write(&empty(), "a.py", b"A").unwrap();
        b = write(&b, "b.py", b"B").unwrap();
        b = rename(&b, "a.py", "c.txt").unwrap();
        assert_eq!(names(&b), ["c.txt", "b.py"]);
        assert_eq!(read(&b, "c.txt").unwrap(), b"A");
        assert_eq!(
            rename(&b, "c.txt", "b.py"),
            Err(FsError::Exists("b.py".into()))
        );
        b = remove(&b, "c.txt").unwrap();
        assert_eq!(names(&b), ["b.py"]);
        // enregistrement b.py : 2 + 4 + 1 + 1 octets, plus la marque de fin
        assert_eq!(parse(&b).used, 8 + 2);
        assert_eq!(remove(&b, "zz.py"), Err(FsError::NotFound("zz.py".into())));
    }

    #[test]
    fn freed_space_is_zeroed() {
        let mut b = write(&empty(), "a.py", b"AAAA").unwrap();
        b = remove(&b, "a.py").unwrap();
        assert_eq!(b, empty());
    }

    #[test]
    fn running_out_of_space_is_reported_and_changes_nothing() {
        let b = write(&empty(), "a.py", &[7; 40]).unwrap();
        let err = write(&b, "b.py", &[8; 40]).unwrap_err();
        assert!(matches!(err, FsError::NoSpace { .. }));
        // Exactement plein : 2 (marque de fin) de réserve.
        let full = write(&empty(), "a.py", &[1; SIZE - 2 - 2 - 5]).unwrap();
        assert_eq!(parse(&full).used, SIZE);
        assert!(write(&empty(), "a.py", &[1; SIZE - 2 - 2 - 4]).is_err());
    }

    #[test]
    fn invalid_names_are_refused() {
        for bad in ["", "noext", ".hidden", "a.", "a\0.py"] {
            assert_eq!(
                write(&empty(), bad, b""),
                Err(FsError::InvalidName(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn corruption_is_located_not_fatal() {
        let mut b = write(&empty(), "a.py", b"A").unwrap();
        b = write(&b, "b.py", b"B").unwrap();
        let second = parse(&b).records[1].offset;
        b[second] = 0xff; // taille absurde
        b[second + 1] = 0xff;
        let p = parse(&b);
        assert_eq!(p.corrupt_at, Some(second));
        assert_eq!(p.records.len(), 1);
        assert_eq!(read(&b, "a.py").unwrap(), b"A");
        assert_eq!(
            write(&b, "c.py", b""),
            Err(FsError::Corrupt { offset: second })
        );

        // Un nom sans zéro final.
        let c = [6, 0, b'a', b'b', b'c', b'd', 0, 0];
        assert_eq!(parse(&c).corrupt_at, Some(0));
        // Une taille trop petite.
        assert_eq!(parse(&[3, 0, 1, 0]).corrupt_at, Some(0));
        // Un tampon sans marque de fin.
        assert_eq!(parse(&[]).corrupt_at, Some(0));
    }

    #[test]
    fn a_record_cannot_exceed_sixty_five_thousand_bytes() {
        let big = vec![0u8; 70_000];
        assert_eq!(
            write(&vec![0; 100_000], "a.py", &big),
            Err(FsError::TooLarge)
        );
    }
}
