//! Les réglages de l'appareil, modifiables à tout moment.
//!
//! Chaque réglage vit à l'endroit où l'appareil le garde, et c'est là qu'il est modifié : les
//! applications le voient comme sur un vrai appareil, soit en lisant la mémoire (versions, nom de
//! l'appareil), soit par un appel système (FCC, numéro de série, batterie…). Un réglage qui
//! réside en flash (versions, nom) est réécrit comme le ferait une mise à jour du firmware.

use crate::device::{DeviceError, DeviceImage};
use crate::kernel::DeviceStatus;
use crate::profile::{FirmwareProfile, Slot};

/// Accès aux réglages d'un appareil (obtenu par `Emulator::settings`).
pub struct Settings<'a> {
    pub(crate) device: &'a mut DeviceImage,
    pub(crate) status: &'a mut DeviceStatus,
}

impl Settings<'_> {
    /// Le profil du firmware (versions, adresses, noms…).
    pub fn profile(&self) -> &FirmwareProfile {
        &self.device.profile
    }

    /// Ce que l'appareil répond sur lui-même (batterie, USB, luminosité…).
    pub fn status(&self) -> &DeviceStatus {
        self.status
    }

    fn change(&mut self, change: impl FnOnce(&mut FirmwareProfile)) -> Result<(), DeviceError> {
        self.device.update_profile(change)?;
        // Ce que le noyau répond par appel système suit le profil.
        let profile = &self.device.profile;
        self.status.pcb_version = profile.pcb_version;
        self.status.serial_number = profile.serial_number.clone();
        self.status.fcc_id_address =
            profile.active().userland_header + crate::device::FCC_ID_OFFSET;
        Ok(())
    }

    /// Nom de l'appareil (vide : aucun), écrit dans les octets persistants en flash.
    pub fn set_device_name(&mut self, name: &str) -> Result<(), DeviceError> {
        self.change(|p| p.device_name = name.to_owned())
    }

    /// Identifiant FCC renvoyé par l'appel système `fcc_id` (31 caractères au plus).
    pub fn set_fcc_id(&mut self, fcc_id: &str) -> Result<(), DeviceError> {
        self.change(|p| p.fcc_id = fcc_id.to_owned())
    }

    pub fn set_pcb_version(&mut self, version: u32) -> Result<(), DeviceError> {
        self.change(|p| p.pcb_version = version)
    }

    /// Numéro de série : exactement 16 caractères ASCII.
    pub fn set_serial_number(&mut self, serial: &str) -> Result<(), DeviceError> {
        self.change(|p| p.serial_number = serial.to_owned())
    }

    /// Version d'Epsilon dans le `KernelHeader` (7 caractères au plus, ex. `26.4.0`). La version
    /// attendue du userland n'est pas touchée : voir [`Settings::set_expected_userland_version`].
    pub fn set_software_version(&mut self, version: &str) -> Result<(), DeviceError> {
        self.change(|p| p.software_version = version.to_owned())
    }

    /// Version du userland que le noyau attend (`UserlandHeader`).
    pub fn set_expected_userland_version(&mut self, version: &str) -> Result<(), DeviceError> {
        self.change(|p| p.expected_userland_version = version.to_owned())
    }

    /// Identifiant du commit de compilation (7 caractères au plus).
    pub fn set_commit_hash(&mut self, hash: &str) -> Result<(), DeviceError> {
        self.change(|p| p.commit_hash = hash.to_owned())
    }

    /// Fait des deux versions la même, comme sur un appareil à jour.
    pub fn set_version(&mut self, version: &str) -> Result<(), DeviceError> {
        self.change(|p| {
            p.software_version = version.to_owned();
            p.expected_userland_version = version.to_owned();
        })
    }

