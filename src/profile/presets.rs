//! Profils prêts à l'emploi.

use super::Model;
use super::firmware::{FirmwareProfile, Provenance};
use super::layout::{SLOT_B_OFFSET, SlotLayout};

impl FirmwareProfile {
    /// Epsilon 26.4.0 (commit `cc9262e`) sur le modèle demandé.
    ///
    /// Le N0110 est **mesuré** dans le firmware officiel (`stable_n0110.dfu`). Les autres modèles
    /// sont **déduits** : même disposition de la flash et mêmes décalages dans la SRAM, dont le
    /// début change (`0x2400_0000` sur N0120). À remplacer par des mesures quand on aura les
    /// firmwares correspondants.
    pub fn epsilon_26_4_0(model: Model) -> Self {
        // Slot A mesuré sur N0110 : le noyau occupe 64 Kio, suivis d'une copie du bootloader
        // (« extra data », 64 Kio), puis du userland dont l'en-tête est à 0x9002_0000.
        let slot_a = SlotLayout {
            origin: 0x9000_0000,
            kernel_header: 0x9000_0008,
            userland_header: 0x9002_0000,
            apps_flash: (0x9020_0000, 0x903f_0000),
            device_name: (0x903f_0000, 0x903f_0400),
            persisting_bytes: (0x903f_0000, 0x9040_0000),
        };
        // Décalages mesurés dans la SRAM du N0110 (début à 0x2000_0000).
        let sram = model.sram_base();
        let provenance = match model {
            Model::N0110 => Provenance::Measured {
                source: "stable_n0110.dfu (Epsilon 26.4.0)",
            },
            _ => Provenance::Derived {
                from: "N0110, Epsilon 26.4.0",
            },
        };
        FirmwareProfile {
            model,
            provenance,
            software_version: "26.4.0".into(),
            commit_hash: "cc9262e".into(),
            security_level: 2,
            payload_length: 1_610_080,
            slot_a,
            slot_b: slot_a.shifted(SLOT_B_OFFSET),
            sram_size: 0x4_0000,
            kernel_sram_size: 0x1000,
            storage_address: sram + 0x657c,
            storage_size: 0xa800,
            apps_ram: (sram + 0x118a4, sram + 0x37000),
            process_stack_start: sram + 0x3eff8,
            kernel_stack_start: sram + 0x3f3f8,
            fcc_id: format!("2ALWP-{}", model.name()),
            pcb_version: 0,
            serial_number: "0000000000000000".into(),
            device_name: String::new(),
        }
    }

    /// Le profil par défaut d'un modèle : Epsilon 26.4.0.
    pub fn default_for(model: Model) -> Self {
        Self::epsilon_26_4_0(model)
    }
}

