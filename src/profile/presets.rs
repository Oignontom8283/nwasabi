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
    /// Fréquence du processeur en fonctionnement normal (MHz).
    cpu_mhz: u32,
}

fn facts(model: Model) -> ModelFacts {
    match model {
        Model::N0110 => ModelFacts {
            source: "stable_n0110.dfu (Epsilon 26.4.0)",
            apps_flash_start: 0x9020_0000,
            payload_length: 1_610_080,
            fcc_id: "2ALWP-N0110",
            cpu_mhz: 192,
        },
        Model::N0115 => ModelFacts {
            source: "stable_n0115.dfu (Epsilon 26.4.0)",
            apps_flash_start: 0x9020_0000,
            payload_length: 1_610_080,
            // Le noyau du N0115 renvoie « NA » (et non « 2ALWP-N0115 »).
            fcc_id: "NA",
            cpu_mhz: 192,
        },
        Model::N0120 => ModelFacts {
            source: "stable_n0120.dfu (Epsilon 26.4.0)",
            apps_flash_start: 0x9018_0000,
            payload_length: 1_556_440,
            fcc_id: "2ALWP-N0120",
            cpu_mhz: 540,
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
            cpu_mhz: facts.cpu_mhz,
            fcc_id: facts.fcc_id.into(),
            pcb_version: 0,
            serial_number: "0000000000000000".into(),
            device_name: String::new(),
        }
    }

    /// Un appareil comme ceux relevés sur matériel (Epsilon 23.2.6 à 24.11.0, avant la 26) : le
    /// `UserlandHeader` est à `slot + 0x10000` sur N0110 et N0115 (pas de copie du bootloader),
    /// à `slot + 0x20000` sur N0120 ; la zone des applications commence plus tôt (`+0x170000`,
    /// `+0x190000` sur N0120) et la RAM des applications un peu plus loin. C'est la disposition que
    /// supposent NumWorks Extapp Storage, eadkp et Nwagyu : leurs applications y retrouvent le
    /// système de fichiers (voir `default_for`).
    ///
    /// Valeurs relevées sur les appareils de l'auteur : versions, commits, RAM et flash des
    /// applications, slot actif (N0110 : 24.11.0 slot B ; N0115 : 24.3.0 slot A ; N0120 : 23.2.6
    /// slot B). Valeurs déduites des sources d'Epsilon 23.2.3 : position des en-têtes. Valeurs
    /// reprises de la 26.4.0 faute de mieux : adresse du système de fichiers, piles, niveau de
    /// sécurité et taille de la charge signée.
    pub fn epsilon_24(model: Model) -> Self {
        let mut profile = Self::epsilon_26_4_0(model);
        let (version, hash, apps_offset, userland_offset, slot, ram_start, fcc) = match model {
            Model::N0110 => (
                "24.11.0",
                "a58aaa0",
                0x17_0000,
                0x1_0000,
                Slot::B,
                0x12804,
                "2ALWP-N0110",
            ),
            Model::N0115 => (
                "24.3.0",
                "804c50c",
                0x17_0000,
                0x1_0000,
                Slot::A,
                0x12804,
                "NA",
            ),
            Model::N0120 => (
                "23.2.6",
                "22d1daa",
                0x19_0000,
                0x2_0000,
                Slot::B,
                0x12a40,
                "2ALWP-N0120",
            ),
        };
        let slot_a = SlotLayout {
            origin: 0x9000_0000,
            kernel_header: 0x9000_0008,
            userland_header: 0x9000_0000 + userland_offset,
            apps_flash: (0x9000_0000 + apps_offset, 0x903f_0000),
            device_name: (0x903f_0000, 0x903f_0400),
            persisting_bytes: (0x903f_0000, 0x9040_0000),
        };
        let sram = model.sram_base();
        profile.provenance = Provenance::Derived {
            from: "relevés sur appareils (versions, RAM et flash des applications) et sources d'Epsilon 23.2.3",
        };
        profile.software_version = version.into();
        profile.expected_userland_version = version.into();
        profile.commit_hash = hash.into();
        profile.slot_a = slot_a;
        profile.slot_b = slot_a.shifted(SLOT_B_OFFSET);
        profile.active_slot = slot;
        profile.apps_ram = (sram + ram_start, sram + 0x37000);
        profile.fcc_id = fcc.into();
        profile
    }

    /// Le profil par défaut d'un modèle : la disposition des appareils relevés (voir
    /// [`FirmwareProfile::epsilon_24`]). Epsilon 26.4.0 s'obtient avec
    /// [`FirmwareProfile::epsilon_26_4_0`] : sur N0110 et N0115 son `UserlandHeader` est à
    /// `0x90020000`, ce qui trompe la détection du modèle d'eadkp (les applications qui s'en
    /// servent plantent alors en lisant la RAM du N0120).
    pub fn default_for(model: Model) -> Self {
        Self::epsilon_24(model)
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
    fn every_26_4_0_preset_is_measured_in_an_official_firmware() {
        for model in Model::ALL {
            let p = FirmwareProfile::epsilon_26_4_0(model);
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
        let mut p = FirmwareProfile::epsilon_26_4_0(Model::N0110);
        assert_eq!(p.active().userland_header, 0x9002_0000);
        assert_eq!(p.apps_flash(), (0x9020_0000, 0x903f_0000));
        p.active_slot = Slot::B;
        assert_eq!(p.active().userland_header, 0x9042_0000);
        assert_eq!(p.apps_flash(), (0x9060_0000, 0x907f_0000));
        assert_eq!(p.trampoline_address(), 0x9042_0038);
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn the_default_is_the_layout_of_the_surveyed_devices() {
        let n0110 = FirmwareProfile::default_for(Model::N0110);
        assert_eq!(n0110, FirmwareProfile::epsilon_24(Model::N0110));
        assert_eq!(
            (n0110.software_version.as_str(), n0110.commit_hash.as_str()),
            ("24.11.0", "a58aaa0")
        );
        assert_eq!(n0110.active_slot, Slot::B);
        // Relevés : RAM des applications et fin de la zone de flash (slot B).
        assert_eq!(n0110.apps_ram, (0x2001_2804, 0x2003_7000));
        assert_eq!(n0110.apps_flash(), (0x9057_0000, 0x907f_0000));
        // Pas de copie du bootloader : l'en-tête du userland est à 0x90410000 (slot B).
        assert_eq!(n0110.active().userland_header, 0x9041_0000);
        assert_eq!(n0110.slot(Slot::A).userland_header, 0x9001_0000);

        let n0115 = FirmwareProfile::epsilon_24(Model::N0115);
        assert_eq!(n0115.apps_flash(), (0x9017_0000, 0x903f_0000));
        assert_eq!(n0115.software_version, "24.3.0");

        let n0120 = FirmwareProfile::epsilon_24(Model::N0120);
        assert_eq!(n0120.apps_ram, (0x2401_2a40, 0x2403_7000));
        assert_eq!(n0120.apps_flash(), (0x9059_0000, 0x907f_0000));
        assert_eq!(n0120.active().userland_header, 0x9042_0000);
        assert_eq!(n0120.software_version, "23.2.6");
        for model in Model::ALL {
            let p = FirmwareProfile::epsilon_24(model);
            assert!(matches!(p.provenance, Provenance::Derived { .. }));
            assert_eq!(p.validate(), Ok(()), "{}", model.name());
        }
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
