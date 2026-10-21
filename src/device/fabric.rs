//! Fabrication du contenu de l'appareil : écrire en mémoire les structures que l'OS y laisse.

use crate::link::runtime::SVC_DRAW_STRING;
use crate::memory::{Memory, MemoryError};
use crate::profile::{FirmwareProfile, SlotLayout};
use crate::structures::{
    KernelHeader, SLOT_INFO_SIZE, STORAGE_MAGIC, SlotHeader, SlotInfo, TRAMPOLINE_TABLE_OFFSET,
    USERLAND_HEADER_MAGIC, USERLAND_HEADER_SIZE, UserlandHeader, VectorTable,
};

/// Décalage, depuis le `UserlandHeader`, du stub qui sert de `drawString` : `svc #0x80 ; bx lr`.
const DRAW_STRING_STUB_OFFSET: u32 = 0x40;
/// Décalage du stub qui sert de point d'entrée du userland : `b .` (boucle infinie ; le userland
/// n'est jamais exécuté, c'est l'application qui l'est).
const START_STUB_OFFSET: u32 = 0x44;

/// Écrit les structures d'un slot : en-tête de sécurité, `KernelHeader`, `UserlandHeader`, table
/// de vecteurs (avec ses deux petits stubs) et nom de l'appareil.
fn write_slot(
    memory: &mut Memory,
    profile: &FirmwareProfile,
    slot: &SlotLayout,
) -> Result<(), MemoryError> {
    let header = SlotHeader {
        security_level: profile.security_level,
        payload_length: profile.payload_length,
    };
    memory.write_bytes(slot.origin, &header.encode())?;
    let kernel = KernelHeader::new(&profile.software_version, &profile.commit_hash);
    memory.write_bytes(slot.kernel_header, &kernel.encode())?;

    let userland = UserlandHeader {
        header: USERLAND_HEADER_MAGIC,
        expected_software_version: profile.software_version.clone(),
        storage_address: profile.storage_address,
        storage_size: profile.storage_size,
        external_apps_flash: slot.apps_flash,
        external_apps_ram: profile.apps_ram,
        device_name: slot.device_name,
        footer: USERLAND_HEADER_MAGIC,
    };
    let at = slot.userland_header;
    memory.write_bytes(at, &userland.encode())?;

    let vectors = VectorTable {
        initial_stack: profile.process_stack_start,
        start: (at + START_STUB_OFFSET) | 1,
        draw_string: (at + DRAW_STRING_STUB_OFFSET) | 1,
    };
    debug_assert_eq!(
        at + USERLAND_HEADER_SIZE as u32,
        at + TRAMPOLINE_TABLE_OFFSET - 8
    );
    memory.write_bytes(at + USERLAND_HEADER_SIZE as u32, &vectors.encode())?;
    memory.write_bytes(
        at + DRAW_STRING_STUB_OFFSET,
        &[SVC_DRAW_STRING, 0xdf, 0x70, 0x47],
    )?;
    memory.write_bytes(at + START_STUB_OFFSET, &[0xfe, 0xe7, 0x00, 0xbf])?;

    if !profile.device_name.is_empty() {
        let mut name = profile.device_name.clone().into_bytes();
        name.push(0);
        memory.write_bytes(slot.device_name.0, &name)?;
    }
    Ok(())
}

/// Écrit tout ce que l'OS laisse en mémoire après son démarrage : les deux slots, le
/// `SlotInfo` (qui désigne le slot A) et un système de fichiers vide.
pub fn write_structures(memory: &mut Memory, profile: &FirmwareProfile) -> Result<(), MemoryError> {
    write_slot(memory, profile, &profile.slot_a)?;
    write_slot(memory, profile, &profile.slot_b)?;

    let info = SlotInfo::new(profile.slot_a.kernel_header, profile.slot_a.userland_header);
    let at = profile.slot_info_address();
    debug_assert_eq!(info.encode().len(), SLOT_INFO_SIZE);
    memory.write_bytes(at, &info.encode())?;

    // Système de fichiers : un magic avant le tampon, un magic après. Le tampon est vide (le
    // premier enregistrement a une taille de 0) ; le reste de l'objet reste à zéro.
    memory.write_u32(profile.storage_address, STORAGE_MAGIC)?;
    memory.write_u32(
        profile.storage_address + 4 + profile.storage_size,
        STORAGE_MAGIC,
    )?;
    Ok(())
}
