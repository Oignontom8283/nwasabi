//! Rejoue sur le processeur de nwasabi les programmes de test produits par
//! `tests/cpu/generate_vectors.py` (exécutés par QEMU) et compare les résultats octet pour octet.

use std::fs;
use std::path::PathBuf;

use nwasabi::cpu::{Cpu, Hooks, Stop};
use nwasabi::memory::{Memory, Perms, RegionKind, RegionSpec};

/// Adresse de la zone de résultats et taille de chaque cas (voir le générateur).
const OUT: u32 = 0x2001_0000;
const CASE_WORDS: usize = 29;
/// Cas des lots flottants : 12 registres, APSR, FPSCR, s0-s31, 16 mots de tampon.
const FP_CASE_WORDS: usize = 62;
const MARKER: u32 = 0x00c0_ffee;

struct NoHooks;
impl Hooks for NoHooks {}

fn region(name: &str, base: u32, size: u32) -> RegionSpec {
    RegionSpec {
        name: name.into(),
        base,
        size,
        perms: Perms::READ_WRITE_EXECUTE,
        kind: RegionKind::Ram,
        fill: 0,
    }
}

/// Dossier des vecteurs : `tests/golden/cpu`, ou celui que désigne `NWASABI_CPU_VECTORS` (pour essayer
/// d'autres vecteurs produits avec une autre graine du générateur).
fn vectors_dir() -> PathBuf {
    match std::env::var_os("NWASABI_CPU_VECTORS") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/cpu"),
    }
}

fn word(bytes: &[u8], index: usize) -> u32 {
    u32::from_le_bytes(bytes[4 * index..4 * index + 4].try_into().unwrap())
}

/// Nom de la i-ème valeur enregistrée dans un cas.
fn slot_name(i: usize, fp: bool) -> String {
    match (i, fp) {
        (0..=11, _) => format!("r{i}"),
        (12, _) => "apsr".to_owned(),
        (13, true) => "fpscr".to_owned(),
        (14..=45, true) => format!("s{}", i - 14),
        (n, true) => format!("tampon[{}]", n - 46),
        (n, false) => format!("tampon[{}]", n - 13),
    }
}

/// Texte de chaque cas, d'après le fichier `.cases`.
fn case_texts(path: &std::path::Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut cases: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') && line[1..].chars().all(|c| c.is_ascii_digit()) {
            cases.push(String::new());
        } else if let Some(last) = cases.last_mut() {
            last.push_str(line);
            last.push('\n');
        }
    }
    cases
}

