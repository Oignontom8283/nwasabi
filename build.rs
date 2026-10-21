//! Prépare les fichiers du site web que le serveur `nwasabi` embarque dans son binaire.
//!
//! Si le dossier `web-dist/` existe (site déjà construit), chacun de ses fichiers est inclus dans
//! le binaire ; sinon la liste est vide et le serveur affiche une page de repli. Ce script ne
//! demande ni Node ni la cible WebAssembly : il ne fait que lister des fichiers.

use std::fs;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dist = root.join("web-dist");
    println!("cargo:rerun-if-changed=web-dist");
    println!("cargo:rerun-if-changed=build.rs");

    let mut files = Vec::new();
    collect(&dist, &mut files);
    files.sort();

    let mut code = String::from(
        "/// Fichiers du site : (chemin d'URL, contenu).\npub static ASSETS: &[(&str, &[u8])] = &[\n",
    );
    for file in &files {
        let relative = file.strip_prefix(&dist).expect("sous web-dist");
        let url = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
        code += &format!(
            "    ({url:?}, include_bytes!({:?})),\n",
            file.to_string_lossy()
        );
    }
    code += "];\n";

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("assets.rs");
    fs::write(out, code).expect("écriture de assets.rs");
}
