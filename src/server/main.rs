//! `nwasabi` — serveur web pour essayer l'émulateur (feature cargo `serve`).
//!
//! Il sert la page de démonstration et la version WebAssembly de nwasabi : l'émulation elle-même
//! s'exécute dans le navigateur, pas dans ce binaire. Avec `--nwa`, la page est servie déjà chargée
//! avec l'application : elle la récupère sur `/__nwasabi/preload`. Avec `--nmk`, le mapping des touches
//! (vérifié au démarrage) est sur `/__nwasabi/nmk`.
//!
//! Le site (construit par `build.rs`) est embarqué dans le binaire par `rust-embed`. Le seul disque lu
//! est celui des fichiers donnés au démarrage (`--nwa`, `--data`, `--nmk`, `--nfs`, `--flash`, `--nds`) : relus à
//! chaque requête, pour que la page puisse les recharger quand ils changent
//! (`/__nwasabi/version` donne leur empreinte). Aucun chemin de la requête n'est jamais lu.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
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
    /// Octets à placer en flash à côté de l'application (fichier .nfd : voir `nwasabi-cli flash pack`)
    #[arg(long, value_name = "FICHIER.nfd")]
    flash: Option<PathBuf>,
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
    /// Port d'un serveur GDB : un débogueur (GDB, LLDB, VS Code…) s'y branche pour déboguer l'émulation de la page, la même que
    /// celle qu'on voit et qu'on commande. Il faut aussi activer « Débogueur GDB » dans l'onglet Débogage de la page (désactivé
    /// par défaut). Sans cette option, rien n'écoute
    #[arg(long, value_name = "PORT")]
    gdb: Option<u16>,
    /// Comme `--gdb`, mais sans page : le débogueur pilote une émulation à part de l'application `--nwa`, qui repart de zéro à
    /// chaque connexion avec le fichier tel qu'il est sur le disque
    #[arg(long, value_name = "PORT", requires = "nwa", conflicts_with = "gdb")]
    gdb_headless: Option<u16>,
}

/// Une session neuve, avec l'application `--nwa` telle qu'elle est sur le disque maintenant.
fn gdb_session(served: &Served) -> Result<nwasabi::session::Session, String> {
    let nwa = served.nwa.as_ref().ok_or("--gdb-headless demande --nwa")?;
    let bytes = std::fs::read(nwa).map_err(|e| format!("{} : {e}", nwa.display()))?;
    let mut request = json!({
        "cmd": "load",
        "model": served.model,
        "firmware": served.firmware,
        "nwa": nwasabi::json::base64_encode(&bytes),
    });
    if let Some(path) = &served.data {
        let data = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
        request["data"] = json!(nwasabi::json::base64_encode(&data));
    }
    if let Some(path) = &served.flash {
        request["nfd"] = parse_json(path, &read_text(path)?)?;
    }
    let mut session = nwasabi::session::Session::new();
    let reply = session.handle_line(&request.to_string());
    if reply.contains("\"ok\":true") {
        Ok(session)
    } else {
        Err(format!("{} : {reply}", nwa.display()))
    }
}

/// Le serveur GDB de la page : un débogueur à la fois, sur l'appareil de la page (voir `nwasabi::gdb::Bridge`).
fn serve_gdb_page(bridge: &Arc<nwasabi::gdb::Bridge>, listener: std::net::TcpListener) {
    for stream in listener.incoming().flatten() {
        bridge.client_connected();
        let mut device = nwasabi::gdb::PageDebuggee(Arc::clone(bridge));
        if let Err(e) = nwasabi::gdb::serve_connection(&mut device, stream) {
            eprintln!("GDB : connexion interrompue : {e}");
        }
        bridge.client_gone();
    }
}

/// Le serveur GDB sans page : un débogueur à la fois, chacun sur une émulation neuve de l'application.
fn serve_gdb(served: Arc<Served>, listener: std::net::TcpListener) {
    for stream in listener.incoming().flatten() {
        match gdb_session(&served) {
            Ok(mut session) => {
                if let Err(e) = nwasabi::gdb::serve_connection(&mut session, stream) {
                    eprintln!("GDB : connexion interrompue : {e}");
                }
            }
            Err(message) => eprintln!("GDB : {message}"),
        }
    }
}

/// Les fichiers donnés au démarrage, relus à chaque requête.
struct Served {
    nwa: Option<PathBuf>,
    data: Option<PathBuf>,
    nmk: Option<PathBuf>,
    nfs: Option<PathBuf>,
    flash: Option<PathBuf>,
    nds: Option<PathBuf>,
    model: &'static str,
    firmware: &'static str,
    /// Le pont vers la page, quand `--gdb` est donné, et le port sur lequel les débogueurs se connectent.
    bridge: Option<(Arc<nwasabi::gdb::Bridge>, u16)>,
}

