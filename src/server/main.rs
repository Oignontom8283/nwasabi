//! `nwasabi` — serveur web pour essayer l'émulateur (feature cargo `serve`).
//!
//! Il sert la page de démonstration et la version WebAssembly de nwasabi : l'émulation elle-même
//! s'exécute dans le navigateur, pas dans ce binaire. Avec `--nwa`, la page est servie déjà chargée
//! avec l'application : elle la récupère sur `/__nwasabi/preload`.
//!
//! Le serveur n'accède jamais au disque pour répondre : il ne sert que ce qui est embarqué dans le
//! binaire (voir `build.rs`) et l'application donnée au démarrage.

mod http;

use std::net::TcpListener;
use std::sync::Arc;

use http::{Response, content_type};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

const USAGE: &str = "\
nwasabi — serveur web de démonstration de l'émulateur d'applications NumWorks (.nwa)

USAGE :
  nwasabi [--nwa <app.nwa>] [--model n0110|n0115|n0120] [--data <fichier>]
          [--port <n>] [--host <adresse>] [--open]

  --nwa     application à précharger dans la page
  --model   modèle de calculatrice (défaut : n0110)
  --data    données externes de l'application
  --port    port d'écoute (défaut : 8080 ; 0 : un port libre)
  --host    adresse d'écoute (défaut : 127.0.0.1, accessible depuis cette machine seulement)
  --open    ouvre la page dans le navigateur";

struct Preload {
    name: String,
    model: String,
    nwa: Vec<u8>,
    data: Option<Vec<u8>>,
}

fn escape_json(s: &str) -> String {
    nwasabi::json::Value::from(s).to_string()
}

