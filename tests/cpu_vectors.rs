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

