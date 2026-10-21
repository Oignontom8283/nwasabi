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

/// Crée les zones de l'appareil décrit par `profile`.
///
/// - la flash externe de 8 Mio (lecture et exécution seulement : l'application ne peut pas
///   l'écrire) ;
/// - la SRAM accessible à l'application ;
/// - la SRAM du noyau, qui existe mais où l'application n'a aucun droit ;
/// - la flash interne (bootloader), vide : nwasabi n'embarque aucun code d'Epsilon.
///
/// `ram_fill` est la valeur initiale des octets de RAM jamais écrits.
pub fn build_memory(profile: &FirmwareProfile, ram_fill: u8) -> Result<Memory, MemoryError> {
    let mut memory = Memory::new();
    memory.add_region(region(
        "flash_ext",
        EXTERNAL_FLASH_BASE,
        EXTERNAL_FLASH_SIZE,
        Perms::READ_EXECUTE,
        RegionKind::Flash,
        0xff,
    ))?;
    let kernel_sram = profile.kernel_sram_base();
    memory.add_region(region(
        "sram",
        profile.sram_base(),
        kernel_sram - profile.sram_base(),
        Perms::READ_WRITE_EXECUTE,
        RegionKind::Ram,
        ram_fill,
    ))?;
    memory.add_region(region(
        "sram_kernel",
        kernel_sram,
        profile.kernel_sram_size,
        Perms::NONE,
        RegionKind::Reserved,
        0,
    ))?;
    match profile.model {
        Model::N0110 | Model::N0115 => {
            memory.add_region(region(
                "internal_flash",
                INTERNAL_FLASH_BASE,
                0x1_0000,
                Perms::READ_EXECUTE,
                RegionKind::Flash,
                0xff,
            ))?;
            memory.add_region(region(
                "bootloader_itcm",
                ITCM_BASE,
                0x1_0000,
                Perms::READ_EXECUTE,
                RegionKind::Flash,
                0xff,
            ))?;
        }
        Model::N0120 => {
            memory.add_region(region(
                "internal_flash",
                INTERNAL_FLASH_BASE,
                0x8_0000,
                Perms::READ_EXECUTE,
                RegionKind::Flash,
                0xff,
            ))?;
        }
    }
    Ok(memory)
}

