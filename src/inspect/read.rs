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

