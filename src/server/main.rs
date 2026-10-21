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

fn preload_json(p: &Preload) -> String {
    use nwasabi::json::{Value, base64_encode};
    Value::object()
        .with("name", p.name.as_str())
        .with("model", p.model.as_str())
        .with("nwa_base64", base64_encode(&p.nwa))
        .with("data_base64", p.data.as_deref().map(base64_encode))
        .to_string()
}

/// Page affichée quand le site n'a pas été construit dans ce binaire.
fn fallback_page(preload: Option<&Preload>) -> String {
    let app = match preload {
        Some(p) => format!(
            "<p>Application préchargée : <b>{}</b> ({}). Ses octets sont disponibles sur \
             <a href=\"/__nwasabi/preload\">/__nwasabi/preload</a>.</p>",
            p.name.replace('<', "&lt;"),
            p.model
        ),
        None => "<p>Aucune application préchargée (option <code>--nwa</code>).</p>".to_owned(),
    };
    format!(
        "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>nwasabi</title>\
         <body style=\"font-family:sans-serif;max-width:40em;margin:2em auto;padding:0 1em\">\
         <h1>nwasabi</h1>\
         <p>Le serveur fonctionne, mais l'interface web n'est pas incluse dans ce binaire \
         (dossier <code>web-dist/</code> absent à la compilation).</p>{app}\
         <p>En attendant, l'outil <code>nwasabi-cli</code> permet de tout essayer en ligne de commande.</p>"
    )
}

fn route(path: &str, preload: Option<&Preload>) -> Response {
    match path {
        "/__nwasabi/health" => Response::ok("application/json", b"{\"ok\":true}".to_vec()),
        "/__nwasabi/preload" => match preload {
            Some(p) => Response::ok("application/json", preload_json(p).into_bytes()),
            None => Response::ok("application/json", b"null".to_vec()),
        },
        _ => {
            let wanted = if path == "/" { "/index.html" } else { path };
            if let Some((_, body)) = ASSETS.iter().find(|(url, _)| *url == wanted) {
                return Response::ok(content_type(wanted), body.to_vec());
            }
            if path == "/" {
                return Response::ok(
                    "text/html; charset=utf-8",
                    fallback_page(preload).into_bytes(),
                );
            }
            Response::text(404, &format!("introuvable : {}", escape_json(path)))
        }
    }
}

fn open_browser(url: &str) {
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };
    if std::process::Command::new(program)
        .args(args)
        .spawn()
        .is_err()
    {
        eprintln!("impossible d'ouvrir le navigateur : ouvrir {url} à la main");
    }
}

