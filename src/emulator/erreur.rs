//! Les erreurs de l'émulateur.

use thiserror::Error;

use crate::device::DeviceError;
use crate::link::LinkError;

/// Ce qui peut échouer en préparant l'application.
#[derive(Debug, Error)]
pub enum EmulatorError {
    #[error("édition de liens : {0}")]
    Link(#[from] LinkError),
    #[error(transparent)]
    Device(#[from] DeviceError),
    /// Aucune application n'est installée.
    #[error("aucune application installée")]
    NoApp,
    /// Le mode examen cache les applications externes.
    #[error("mode examen : les applications externes sont cachées")]
    ExamMode,
    /// Des octets à placer en flash ne peuvent pas l'être (voir [`Emulator::install_flash`]).
    #[error("flash : {0}")]
    Flash(String),
}
