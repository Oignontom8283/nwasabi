//! Les informations de débogage de vraies applications (le corpus `local/nwa`, ignoré s'il est absent) se lisent sans
//! erreur, et leurs adresses se retrouvent dans la table des lignes.

mod support;

use nwasabi::emulator::Emulator;
use nwasabi::profile::Model;

#[test]
fn real_applications_debug_info_is_readable() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (mut with, mut total) = (0, 0);
    for (name, nwa) in apps {
        let mut emu = Emulator::new(Model::N0110).unwrap();
        if emu.install_nwa(&nwa, None).is_err() {
            continue;
        }
        total += 1;
        let started = std::time::Instant::now();
        let Some(info) = emu.debug_info() else {
            continue;
        };
        with += 1;
        let ms = started.elapsed().as_millis();
        eprintln!(
            "{name}: {} fonctions, {} lignes, {} types, {} globales, {ms} ms",
            info.functions.len(),
            info.rows().len(),
            info.type_count(),
            info.globals.len()
        );
        // Chaque fonction a une ligne à son adresse de début (sauf les fonctions écrites en assembleur).
        let mapped = info
            .functions
            .iter()
            .filter(|f| info.line_at(f.low).is_some())
            .count();
        assert!(
            mapped * 2 >= info.functions.len(),
            "{name} : {mapped}/{} fonctions ont une ligne",
            info.functions.len()
        );
        assert!(ms < 5_000, "{name} : lecture trop longue ({ms} ms)");
    }
    eprintln!("{with} applications sur {total} ont des informations de débogage");
}
