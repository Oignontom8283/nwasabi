//! Profils prêts à l'emploi.

use super::Model;
use super::firmware::{FirmwareProfile, Provenance};
use super::layout::{SLOT_B_OFFSET, Slot, SlotLayout};

/// Valeurs d'Epsilon 26.4.0 qui diffèrent d'un modèle à l'autre, relevées dans les firmwares
/// officiels (`stable_n0110.dfu`, `stable_n0115.dfu`, `stable_n0120.dfu`).
struct ModelFacts {
    source: &'static str,
    /// Début de la zone des applications externes (dépend de la taille du userland).
    apps_flash_start: u32,
    /// Taille de la charge signée du slot.
    payload_length: u32,
    /// Chaîne renvoyée par l'appel système « FCC ID ».
    fcc_id: &'static str,
}

fn facts(model: Model) -> ModelFacts {
    match model {
        Model::N0110 => ModelFacts {
            source: "stable_n0110.dfu (Epsilon 26.4.0)",
            apps_flash_start: 0x9020_0000,
            payload_length: 1_610_080,
            fcc_id: "2ALWP-N0110",
        },
        Model::N0115 => ModelFacts {
            source: "stable_n0115.dfu (Epsilon 26.4.0)",
            apps_flash_start: 0x9020_0000,
            payload_length: 1_610_080,
            // Le noyau du N0115 renvoie « NA » (et non « 2ALWP-N0115 »).
            fcc_id: "NA",
        },
        Model::N0120 => ModelFacts {
            source: "stable_n0120.dfu (Epsilon 26.4.0)",
            apps_flash_start: 0x9018_0000,
            payload_length: 1_556_440,
            fcc_id: "2ALWP-N0120",
        },
    }
}

impl FirmwareProfile {
    /// Epsilon 26.4.0 (commit `cc9262e`) sur le modèle demandé, **mesuré** dans le firmware
    /// officiel de ce modèle. Sur les trois modèles : noyau de 64 Kio, copie du bootloader
    /// (« extra data », 64 Kio), `UserlandHeader` à `slot + 0x20000`, pile du userland et système
    /// de fichiers aux mêmes décalages dans la SRAM (dont le début change : `0x2400_0000` sur N0120).
    ///
    /// Valeurs encore inconnues, donc fictives : `pcb_version` et `serial_number`.
    pub fn epsilon_26_4_0(model: Model) -> Self {
        let facts = facts(model);
        let slot_a = SlotLayout {
            origin: 0x9000_0000,
            kernel_header: 0x9000_0008,
            userland_header: 0x9002_0000,
            apps_flash: (facts.apps_flash_start, 0x903f_0000),
            device_name: (0x903f_0000, 0x903f_0400),
            persisting_bytes: (0x903f_0000, 0x9040_0000),
        };
        let sram = model.sram_base();
        FirmwareProfile {
            model,
            provenance: Provenance::Measured {
                source: facts.source,
            },
            software_version: "26.4.0".into(),
            commit_hash: "cc9262e".into(),
            security_level: 2,
            payload_length: facts.payload_length,
            slot_a,
            slot_b: slot_a.shifted(SLOT_B_OFFSET),
            active_slot: Slot::A,
            sram_size: 0x4_0000,
            kernel_sram_size: 0x1000,
            storage_address: sram + 0x657c,
            storage_size: 0xa800,
            apps_ram: (sram + 0x118a4, sram + 0x37000),
            process_stack_start: sram + 0x3eff8,
            kernel_stack_start: sram + 0x3f3f8,
            fcc_id: facts.fcc_id.into(),
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

