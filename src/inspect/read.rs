//! Lecture des structures de l'OS dans la mémoire.

use super::{Found, ModelClass, StorageView, Structures};
use crate::device::list_apps;
use crate::memory::Memory;
use crate::profile::{EXTERNAL_FLASH_BASE, EXTERNAL_FLASH_SIZE};
use crate::structures::{
    KERNEL_HEADER_SIZE, KernelHeader, SLOT_HEADER_SIZE, SLOT_INFO_SIZE, STORAGE_MAGIC, SlotHeader,
    SlotInfo, USERLAND_HEADER_SIZE, UserlandHeader, VectorTable,
};

/// Débuts possibles de la SRAM, et donc emplacements possibles du `SlotInfo`.
const SRAM_CANDIDATES: [u32; 2] = [0x2000_0000, 0x2400_0000];

fn flash_pointer(address: u32) -> bool {
    (EXTERNAL_FLASH_BASE..EXTERNAL_FLASH_BASE + EXTERNAL_FLASH_SIZE).contains(&address)
}

fn read_array<const N: usize>(memory: &mut Memory, address: u32) -> Option<[u8; N]> {
    memory.read_bytes(address, N as u32).ok()?.try_into().ok()
}

fn slot_info_at(memory: &mut Memory, base: u32) -> Option<SlotInfo> {
    let info = SlotInfo::decode(&read_array::<SLOT_INFO_SIZE>(memory, base)?);
    (info.is_valid() && flash_pointer(info.kernel_header) && flash_pointer(info.userland_header))
        .then_some(info)
}

/// Cherche le début de la SRAM comme le ferait une application : le `SlotInfo` est à la première
/// adresse de la SRAM, dont la position dépend du modèle. Renvoie le premier candidat où un
/// `SlotInfo` valide, qui pointe vers la flash, est présent.
pub fn detect_sram_base(memory: &mut Memory) -> Option<u32> {
    SRAM_CANDIDATES
        .into_iter()
        .find(|&base| slot_info_at(memory, base).is_some())
}

/// Relit toutes les structures de l'OS à partir du `SlotInfo`. Ce qui est absent ou illisible vaut
/// `None` ; ce qui est présent mais abîmé est renvoyé avec `valid = false`.
pub fn read_structures(memory: &mut Memory, exam_mode: bool) -> Structures {
    let mut out = Structures {
        sram_base: None,
        model_class: None,
        slot_info: None,
        slot_header: None,
        kernel_header: None,
        userland_header: None,
        vector_table: None,
        storage: None,
        device_name: None,
        apps: Vec::new(),
    };
    let Some(base) = detect_sram_base(memory) else {
        return out;
    };
    out.sram_base = Some(base);
    out.model_class = Some(if base == 0x2400_0000 {
        ModelClass::N0120
    } else {
        ModelClass::N0110OrN0115
    });
    let info = slot_info_at(memory, base).expect("détecté juste avant");
    out.slot_info = Some(Found {
        address: base,
        value: info,
        valid: true,
    });

    // En-tête de sécurité (8 octets) puis KernelHeader juste après.
    let slot_header_at = info.kernel_header.wrapping_sub(SLOT_HEADER_SIZE as u32);
    if let Some(bytes) = read_array::<SLOT_HEADER_SIZE>(memory, slot_header_at) {
        out.slot_header = Some(Found {
            address: slot_header_at,
            value: SlotHeader::decode(&bytes),
            valid: true,
        });
    }
    if let Some(bytes) = read_array::<KERNEL_HEADER_SIZE>(memory, info.kernel_header) {
        let value = KernelHeader::decode(&bytes);
        let valid = value.is_valid();
        out.kernel_header = Some(Found {
            address: info.kernel_header,
            value,
            valid,
        });
    }

    let at = info.userland_header;
    let Some(bytes) = read_array::<USERLAND_HEADER_SIZE>(memory, at) else {
        return out;
    };
    let userland = UserlandHeader::decode(&bytes);
    let valid = userland.is_valid();
    out.userland_header = Some(Found {
        address: at,
        value: userland.clone(),
        valid,
    });
    if let Some(bytes) = read_array::<12>(memory, at + USERLAND_HEADER_SIZE as u32) {
        out.vector_table = Some(Found {
            address: at + USERLAND_HEADER_SIZE as u32,
            value: VectorTable::decode(&bytes),
            valid: true,
        });
    }
    if !valid {
        return out;
    }

    // Système de fichiers : un magic avant le tampon, un autre après.
    let (address, size) = (userland.storage_address, userland.storage_size);
    if let (Ok(head), Ok(foot), Ok(first)) = (
        memory.read_u32(address),
        memory.read_u32(address.wrapping_add(4).wrapping_add(size)),
        memory.read_bytes(address.wrapping_add(4), 2),
    ) {
        out.storage = Some(StorageView {
            address,
            size,
            header_magic_ok: head == STORAGE_MAGIC,
            footer_magic_ok: foot == STORAGE_MAGIC,
            first_record_size: u16::from_le_bytes([first[0], first[1]]),
        });
    }

    // Nom de l'appareil : chaîne terminée par un zéro dans la zone dédiée ; une flash vierge (0xFF)
    // signifie « pas de nom ».
    let (name_start, name_end) = userland.device_name;
    if name_end > name_start
        && let Ok(bytes) = memory.read_bytes(name_start, (name_end - name_start).min(1024))
    {
        let len = bytes
            .iter()
            .position(|&b| b == 0 || b == 0xff)
            .unwrap_or(bytes.len());
        out.device_name = Some(String::from_utf8_lossy(&bytes[..len]).into_owned());
    }

    let (apps_start, apps_end) = userland.external_apps_flash;
    out.apps = list_apps(memory, apps_start, apps_end, exam_mode);
    out
}
