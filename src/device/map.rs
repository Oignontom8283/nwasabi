//! La carte mémoire d'un appareil : quelles zones existent, où, et avec quels droits.

use crate::memory::{Memory, MemoryError, Perms, RegionKind, RegionSpec};
use crate::profile::{EXTERNAL_FLASH_BASE, EXTERNAL_FLASH_SIZE, FirmwareProfile, Model};

/// Début de la flash interne vue par le bus AXI (le bootloader y vit).
const INTERNAL_FLASH_BASE: u32 = 0x0800_0000;
/// Début de la même flash vue par le bus ITCM (N0110 et N0115).
const ITCM_BASE: u32 = 0x0020_0000;

fn region(
    name: &str,
    base: u32,
    size: u32,
    perms: Perms,
    kind: RegionKind,
    fill: u8,
) -> RegionSpec {
    RegionSpec {
        name: name.to_owned(),
        base,
        size,
        perms,
        kind,
        fill,
    }
}

