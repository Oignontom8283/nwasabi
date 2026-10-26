//! Les applications de test écrites en C (`tests/apps/`) : de vrais fichiers `.nwa`, produits par un
//! compilateur, qui passent par toute la chaîne (lecture, édition de liens, exécution).
//!
//! Chacune se vérifie elle-même et rend 0 si tout est correct, sinon la ligne de la première vérification ratée.

use nwasabi::cpu::FaultKind;
use nwasabi::emulator::{Emulator, Fault, RunStop};
use nwasabi::profile::Model;

fn run(name: &str) -> RunStop {
    let path = format!("{}/tests/apps/bin/{name}.nwa", env!("CARGO_MANIFEST_DIR"));
    let nwa = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut emu = Emulator::new(Model::N0110).unwrap();
    emu.install_nwa(&nwa, None).unwrap();
    emu.run(1_000_000)
}

#[test]
fn integer_instructions_compiled_by_gcc_give_the_right_results() {
    assert_eq!(run("integer"), RunStop::Exited(0));
}

#[test]
fn ldrex_in_flash_is_accepted_in_a_compiled_app() {
    assert_eq!(run("exclusive_flash"), RunStop::Exited(0));
}

#[test]
fn ldrex_on_the_stack_of_a_compiled_app_is_a_bus_fault() {
    let RunStop::Fault(Fault::Bus(fault)) = run("exclusive_ram") else {
        panic!("une BusFault était attendue");
    };
    assert_eq!(fault.kind, FaultKind::ExclusiveUnsupported);
}

#[test]
fn single_precision_float_instructions_compiled_by_gcc_give_the_right_results() {
    assert_eq!(run("float"), RunStop::Exited(0));
}
