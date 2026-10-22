//! Construit le site web que le serveur `nwasabi` embarque (feature `serve` seulement).
//!
//! Trois étapes, toutes dans le dossier de compilation (`OUT_DIR`), jamais dans l'arbre source :
//! 1. s'assurer que la cible `wasm32-unknown-unknown` est installée (`rustup target add`) ;
//! 2. compiler le module WebAssembly de nwasabi avec un `cargo` imbriqué (dossier de cibles à part,
//!    sinon le verrou de cargo bloquerait) ;
//! 3. construire avec Node (`npm ci`, puis `@nwasabi/js` en TypeScript, puis l'interface Vue de `web/`)
//!    sur une copie des workspaces npm (`package.json`, `packages/`, `web/`) placée dans `OUT_DIR`.
//!
//! Les chemins sont décidés ici puis donnés aux outils : à Vite par `NWASABI_WASM` et
//! `NWASABI_WEB_OUT`, au reste de la compilation par `NWASABI_WEB_DIST` (lu par `rust-embed`).
//!
//! `NWASABI_NO_WEB=1` saute tout cela : le serveur ne contient alors qu'une page qui explique
//! comment construire l'interface (utile pour compiler sans Node).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Nom du programme npm. Sous Windows, Node installe `npm.cmd` (pas de `npm.exe`) : les interpréteurs de
/// commandes le trouvent grâce à `PATHEXT`, mais `Command` crée le processus directement et n'essaie
/// que `.exe`. `cfg!` désigne ici la machine qui exécute ce script (l'hôte), qui est aussi celle où
/// tourne npm, même quand on compile pour une autre cible.
const NPM: &str = if cfg!(windows) { "npm.cmd" } else { "npm" };

fn npm() -> Command {
    Command::new(NPM)
}

/// Lance une commande et rend une erreur lisible si elle manque ou échoue.
fn run(command: &mut Command, what: &str, hint: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|e| format!("impossible de lancer {what} ({e}). {hint}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what} a échoué ({status}). {hint}"))
    }
}

/// Copie le dossier `from` dans `to` en sautant `node_modules` et `dist` : le site se construit
/// dans le dossier de compilation, jamais dans l'arbre source (cargo l'interdit et `cargo install`
/// depuis le registre en dépend).
fn copy_site_sources(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{} : {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("{} : {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if name == "node_modules" || name == "dist" {
            continue;
        }
        let target = to.join(&name);
        if entry.path().is_dir() {
            copy_site_sources(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| format!("{} : {e}", target.display()))?;
        }
    }
    Ok(())
}

fn build_wasm(manifest: &Path, out: &Path) -> Result<PathBuf, String> {
    run(
        Command::new("rustup").args(["target", "add", WASM_TARGET]),
        "« rustup target add wasm32-unknown-unknown »",
        "Rustup est nécessaire pour installer la cible WebAssembly (https://rustup.rs).",
    )?;
    let target_dir = out.join("wasm-target");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(manifest)
        .args([
            "build",
            "--lib",
            "--profile",
            "wasm",
            "--no-default-features",
        ])
        .args(["--features", "wasm", "--target", WASM_TARGET])
        .arg("--target-dir")
        .arg(&target_dir);
    // Le cargo imbriqué ne doit pas hériter de ce que cargo a réglé pour ce script de compilation.
    for (key, _) in env::vars() {
        if key.starts_with("CARGO_FEATURE_")
            || key.starts_with("CARGO_CFG_")
            || key == "CARGO_ENCODED_RUSTFLAGS"
        {
            command.env_remove(key);
        }
    }
    run(
        &mut command,
        "la compilation du module WebAssembly",
        "Voir le message de cargo ci-dessus.",
    )?;
    Ok(target_dir
        .join(WASM_TARGET)
        .join("wasm")
        .join("nwasabi.wasm"))
}

fn build_site(manifest: &Path, out: &Path, wasm: &Path, dist: &Path) -> Result<(), String> {
    let hint = "Installer Node.js (https://nodejs.org) ou définir NWASABI_NO_WEB=1 pour compiler sans l'interface web.";
    // Copie de travail des workspaces npm (racine, paquets et site) : les paquets npm s'installent ici,
    // pas dans l'arbre source.
    let work = out.join("web-src");
    fs::create_dir_all(&work).map_err(|e| format!("{} : {e}", work.display()))?;
    for file in ["package.json", "package-lock.json"] {
        fs::copy(manifest.join(file), work.join(file)).map_err(|e| format!("{file} : {e}"))?;
    }
    // Le site n'utilise que @nwasabi/js : le paquet @nwasabi/wasm (publication) n'est pas nécessaire.
    for dir in ["packages/js", "web"] {
        copy_site_sources(&manifest.join(dir), &work.join(dir))?;
    }
    if !work.join("node_modules").exists() {
        run(
            npm()
                .args(["ci", "--no-audit", "--no-fund"])
                .current_dir(&work),
            "« npm ci »",
            hint,
        )?;
    }
    run(
        npm()
            .args(["run", "build", "-w", "@nwasabi/js"])
            .current_dir(&work),
        "« npm run build -w @nwasabi/js »",
        hint,
    )?;
    run(
        npm()
            .args(["run", "build", "-w", "nwasabi-web"])
            .current_dir(&work)
            .env("NWASABI_WASM", wasm)
            .env("NWASABI_WEB_OUT", dist),
        "« npm run build -w nwasabi-web »",
        hint,
    )
}

/// Page servie quand l'interface n'est pas construite (`NWASABI_NO_WEB=1`).
fn placeholder(dist: &Path) -> Result<(), String> {
    fs::create_dir_all(dist).map_err(|e| e.to_string())?;
    let page = "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>nwasabi</title>\
        <body style=\"font-family:sans-serif;max-width:40em;margin:2em auto;padding:0 1em\">\
        <h1>nwasabi</h1><p>Ce serveur a été compilé sans l'interface web (<code>NWASABI_NO_WEB=1</code>).</p>\
        <p>Pour l'inclure, installer Node.js puis recompiler sans cette variable. \
        L'outil <code>nwasabi-cli</code> permet de tout essayer en ligne de commande.</p>";
    fs::write(dist.join("index.html"), page).map_err(|e| e.to_string())
}

fn main() {
    println!("cargo:rerun-if-env-changed=NWASABI_NO_WEB");
    // Sans la feature `serve` (CLI, WebAssembly, tests du cœur) il n'y a rien à construire.
    if env::var_os("CARGO_FEATURE_SERVE").is_none() {
        println!(
            "cargo:rustc-env=NWASABI_WEB_DIST={}",
            env::var("OUT_DIR").unwrap_or_default()
        );
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let dist = out.join("web-dist");
    println!("cargo:rustc-env=NWASABI_WEB_DIST={}", dist.display());

    // Ce qui doit relancer la construction : le cœur (donc le wasm) et les sources du site.
    for path in [
        "src",
        "web/src",
        "web/index.html",
        "web/vite.config.js",
        "web/package.json",
        "packages/js/src",
        "packages/js/package.json",
        "packages/js/tsconfig.json",
        "package.json",
        "package-lock.json",
        "Cargo.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let result = if env::var_os("NWASABI_NO_WEB").is_some_and(|v| v != "0") {
        placeholder(&dist)
    } else {
        build_wasm(&manifest, &out).and_then(|wasm| build_site(&manifest, &out, &wasm, &dist))
    };
    if let Err(message) = result {
        eprintln!("\nerreur : {message}\n");
        std::process::exit(1);
    }
}
