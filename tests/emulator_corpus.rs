//! Lance chaque application du corpus dans l'émulateur, sans aucune touche appuyée, et vérifie :
//! (1) qu'aucune ne s'arrête sur une faute d'émulation (instruction inconnue, appel système
//! inconnu, accès mémoire invalide) ; (2) que l'écran obtenu après un nombre fixe d'instructions
//! est toujours le même (références dans `tests/golden/screens.tsv`).
//!
//! Les applications qui réclament des données externes (ROM de jeu…) en reçoivent un jeu vide :
//! elles peuvent alors se terminer ou échouer, ce qui est attendu.

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use nwasabi::emulator::{Emulator, Fault, RunStop};
use nwasabi::profile::Model;

const GOLDEN: &str = "tests/golden/screens.tsv";
const BUDGET: u64 = 40_000_000;

struct Outcome {
    stop: RunStop,
    frames: u64,
    instructions: u64,
    screen_crc: u32,
    needed_data: bool,
}

/// Exécute l'application jusqu'à sa fin, une attente d'événement ou le budget d'instructions.
fn run_app(model: Model, nwa: &[u8]) -> Outcome {
    let mut emu = Emulator::new(model).expect("appareil");
    let needed_data = emu.install_nwa(nwa, None).is_err();
    if needed_data {
        let mut again = Emulator::new(model).expect("appareil");
        again.install_nwa(nwa, Some(&[])).expect("installation");
        emu = again;
    }
    let mut frames = 0;
    let stop = loop {
        let stop = emu.run(1_000_000);
        match stop {
            RunStop::Frame => frames += 1,
            RunStop::Sleep { .. } | RunStop::Budget => {}
            other => break other,
        }
        if emu.cpu.cycles >= BUDGET {
            break RunStop::Budget;
        }
    };
    let screen: Vec<u8> = emu
        .kernel
        .display
        .pixels()
        .iter()
        .flat_map(|p| p.to_le_bytes())
        .collect();
    if let Some(dir) = std::env::var_os("NWASABI_SCREENSHOTS") {
        let _ = std::fs::write(
            PathBuf::from(dir).join(format!("{:08x}.png", support::crc32(&screen))),
            support::png(320, 240, &emu.kernel.display.to_rgba()),
        );
    }
    Outcome {
        stop,
        frames,
        instructions: emu.cpu.cycles,
        screen_crc: support::crc32(&screen),
        needed_data,
    }
}

fn kind(stop: &RunStop) -> String {
    match stop {
        RunStop::Fault(f) => format!("faute({f:?})"),
        RunStop::Exited(code) => format!("fin({code})"),
        RunStop::WaitEvent { .. } => "attend-evenement".to_owned(),
        RunStop::Budget => "budget".to_owned(),
        other => format!("{other:?}"),
    }
}

#[test]
fn corpus_apps_run_and_draw_the_same_screen() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let mut results = BTreeMap::new();
    let mut failures = Vec::new();
    for (name, nwa) in &apps {
        let mut outcome = run_app(Model::N0110, nwa);
        // Une application faite pour le N0120 sonde la RAM à 0x24000000, absente du N0110.
        if matches!(outcome.stop, RunStop::Fault(Fault::Bus(f)) if f.address >> 24 == 0x24) {
            outcome = run_app(Model::N0120, nwa);
        }
        if matches!(outcome.stop, RunStop::Fault(_)) && !outcome.needed_data {
            failures.push(format!("{name}: {:?}", outcome.stop));
        }
        let line = format!(
            "{}\t{}\t{}\t{:08x}",
            kind(&outcome.stop),
            outcome.frames,
            outcome.instructions,
            outcome.screen_crc
        );
        eprintln!("{name:40} {line}");
        results.insert(name.trim_end_matches(".nwa").to_owned(), line);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    if std::env::var_os("NWASABI_UPDATE_GOLDEN").is_some() {
        let mut text = String::from("# nom\tarrêt\timages\tinstructions\tcrc32_écran\n");
        for (name, line) in &results {
            text += &format!("{name}\t{line}\n");
        }
        std::fs::write(&path, text).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{GOLDEN} illisible ({e}) ; le générer avec NWASABI_UPDATE_GOLDEN=1")
    });
    let expected: BTreeMap<&str, &str> = golden
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .filter_map(|l| l.split_once('\t'))
        .collect();
    for (name, line) in &results {
        match expected.get(name.as_str()) {
            Some(want) => assert_eq!(line, want, "{name}: l'exécution a changé"),
            None => eprintln!("{name} absent de {GOLDEN} : comparaison ignorée"),
        }
    }
}
