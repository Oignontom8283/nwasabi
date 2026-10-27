//! Vues « mémoire en direct » : utilisation de la pile et du tas, points d'observation, changements.

mod support;

use nwasabi::emulator::{Emulator, RunStop};
use nwasabi::memory::WatchKind;
use nwasabi::profile::Model;

fn start(prefix: &str) -> Option<Emulator> {
    let apps = support::corpus().or_else(|| {
        support::skip_without_corpus();
        None
    })?;
    let (_, nwa) = apps.iter().find(|(n, _)| n.starts_with(prefix))?;
    let mut emu = Emulator::new(Model::N0110).unwrap();
    emu.install_nwa(nwa, None).unwrap();
    Some(emu)
}

#[test]
fn stack_usage_grows_below_the_initial_stack_pointer() {
    let Some(mut emu) = start("mandelbrot") else {
        return;
    };
    assert_eq!(
        emu.usage().unwrap().stack_low_water,
        None,
        "rien n'a été écrit"
    );
    emu.run(2_000_000);
    let usage = emu.usage().unwrap();
    let low = usage.stack_low_water.expect("la pile a servi");
    assert!(
        low < usage.stack.end && low >= usage.stack.start,
        "{usage:?}"
    );
    assert!(usage.sp <= usage.stack.end);
}

#[test]
fn a_stopping_watchpoint_interrupts_the_app_on_the_access() {
    let Some(mut emu) = start("storage-1.1.1") else {
        return;
    };
    // L'application commence par vérifier le magic qui précède le tampon.
    let buffer = emu.device.profile.storage_address;
    let id = emu.device.memory.watch(buffer, 4, WatchKind::Read, true);
    let stop = loop {
        match emu.run(5_000_000) {
            RunStop::Frame | RunStop::Sleep { .. } | RunStop::Budget => {
                assert!(emu.cpu.cycles < 50_000_000, "aucun accès au magic");
            }
            other => break other,
        }
    };
    match stop {
        RunStop::Watchpoint(hit) => {
            assert_eq!(hit.id, id);
            assert!(!hit.write);
            assert_ne!(hit.pc, 0, "l'instruction qui a lu est connue");
            assert!(hit.address >= buffer && hit.address < buffer + 4);
        }
        other => panic!("arrêt inattendu : {other:?}"),
    }
    // L'application peut reprendre.
    emu.device.memory.unwatch(id);
    assert!(matches!(
        emu.run(100_000),
        RunStop::Budget | RunStop::Sleep { .. } | RunStop::Frame
    ));
}

#[test]
fn changes_since_follows_what_the_app_draws_and_stores() {
    let Some(mut emu) = start("storage-1.1.1") else {
        return;
    };
    let cursor = emu.device.memory.change_cursor();
    emu.run(2_000_000);
    let ranges = emu.device.memory.changes_since(cursor);
    assert!(!ranges.is_empty(), "l'application écrit en RAM");
    assert!(ranges.iter().all(|r| r.start >= 0x2000_0000));
}
