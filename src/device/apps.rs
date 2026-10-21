//! Les applications externes flashées, relues en mémoire comme le fait Epsilon.

use crate::memory::Memory;
use crate::profile::EXTERNAL_APP_SECTOR;
use crate::structures::{APP_HEADER_SIZE, AppHeader};

/// Une application trouvée en parcourant la zone des applications externes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    /// Adresse du début de l'application (son en-tête).
    pub address: u32,
    pub header: AppHeader,
    /// Nom, s'il est lisible et entièrement dans la zone des applications (sinon Epsilon
    /// refuserait de l'afficher).
    pub name: Option<String>,
}

/// Plus petite adresse multiple d'un secteur qui soit supérieure ou égale à `address`.
fn next_sector_aligned(address: u32) -> u32 {
    address
        .div_ceil(EXTERNAL_APP_SECTOR)
        .wrapping_mul(EXTERNAL_APP_SECTOR)
}

fn read_header(memory: &mut Memory, address: u32) -> Option<AppHeader> {
    let bytes: [u8; APP_HEADER_SIZE] = memory
        .read_bytes(address, APP_HEADER_SIZE as u32)
        .ok()?
        .try_into()
        .ok()?;
    let header = AppHeader::decode(&bytes);
    header.is_valid().then_some(header)
}

/// Nom de l'application, sous les conditions qu'impose Epsilon : l'adresse et chaque caractère
/// doivent se trouver dans `[start, end)`.
fn read_name(
    memory: &mut Memory,
    app: u32,
    header: &AppHeader,
    start: u32,
    end: u32,
) -> Option<String> {
    let first = app.checked_add(header.name_offset)?;
    let mut bytes = Vec::new();
    let mut at = first;
    loop {
        if at < start || at >= end {
            return None;
        }
        let byte = memory.read_bytes(at, 1).ok()?[0];
        if byte == 0 {
            return Some(String::from_utf8_lossy(&bytes).into_owned());
        }
        bytes.push(byte);
        at += 1;
    }
}

