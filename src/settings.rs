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

