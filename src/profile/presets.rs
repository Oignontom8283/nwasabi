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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{FirmwareProfile, Model};

    #[test]
    fn every_preset_is_internally_consistent() {
        for model in Model::ALL {
            let profile = FirmwareProfile::default_for(model);
            assert_eq!(profile.validate(), Ok(()), "{}", model.name());
        }
    }

    #[test]
    fn n0110_values_are_those_measured_in_the_official_firmware() {
        let p = FirmwareProfile::epsilon_26_4_0(Model::N0110);
        assert!(matches!(p.provenance, Provenance::Measured { .. }));
        assert_eq!(
            (p.software_version.as_str(), p.commit_hash.as_str()),
            ("26.4.0", "cc9262e")
        );
        assert_eq!(p.slot_a.userland_header, 0x9002_0000);
        assert_eq!(p.slot_a.apps_flash, (0x9020_0000, 0x903f_0000));
        assert_eq!(p.slot_a.device_name, (0x903f_0000, 0x903f_0400));
        assert_eq!(p.slot_b.userland_header, 0x9042_0000);
        assert_eq!(p.slot_b.apps_flash, (0x9060_0000, 0x907f_0000));
        assert_eq!(p.slot_b.device_name, (0x907f_0000, 0x907f_0400));
        assert_eq!((p.storage_address, p.storage_size), (0x2000_657c, 0xa800));
        assert_eq!(p.apps_ram, (0x2001_18a4, 0x2003_7000));
        assert_eq!(
            (p.process_stack_start, p.kernel_stack_start),
            (0x2003_eff8, 0x2003_f3f8)
        );
        assert_eq!(p.trampoline_address(), 0x9002_0038);
        assert_eq!(p.fcc_id, "2ALWP-N0110");
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
    fn derived_profiles_shift_the_sram_addresses_only() {
        let measured = FirmwareProfile::epsilon_26_4_0(Model::N0110);
        let n0120 = FirmwareProfile::epsilon_26_4_0(Model::N0120);
        assert!(matches!(n0120.provenance, Provenance::Derived { .. }));
        assert_eq!(n0120.slot_a, measured.slot_a);
        assert_eq!(n0120.storage_address, 0x2400_657c);
        assert_eq!(n0120.apps_ram, (0x2401_18a4, 0x2403_7000));
        assert_eq!(n0120.slot_info_address(), 0x2400_0000);
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
            crate::profile::ProfileError::BadString {
                field: "software_version",
                ..
            }
        )));
        assert!(errors.iter().any(|e| matches!(
            e,
            crate::profile::ProfileError::BadString {
                field: "serial_number",
                ..
            }
        )));
        assert!(errors.iter().any(|e| matches!(
            e,
            crate::profile::ProfileError::OutOfBounds { field: "apps_ram" }
        )));
        assert!(errors.iter().any(|e| matches!(
            e,
            crate::profile::ProfileError::OutOfBounds { field: "slot_a" }
        )));
    }
}
