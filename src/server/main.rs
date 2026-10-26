//! `nwasabi` — serveur web pour essayer l'émulateur (feature cargo `serve`).
//!
//! Il sert la page de démonstration et la version WebAssembly de nwasabi : l'émulation elle-même
//! s'exécute dans le navigateur, pas dans ce binaire. Avec `--nwa`, la page est servie déjà chargée
//! avec l'application : elle la récupère sur `/__nwasabi/preload`. Avec `--nmk`, le mapping des touches
//! (vérifié au démarrage) est sur `/__nwasabi/nmk`.
//!
//! Le site (construit par `build.rs`) est embarqué dans le binaire par `rust-embed` : le serveur ne lit
//! jamais le disque pour répondre, il ne sert que ce qui est embarqué et l'application donnée au
//! démarrage.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use clap::{Parser, ValueEnum};
use rust_embed::RustEmbed;
use serde_json::{Value, json};

/// Le site, construit par `build.rs` (dossier donné par `NWASABI_WEB_DIST`).
#[derive(RustEmbed)]
#[folder = "$NWASABI_WEB_DIST"]
struct Site;

/// Modèle de calculatrice.
#[derive(Clone, Copy, ValueEnum)]
enum ModelOpt {
    N0110,
    N0115,
    N0120,
}

/// Disposition du firmware émulé.
#[derive(Clone, Copy, ValueEnum)]
enum FirmwareOpt {
    /// Appareils relevés (Epsilon 23–24), celle que supposent les applications eadkp
    #[value(name = "24")]
    V24,
    /// Epsilon 26.4.0 officiel
    #[value(name = "26.4.0")]
    V26,
}

/// nwasabi : serveur web de démonstration de l'émulateur d'applications NumWorks (.nwa).
#[derive(Parser)]
#[command(name = "nwasabi", version)]
struct Options {
    /// Application à précharger dans la page
    #[arg(long, value_name = "FICHIER.nwa")]
    nwa: Option<PathBuf>,
    /// Mapping des touches à charger dans la page (fichier .nmk)
    #[arg(long, value_name = "FICHIER.nmk")]
    nmk: Option<PathBuf>,
    /// Fichiers de la calculatrice à charger dans la page (fichier .nfs)
    #[arg(long, value_name = "FICHIER.nfs")]
    nfs: Option<PathBuf>,
    /// État à reprendre dans la page (fichier .nds)
    #[arg(long, value_name = "FICHIER.nds")]
    nds: Option<PathBuf>,
    /// Modèle de calculatrice
    #[arg(long, value_enum, ignore_case = true, default_value = "n0110")]
    model: ModelOpt,
    /// Disposition du firmware
    #[arg(long, value_enum, default_value = "24")]
    firmware: FirmwareOpt,
    /// Données externes de l'application
    #[arg(long, value_name = "FICHIER", requires = "nwa")]
    data: Option<PathBuf>,
    /// Port d'écoute (0 : un port libre)
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// Adresse d'écoute (par défaut : cette machine seulement)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// Ouvre la page dans le navigateur
    #[arg(long)]
    open: bool,
}

/// L'application donnée avec `--nwa`, servie à la page.
struct Preload {
    name: String,
    model: String,
    firmware: String,
    nwa: Vec<u8>,
    data: Option<Vec<u8>>,
}

/// Ce que le serveur donne à la page en plus du site.
struct Served {
    preload: Option<Preload>,
    /// Le fichier `--nmk`, déjà vérifié.
    nmk: Option<String>,
    /// Le fichier `--nfs`, déjà vérifié.
    nfs: Option<String>,
    /// Le fichier `--nds`, déjà vérifié.
    nds: Option<String>,
}

type Shared = Arc<Served>;

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn preload(State(served): State<Shared>) -> Json<Value> {
    use nwasabi::json::base64_encode;
    Json(match served.preload.as_ref() {
        Some(p) => json!({
            "name": p.name,
            "model": p.model,
            "firmware": p.firmware,
            "nwa_base64": base64_encode(&p.nwa),
            "data_base64": p.data.as_deref().map(base64_encode),
        }),
        None => Value::Null,
    })
}

