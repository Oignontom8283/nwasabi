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

/// Exécute un lot et renvoie les différences (texte lisible), vide si tout concorde.
fn run_batch(name: &str) -> Vec<String> {
    let dir = vectors_dir();
    let program = fs::read(dir.join(format!("{name}.bin"))).unwrap();
    let expected = fs::read(dir.join(format!("{name}.expected"))).unwrap();
    let cases = case_texts(&dir.join(format!("{name}.cases")));
    let fp = name
        .split_once('_')
        .is_some_and(|(_, rest)| rest.starts_with("fp_"));
    let words = if fp { FP_CASE_WORDS } else { CASE_WORDS };
    let count = (expected.len() - 4) / (4 * words);

    let mut memory = Memory::new();
    memory.add_region(region("code", 0, 0x4_0000)).unwrap();
    memory
        .add_region(region("ram", 0x2000_0000, 0x4_0000))
        .unwrap();
    // Les programmes écrivent CPACR pour activer la FPU : on accepte ces écritures.
    memory
        .add_region(region("scs", 0xe000_e000, 0x1000))
        .unwrap();
    memory.write_bytes(0, &program).unwrap();

    let mut cpu = Cpu::new();
    cpu.regs.sp = memory.read_u32(0).unwrap();
    cpu.regs.pc = memory.read_u32(4).unwrap() & !1;
    let stop = cpu.run(&mut memory, &mut NoHooks, 20_000_000);
    let mut problems = Vec::new();
    if !matches!(stop, Stop::Breakpoint { .. }) {
        problems.push(format!(
            "{name}: arrêt inattendu {stop:?} à pc={:#x}",
            cpu.regs.pc
        ));
    }

    let actual = memory.read_bytes(OUT, expected.len() as u32).unwrap();
    for i in 0..count {
        let a = &actual[4 * words * i..4 * words * (i + 1)];
        let e = &expected[4 * words * i..4 * words * (i + 1)];
        if a != e {
            let mut text = format!(
                "{name} cas #{i} :\n{}",
                cases.get(i).map_or("", String::as_str)
            );
            for slot in 0..words {
                let (got, want) = (word(a, slot), word(e, slot));
                if got != want {
                    text += &format!(
                        "  {} : nwasabi {got:#010x}, QEMU {want:#010x}\n",
                        slot_name(slot, fp)
                    );
                }
            }
            problems.push(text);
        }
    }
    if word(&actual, count * words) != MARKER && problems.is_empty() {
        problems.push(format!("{name}: marqueur de fin absent"));
    }
    problems
}

fn check(name: &str) {
    let problems = run_batch(name);
    assert!(
        problems.is_empty(),
        "{} différence(s) avec QEMU (premières) :\n{}",
        problems.len(),
        problems
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Un test par lot de vecteurs.
macro_rules! batches {
    ($($test:ident => $file:literal,)*) => {
        $(#[test] fn $test() { check($file); })*
    };
}

batches! {
