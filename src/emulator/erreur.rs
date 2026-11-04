//! Les erreurs de l'émulateur.

use std::fmt;

use crate::device::DeviceError;
use crate::link::LinkError;

/// Ce qui peut échouer en préparant l'application.
#[derive(Debug)]
pub enum EmulatorError {
    Link(LinkError),
    Device(DeviceError),
    /// Aucune application n'est installée.
    NoApp,
    /// Le mode examen cache les applications externes.
    ExamMode,
    /// Des octets à placer en flash ne peuvent pas l'être (voir [`Emulator::install_flash`]).
    Flash(String),
}

impl fmt::Display for EmulatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Link(e) => write!(f, "édition de liens : {e}"),
            Self::Device(e) => write!(f, "{e}"),
            Self::NoApp => write!(f, "aucune application installée"),
            Self::ExamMode => write!(f, "mode examen : les applications externes sont cachées"),
            Self::Flash(message) => write!(f, "flash : {message}"),
        }
    }
}

impl std::error::Error for EmulatorError {}

impl From<LinkError> for EmulatorError {
    fn from(e: LinkError) -> Self {
        Self::Link(e)
    }
}

impl From<DeviceError> for EmulatorError {
    fn from(e: DeviceError) -> Self {
        Self::Device(e)
    }
}
