//! `.nfd` — Nwasabi Flash Data : des octets à placer à des adresses de la flash externe.
//!
//! ```json
//! { "nfd": 1, "active_slot": "a", "segments": [ { "name": "khi120ab.tar", "address": "0x90400000", "content": "…" } ] }
//! ```
//!
//! Certaines applications ne tiennent pas dans leur propre fichier : elles lisent, à une adresse
//! fixe de la flash, un gros programme ou des données que l'on flashe à part. Un `.nfd` rassemble
//! ces morceaux, chacun avec son adresse, et se charge avec l'application. `active_slot` (facultatif,
//! `"a"` ou `"b"`) dit quel slot du firmware l'appareil doit exécuter, quand les segments occupent l'autre.

use super::{base64, field, number, text};
use crate::json::{self, Value, ValueExt};
use crate::profile::{EXTERNAL_FLASH_BASE, EXTERNAL_FLASH_SIZE, Slot};

/// Version du format.
pub const VERSION: u64 = 1;
/// Nombre maximal de segments d'un document.
pub const MAX_SEGMENTS: usize = 64;

/// Des octets à écrire à partir de `address`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Nom indicatif (en général celui du fichier d'origine).
    pub name: String,
    pub address: u32,
    pub data: Vec<u8>,
}

impl Segment {
    /// Adresse du premier octet qui suit le segment.
    pub fn end(&self) -> u32 {
        self.address + self.data.len() as u32
    }
}

/// Vérifie qu'un segment tient dans la flash externe.
pub fn check(segment: &Segment) -> Result<(), String> {
    let flash_end = EXTERNAL_FLASH_BASE + EXTERNAL_FLASH_SIZE;
    if segment.data.is_empty() {
        return Err("contenu vide".into());
    }
    let end = u64::from(segment.address) + segment.data.len() as u64;
    if segment.address < EXTERNAL_FLASH_BASE || end > u64::from(flash_end) {
        return Err(format!(
            "0x{:08x}..0x{end:08x} sort de la flash externe (0x{EXTERNAL_FLASH_BASE:08x}..0x{flash_end:08x})",
            segment.address
        ));
    }
    Ok(())
}

/// Vérifie une liste de segments : chacun dans la flash, et aucun ne recouvre un autre.
pub fn check_all(segments: &[Segment]) -> Result<(), String> {
    if segments.len() > MAX_SEGMENTS {
        return Err(format!("au plus {MAX_SEGMENTS} segments"));
    }
    for (i, segment) in segments.iter().enumerate() {
        check(segment).map_err(|e| format!("segments[{i}] : {e}"))?;
        for (j, other) in segments.iter().enumerate().take(i) {
            if segment.address < other.end() && other.address < segment.end() {
                return Err(format!("segments[{i}] : recouvre segments[{j}]"));
            }
        }
    }
    Ok(())
}

/// Enregistre des segments.
pub fn export(segments: &[Segment]) -> Value {
    let list: Vec<Value> = segments
        .iter()
        .map(|s| {
            Value::object()
                .with("name", s.name.as_str())
                .with("address", format!("0x{:08x}", s.address))
                .with("content", json::base64_encode(&s.data))
        })
        .collect();
    Value::object().with("nfd", VERSION).with("segments", list)
}

/// Le slot que le document demande d'exécuter, s'il en demande un.
pub fn active_slot(document: &Value) -> Result<Option<Slot>, String> {
    match document.get("active_slot") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => match value.as_str() {
            Some("a") => Ok(Some(Slot::A)),
            Some("b") => Ok(Some(Slot::B)),
            _ => Err("« active_slot » : « a » ou « b » est attendu".into()),
        },
    }
}

/// Les segments d'un document, vérifiés. L'adresse s'écrit en nombre ou en chaîne (`"0x90400000"`).
pub fn parse(document: &Value) -> Result<Vec<Segment>, String> {
    let version = number(document, "nfd")
        .map_err(|_| "ce n'est pas un fichier .nfd (champ « nfd » manquant)".to_owned())?;
    if version != VERSION {
        return Err(format!(
            "version {version} non prise en charge (seule {VERSION} l'est)"
        ));
    }
    let list = field(document, "segments")?
        .as_array()
        .ok_or("« segments » : une liste est attendue")?;
    let mut segments = Vec::new();
    for (i, entry) in list.iter().enumerate() {
        let at = |m: String| format!("segments[{i}] : {m}");
        let address = field(entry, "address")
            .map_err(at)?
            .as_u32()
            .ok_or_else(|| {
                at("« address » : une adresse est attendue (nombre ou « 0x… »)".into())
            })?;
        let name = match entry.get("name") {
            Some(_) => text(entry, "name").map_err(at)?.to_owned(),
            None => String::new(),
        };
        let data = base64(entry, "content").map_err(at)?;
        segments.push(Segment {
            name,
            address,
            data,
        });
    }
    check_all(&segments)?;
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(address: u32, len: usize) -> Segment {
        Segment {
            name: "x".into(),
            address,
            data: vec![7; len],
        }
    }

    #[test]
    fn a_document_round_trips() {
        let segments = vec![segment(0x9040_0000, 5), segment(0x9026_0000, 3)];
        assert_eq!(parse(&export(&segments)).unwrap(), segments);
    }

    #[test]
    fn addresses_can_be_numbers() {
        let doc = Value::object().with("nfd", 1u64).with(
            "segments",
            vec![
                Value::object()
                    .with("address", 0x9040_0000u32)
                    .with("content", json::base64_encode(&[1, 2])),
            ],
        );
        assert_eq!(parse(&doc).unwrap()[0].address, 0x9040_0000);
    }

    #[test]
    fn segments_outside_the_flash_are_refused() {
        assert!(check(&segment(0x2000_0000, 4)).is_err());
        assert!(check(&segment(0x907f_fffe, 4)).is_err());
        assert!(check(&segment(0x907f_fffc, 4)).is_ok());
        assert!(check(&segment(0x9040_0000, 0)).is_err());
    }

    #[test]
    fn overlapping_segments_are_refused() {
        let err = check_all(&[segment(0x9040_0000, 16), segment(0x9040_0008, 16)]).unwrap_err();
        assert!(err.contains("recouvre"), "{err}");
        check_all(&[segment(0x9040_0000, 16), segment(0x9040_0010, 16)]).unwrap();
    }

    #[test]
    fn a_wrong_document_is_explained() {
        let err = parse(&Value::object().with("nfs", 1u64)).unwrap_err();
        assert!(err.contains(".nfd"), "{err}");
    }
}