/// Un fichier donné au démarrage (`--nmk`, `--nfs`, `--nds`), tel quel (404 sans lui).
fn given(text: &Option<String>) -> Response {
    match text {
        Some(text) => ([(header::CONTENT_TYPE, "application/json")], text.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn nmk(State(served): State<Shared>) -> Response {
    given(&served.nmk)
}

async fn nfs(State(served): State<Shared>) -> Response {
    given(&served.nfs)
}

async fn nds(State(served): State<Shared>) -> Response {
    given(&served.nds)
}

/// Un fichier du site ; le type MIME (dont `application/wasm`, indispensable pour compiler un
/// module en continu) est déduit de l'extension par `rust-embed`.
async fn site(uri: Uri) -> Response {
    let path = match uri.path().trim_start_matches('/') {
        "" => "index.html",
        other => other,
    };
    match Site::get(path) {
        Some(file) => (
            [
                (header::CONTENT_TYPE, file.metadata.mimetype().to_owned()),
                (header::CACHE_CONTROL, "no-cache".to_owned()),
            ],
            file.data,
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            format!("introuvable : {}", uri.path()),
        )
            .into_response(),
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

async fn run(options: Options) -> Result<(), String> {
    let model = match options.model {
        ModelOpt::N0110 => "n0110",
        ModelOpt::N0115 => "n0115",
        ModelOpt::N0120 => "n0120",
    };
    let firmware = match options.firmware {
        FirmwareOpt::V24 => "24",
        FirmwareOpt::V26 => "26.4.0",
    };
    let nmk_text = match &options.nmk {
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
            nwasabi::keymap::KeyMap::parse(&text)
                .map_err(|e| format!("{} : {e}", path.display()))?;
            Some(text)
        }
        None => None,
    };
    let nfs_text = match &options.nfs {
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
            let document = nwasabi::json::parse(&text)
                .map_err(|e| format!("{} : JSON invalide : {e}", path.display()))?;
            nwasabi::formats::nfs::parse(&document)
                .map_err(|e| format!("{} : {e}", path.display()))?;
            Some(text)
        }
        None => None,
    };
    let nds_text = match &options.nds {
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
            let document = nwasabi::json::parse(&text)
                .map_err(|e| format!("{} : JSON invalide : {e}", path.display()))?;
            nwasabi::formats::nds::restore(&document, None)
                .map_err(|e| format!("{} : {e}", path.display()))?;
            Some(text)
        }
        None => None,
    };
    let preload_data = match &options.nwa {
        Some(path) => {
            let shown = path.display();
            let nwa = std::fs::read(path).map_err(|e| format!("{shown} : {e}"))?;
            let data = match &options.data {
                Some(p) => Some(std::fs::read(p).map_err(|e| format!("{} : {e}", p.display()))?),
                None => None,
            };
            let name = path
                .file_name()
                .map_or(shown.to_string(), |n| n.to_string_lossy().into_owned());
            Some(Preload {
                name,
                model: model.to_owned(),
                firmware: firmware.to_owned(),
                nwa,
                data,
            })
        }
        None => None,
    };

    let app = Router::new()
        .route("/__nwasabi/health", get(health))
        .route("/__nwasabi/preload", get(preload))
        .route("/__nwasabi/nmk", get(nmk))
        .route("/__nwasabi/nfs", get(nfs))
        .route("/__nwasabi/nds", get(nds))
        .fallback(get(site))
        .with_state(Arc::new(Served {
            preload: preload_data,
            nmk: nmk_text,
            nfs: nfs_text,
            nds: nds_text,
        }));

    let (host, port) = (options.host.as_str(), options.port);
    let listener = tokio::net::TcpListener::bind((host, port))
        .await
        .map_err(|e| format!("{host}:{port} : {e}"))?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    let url = format!("http://{address}/");
    println!("{url}");
    if options.open {
        open_browser(&url);
    }
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    if let Err(message) = run(Options::parse()).await {
        eprintln!("erreur : {message}");
        std::process::exit(1);
    }
}