type Shared = Arc<Served>;

fn read_text(path: &std::path::Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))
}

fn parse_json(path: &std::path::Path, text: &str) -> Result<nwasabi::json::Value, String> {
    nwasabi::json::parse(text).map_err(|e| format!("{} : JSON invalide : {e}", path.display()))
}

/// Le fichier `--nmk`, vérifié.
fn load_nmk(path: &std::path::Path) -> Result<String, String> {
    let text = read_text(path)?;
    nwasabi::keymap::KeyMap::parse(&text).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(text)
}

/// Le fichier `--nfs`, vérifié.
fn load_nfs(path: &std::path::Path) -> Result<String, String> {
    let text = read_text(path)?;
    nwasabi::formats::nfs::parse(&parse_json(path, &text)?)
        .map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(text)
}

/// Le fichier `--flash`, vérifié.
fn load_nfd(path: &std::path::Path) -> Result<String, String> {
    let text = read_text(path)?;
    nwasabi::formats::nfd::parse(&parse_json(path, &text)?)
        .map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(text)
}

/// Le fichier `--nds`, vérifié.
fn load_nds(path: &std::path::Path) -> Result<String, String> {
    let text = read_text(path)?;
    nwasabi::formats::nds::restore(&parse_json(path, &text)?, None)
        .map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(text)
}

/// L'application `--nwa` (et ses données `--data`), telles qu'elles sont sur le disque maintenant.
fn load_preload(served: &Served) -> Result<Option<Value>, String> {
    use nwasabi::json::base64_encode;
    let Some(path) = &served.nwa else {
        return Ok(None);
    };
    let nwa = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let data = match &served.data {
        Some(p) => Some(std::fs::read(p).map_err(|e| format!("{} : {e}", p.display()))?),
        None => None,
    };
    let name = path.file_name().map_or(path.display().to_string(), |n| {
        n.to_string_lossy().into_owned()
    });
    Ok(Some(json!({
        "name": name,
        "model": served.model,
        "firmware": served.firmware,
        "nwa": base64_encode(&nwa),
        "data": data.as_deref().map(base64_encode),
    })))
}

/// Une erreur de lecture ou de vérification, en JSON (code 422) pour que la page puisse l'afficher.
fn failure(message: String) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "error": message })),
    )
        .into_response()
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

/// Ce que la page sait du serveur GDB : s'il y en a un, son port, si elle est branchée dessus et combien de débogueurs sont là.
async fn gdb_status(State(served): State<Shared>) -> Json<Value> {
    match &served.bridge {
        Some((bridge, port)) => Json(
            json!({ "enabled": true, "port": port, "attached": bridge.attached(), "clients": bridge.clients() }),
        ),
        None => Json(json!({ "enabled": false })),
    }
}

