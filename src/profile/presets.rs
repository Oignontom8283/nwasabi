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

