//! Le protocole de session : une conversation JSON avec un appareil émulé.
//!
//! Une requête est un objet `{"cmd": "...", ...}` (avec un `"id"` facultatif, renvoyé tel quel) ;
//! la réponse est un objet `{"ok": true, ...}` ou `{"ok": false, "error": "..."}`. C'est le même
//! contrat pour l'outil en ligne de commande (`nwasabi-cli session`), le serveur et la page web,
//! qui n'ont donc qu'à transporter ces objets.
//!
//! La session ne touche ni au disque ni à l'horloge : les fichiers entrent et sortent en base64
//! (ou en hexadécimal pour la mémoire), le temps est donné par `advance`.
//!
//! Commandes : `new`, `load`, `launch`, `run`, `key`, `advance`, `screen`, `stdout`, `cpu`,
//! `inspect`, `regions`, `usage`, `memory_read`, `memory_write`, `changes`, `watch`, `unwatch`,
//! `watch_hits`, `svc_stats`, `fs_list`, `fs_stats`, `fs_read`, `fs_write`, `fs_remove`,
//! `fs_rename`, `settings_get`, `settings_set`, `help`.

mod views;

use crate::emulator::{Emulator, RunStop};
use crate::inspect::read_structures;
use crate::json::{self, Value};
use crate::kernel::Key;
use crate::memory::WatchKind;
use crate::png;
use crate::profile::{Model, Slot};
use crate::storage::PythonSource;

/// Budget d'instructions par défaut d'une commande `run`.
const DEFAULT_INSTRUCTIONS: u64 = 10_000_000;
/// Instructions exécutées par pas pendant `run` (le temps de réaction aux arrêts).
const CHUNK: u64 = 1_000_000;
/// Taille maximale lue ou écrite d'un coup en mémoire.
const MAX_TRANSFER: u32 = 1 << 20;

type Reply = Result<Value, String>;

/// Un appareil émulé et ce qu'il faut pour répondre aux commandes.
#[derive(Default)]
pub struct Session {
    emu: Option<Emulator>,
    frames: u64,
}

