//! Serveur GDB : un débogueur (GDB, LLDB, VS Code…) se branche par TCP comme sur une sonde ST-Link et pilote un appareil émulé, par les
//! commandes JSON de la session : soit une [`Session`] du même processus, soit celle de la page web, par un [`Bridge`].
//!
//! Le protocole est le « GDB Remote Serial Protocol » : des paquets `$données#somme`. Le sous-ensemble géré suffit pour poser des
//! points d'arrêt et d'observation, avancer pas à pas, lire et écrire registres et mémoire, et reprendre ou interrompre l'exécution.
//!
//! Tout passe par les commandes JSON de la session (`run`, `step`, `break`, `watch`, `cpu`, `memory_read`…) : le débogueur voit
//! exactement ce que voit l'interface web, sans second chemin d'exécution à maintenir.

mod client;
mod paquets;
mod pont;

pub use client::serve_connection;
pub use pont::{Bridge, PageDebuggee};

use crate::session::Session;

/// Ce que pilote le serveur GDB : quelque chose qui répond à des commandes JSON de session, une ligne par commande.
pub trait Debuggee {
    /// Exécute une commande (une ligne JSON) et rend la réponse (une ligne JSON).
    fn exchange(&mut self, request: &str) -> String;
}

impl Debuggee for Session {
    fn exchange(&mut self, request: &str) -> String {
        self.handle_line(request)
    }
}
