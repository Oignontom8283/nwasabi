//! Les accès exclusifs (`ldrex`, `strex`) dans une vraie application : comme sur la calculatrice,
//! dont la SRAM est partageable et dont le bus n'a pas de moniteur d'exclusivité global, ils font une
//! BusFault dans la RAM. La flash, qui n'est pas partageable, les accepte.

mod support;

use nwasabi::cpu::FaultKind;
use nwasabi::emulator::{Emulator, Fault, RunStop};
use nwasabi::profile::Model;

/// Lance une application dont `main` est `code` (en demi-mots Thumb).
fn run(code: &[u16]) -> RunStop {
    let bytes: Vec<u8> = code.iter().flat_map(|hw| hw.to_le_bytes()).collect();
    let mut emu = Emulator::new(Model::N0110).unwrap();
    emu.install_nwa(&support::app_running(&bytes), None)
        .unwrap();
    emu.run(100_000)
}

#[test]
fn ldrex_and_strex_on_the_stack_fault_like_on_the_device() {
    // mov r1, sp ; ldrex r0, [r1] ; bx lr
    let RunStop::Fault(Fault::Bus(fault)) = run(&[0x4669, 0xe851, 0x0f00, 0x4770]) else {
        panic!("une BusFault était attendue");
    };
    assert_eq!(fault.kind, FaultKind::ExclusiveUnsupported);
    assert!(!fault.write);
    // mov r1, sp ; strex r2, r0, [r1] ; bx lr
    let RunStop::Fault(Fault::Bus(fault)) = run(&[0x4669, 0xe841, 0x0200, 0x4770]) else {
        panic!("une BusFault était attendue");
    };
    assert_eq!(fault.kind, FaultKind::ExclusiveUnsupported);
    assert!(fault.write);
}

#[test]
fn the_fault_is_reported_to_the_session_as_a_bus_fault() {
    use nwasabi::session::Session;
    let bytes: Vec<u8> = [0x4669u16, 0xe8d1, 0x0f4f, 0x4770]
        .iter()
        .flat_map(|hw| hw.to_le_bytes())
        .collect();
    let mut session = Session::new();
    session.handle_line(&format!(
        r#"{{"cmd":"load","nwa":"{}"}}"#,
        nwasabi::json::base64_encode(&support::app_running(&bytes))
    ));
    let reply = session.handle_line(r#"{"cmd":"run","instructions":100000}"#);
    assert!(reply.contains(r#""fault":"bus""#), "{reply}");
    assert!(reply.contains("exclusiveunsupported"), "{reply}");
}

#[test]
fn ldrex_in_flash_is_accepted() {
    // add r1, pc, #0 (adresse du code, en flash) ; ldrex r0, [r1] ; movs r0, #0 ; bx lr
    assert_eq!(
        run(&[0xa100, 0xe851, 0x0f00, 0x2000, 0x4770]),
        RunStop::Exited(0)
    );
}