/// La page attend la prochaine commande d'un débogueur (au plus 20 secondes ; sans commande, 204).
async fn gdb_next(State(served): State<Shared>) -> Response {
    let Some((bridge, _)) = served.bridge.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let found = tokio::task::spawn_blocking(move || {
        bridge.next_request(std::time::Duration::from_secs(20))
    })
    .await
    .ok()
    .flatten();
    match found {
        Some((id, request)) => Json(json!({ "id": id, "request": request })).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

/// La page rend la réponse à une commande : `{ "id": …, "reply": "…" }`.
async fn gdb_answer(State(served): State<Shared>, Json(body): Json<Value>) -> StatusCode {
    let Some((bridge, _)) = &served.bridge else {
        return StatusCode::NOT_FOUND;
    };
    let (Some(id), Some(reply)) = (body["id"].as_u64(), body["reply"].as_str()) else {
        return StatusCode::BAD_REQUEST;
    };
    bridge.answer(id, reply.to_owned());
    StatusCode::NO_CONTENT
}

/// La page n'accepte plus de débogueur.
async fn gdb_detach(State(served): State<Shared>) -> StatusCode {
    if let Some((bridge, _)) = &served.bridge {
        bridge.detach();
    }
    StatusCode::NO_CONTENT
}

async fn preload(State(served): State<Shared>) -> Response {
    match load_preload(&served) {
        Ok(found) => Json(found.unwrap_or(Value::Null)).into_response(),
        Err(message) => failure(message),
    }
}

/// L'empreinte d'un fichier donné (taille et date de modification) : elle change quand il change.
fn stamp(path: &Option<PathBuf>) -> Value {
    let Some(path) = path else {
        return Value::Null;
    };
    let text = std::fs::metadata(path).map_or_else(
        |_| "absent".to_owned(),
        |meta| {
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            format!("{}-{modified}", meta.len())
        },
    );
    Value::String(text)
}

/// Les empreintes de tous les fichiers donnés : la page les compare pour savoir ce qui a changé.
async fn version(State(served): State<Shared>) -> Json<Value> {
    Json(json!({
        "nwa": stamp(&served.nwa),
        "data": stamp(&served.data),
        "nmk": stamp(&served.nmk),
        "nfs": stamp(&served.nfs),
        "flash": stamp(&served.flash),
        "nds": stamp(&served.nds),
    }))
}

/// Un fichier donné au démarrage (`--nmk`, `--nfs`, `--nds`), tel quel (404 sans lui).
fn given(path: &Option<PathBuf>, load: fn(&std::path::Path) -> Result<String, String>) -> Response {
    match path {
        Some(path) => match load(path) {
            Ok(text) => ([(header::CONTENT_TYPE, "application/json")], text).into_response(),
            Err(message) => failure(message),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn nmk(State(served): State<Shared>) -> Response {
    given(&served.nmk, load_nmk)
}

async fn nfs(State(served): State<Shared>) -> Response {
    given(&served.nfs, load_nfs)
}

async fn flash(State(served): State<Shared>) -> Response {
    given(&served.flash, load_nfd)
}

async fn nds(State(served): State<Shared>) -> Response {
    given(&served.nds, load_nds)
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
    let served = Served {
        nwa: options.nwa.clone(),
        data: options.data.clone(),
        nmk: options.nmk.clone(),
        nfs: options.nfs.clone(),
        flash: options.flash.clone(),
        nds: options.nds.clone(),
        model,
        firmware,
        bridge: None,
    };
    // Tout est vérifié une première fois : un fichier invalide arrête le serveur avant qu'il écoute.
    load_preload(&served)?;
    if let Some(path) = &served.nmk {
        load_nmk(path)?;
    }
    if let Some(path) = &served.nfs {
        load_nfs(path)?;
    }
    if let Some(path) = &served.flash {
        load_nfd(path)?;
    }
    if let Some(path) = &served.nds {
        load_nds(path)?;
    }

    let mut served = served;
    // `--gdb` : les débogueurs se branchent sur l'émulation de la page, par un pont (la page vient chercher leurs commandes).
    let mut bridge_listener = None;
    if let Some(gdb_port) = options.gdb {
        let listener = std::net::TcpListener::bind((options.host.as_str(), gdb_port))
            .map_err(|e| format!("GDB {}:{gdb_port} : {e}", options.host))?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        println!(
            "GDB : {address} (target remote {address}) ; à activer dans l'onglet Débogage de la page"
        );
        served.bridge = Some((nwasabi::gdb::Bridge::new(), address.port()));
        bridge_listener = Some(listener);
    }
    let served = Arc::new(served);
    if let (Some(listener), Some((bridge, _))) = (bridge_listener, served.bridge.clone()) {
        std::thread::spawn(move || serve_gdb_page(&bridge, listener));
    }
    // `--gdb-headless` : une émulation à part, sans page.
    if let Some(gdb_port) = options.gdb_headless {
        gdb_session(&served)?; // le fichier doit être valide avant d'écouter
        let listener = std::net::TcpListener::bind((options.host.as_str(), gdb_port))
            .map_err(|e| format!("GDB {}:{gdb_port} : {e}", options.host))?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        println!("GDB : {address} (target remote {address})");
        let shared = Arc::clone(&served);
        std::thread::spawn(move || serve_gdb(shared, listener));
    }

    let app = Router::new()
        .route("/__nwasabi/health", get(health))
        .route("/__nwasabi/gdb", get(gdb_status))
        .route("/__nwasabi/gdb/next", get(gdb_next))
        .route("/__nwasabi/gdb/answer", post(gdb_answer))
        .route("/__nwasabi/gdb/detach", post(gdb_detach))
        .route("/__nwasabi/preload", get(preload))
        .route("/__nwasabi/version", get(version))
        .route("/__nwasabi/nmk", get(nmk))
        .route("/__nwasabi/nfs", get(nfs))
        .route("/__nwasabi/flash", get(flash))
        .route("/__nwasabi/nds", get(nds))
        .fallback(get(site))
        .with_state(served);

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
