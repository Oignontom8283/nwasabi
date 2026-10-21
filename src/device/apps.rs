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

/// Parcourt la zone `[start, end)` comme le fait Epsilon : la première application est au début
/// de la zone ; la suivante est à la fin de la précédente (taille écrite dans son en-tête)
/// arrondie au secteur de 64 Kio ; on s'arrête au premier emplacement sans en-tête valide.
///
/// En mode examen, Epsilon masque toutes les applications externes.
///
/// Particularité reproduite : la taille de l'en-tête n'inclut pas la copie de `.data`. Une
/// application dont cette copie franchit une limite de secteur fait donc « perdre » la
/// suivante (voir `LinkWarning::AppSizeMismatch`).
pub fn list_apps(memory: &mut Memory, start: u32, end: u32, exam_mode: bool) -> Vec<AppEntry> {
    let mut apps = Vec::new();
    if exam_mode {
        return apps;
    }
    let mut address = start;
    while let Some(header) = read_header(memory, address) {
        let name = read_name(memory, address, &header, start, end);
        let next = next_sector_aligned(address.wrapping_add(header.size));
        apps.push(AppEntry {
            address,
            header,
            name,
        });
        // Garde-fous : Epsilon boucle sans fin sur une taille nulle ; nous nous arrêtons.
        let room = u64::from(next) + APP_HEADER_SIZE as u64 <= u64::from(end);
        if next <= address || next < start || !room {
            break;
        }
        address = next;
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_alignment_keeps_aligned_addresses() {
        assert_eq!(next_sector_aligned(0x9020_0000), 0x9020_0000);
        assert_eq!(next_sector_aligned(0x9020_0001), 0x9021_0000);
        assert_eq!(next_sector_aligned(0x9020_ffff), 0x9021_0000);
        assert_eq!(next_sector_aligned(0x9021_0000), 0x9021_0000);
    }
}
