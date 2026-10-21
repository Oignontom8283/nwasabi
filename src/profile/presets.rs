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
            expected_userland_version: "26.4.0".into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{FirmwareProfile, Model, ProfileError};

    #[test]
    fn every_preset_is_internally_consistent() {
        for model in Model::ALL {
            let profile = FirmwareProfile::default_for(model);
            assert_eq!(profile.validate(), Ok(()), "{}", model.name());
        }
    }

    #[test]
    fn every_preset_is_measured_in_an_official_firmware() {
        for model in Model::ALL {
            let p = FirmwareProfile::default_for(model);
            assert!(
                matches!(p.provenance, Provenance::Measured { .. }),
                "{}",
                model.name()
            );
            assert_eq!(
                (p.software_version.as_str(), p.commit_hash.as_str()),
                ("26.4.0", "cc9262e")
            );
            // Même disposition de la flash et même `UserlandHeader` sur les trois modèles.
            assert_eq!(p.slot_a.userland_header, 0x9002_0000);
            assert_eq!(p.slot_b.userland_header, 0x9042_0000);
            assert_eq!(p.slot_a.device_name, (0x903f_0000, 0x903f_0400));
            assert_eq!(p.slot_b.device_name, (0x907f_0000, 0x907f_0400));
            assert_eq!(
                (p.storage_size, p.apps_ram.1 - p.sram_base()),
                (0xa800, 0x37000)
            );
            assert_eq!(p.trampoline_address(), 0x9002_0038);
        }
    }

    #[test]
    fn n0110_values_are_those_measured_in_the_official_firmware() {
        let p = FirmwareProfile::epsilon_26_4_0(Model::N0110);
        assert_eq!(p.slot_a.apps_flash, (0x9020_0000, 0x903f_0000));
        assert_eq!(p.slot_b.apps_flash, (0x9060_0000, 0x907f_0000));
        assert_eq!((p.storage_address, p.storage_size), (0x2000_657c, 0xa800));
        assert_eq!(p.apps_ram, (0x2001_18a4, 0x2003_7000));
        assert_eq!(
            (p.process_stack_start, p.kernel_stack_start),
            (0x2003_eff8, 0x2003_f3f8)
        );
        assert_eq!((p.security_level, p.payload_length), (2, 1_610_080));
        assert_eq!(p.fcc_id, "2ALWP-N0110");
    }

    #[test]
    fn n0115_differs_from_n0110_only_by_its_fcc_id() {
        let n0110 = FirmwareProfile::epsilon_26_4_0(Model::N0110);
        let n0115 = FirmwareProfile::epsilon_26_4_0(Model::N0115);
        assert_eq!(n0115.fcc_id, "NA");
        let normalized = FirmwareProfile {
            model: Model::N0110,
            provenance: n0110.provenance,
            fcc_id: n0110.fcc_id.clone(),
            ..n0115
        };
        assert_eq!(normalized, n0110);
    }

    #[test]
    fn n0120_has_its_own_sram_and_a_larger_apps_area() {
        let p = FirmwareProfile::epsilon_26_4_0(Model::N0120);
        assert_eq!(p.slot_info_address(), 0x2400_0000);
        assert_eq!(
            (p.storage_address, p.apps_ram),
            (0x2400_657c, (0x2401_18a4, 0x2403_7000))
        );
        assert_eq!(
            (p.process_stack_start, p.kernel_stack_start),
            (0x2403_eff8, 0x2403_f3f8)
        );
        // Le userland du N0120 est plus petit : la zone des applications commence plus tôt (1,5 Mio).
        assert_eq!(p.slot_a.apps_flash, (0x9018_0000, 0x903f_0000));
        assert_eq!((p.security_level, p.payload_length), (2, 1_556_440));
        assert_eq!(p.fcc_id, "2ALWP-N0120");
    }

    #[test]
    fn the_stack_sits_right_below_the_kernel_ram() {
        for model in Model::ALL {
            let p = FirmwareProfile::default_for(model);
            // Pile du userland : 32 Kio juste sous la RAM du noyau, moins 8 octets.
            assert_eq!(p.apps_ram.1 + 0x8000 - 8, p.process_stack_start);
            assert_eq!(p.process_stack_start + 8, p.kernel_sram_base());
        }
    }

    #[test]
    fn the_active_slot_selects_the_published_layout() {
        let mut p = FirmwareProfile::default_for(Model::N0110);
        assert_eq!(p.active().userland_header, 0x9002_0000);
        assert_eq!(p.apps_flash(), (0x9020_0000, 0x903f_0000));
        p.active_slot = Slot::B;
        assert_eq!(p.active().userland_header, 0x9042_0000);
        assert_eq!(p.apps_flash(), (0x9060_0000, 0x907f_0000));
        assert_eq!(p.trampoline_address(), 0x9042_0038);
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn invalid_profiles_are_detected() {
        let mut p = FirmwareProfile::default_for(Model::N0110);
        p.software_version = "26.4.0.1234".into();
        p.serial_number = "court".into();
        p.apps_ram.0 = p.apps_ram.1;
        p.slot_a.userland_header = 0x8000_0000;
        let errors = p.validate().unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ProfileError::BadString {
                field: "software_version",
                ..
            }
        )));
        assert!(errors.iter().any(|e| matches!(
            e,
            ProfileError::BadString {
                field: "serial_number",
                ..
            }
        )));
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ProfileError::OutOfBounds { field: "apps_ram" }))
        );
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ProfileError::OutOfBounds { field: "slot_a" }))
        );
    }
}
