//! Les réglages modifiés sont réécrits là où l'appareil les garde : on les relit par l'inspection
//! de la mémoire (comme le ferait une application) ou par les appels système.

mod support;

use nwasabi::cpu::{Cpu, Hooks};
use nwasabi::device::FCC_ID_OFFSET;
use nwasabi::emulator::{Emulator, EmulatorError};
use nwasabi::inspect::read_structures;
use nwasabi::profile::{Model, Slot};

fn emulator() -> Emulator {
    Emulator::new(Model::N0110).unwrap()
}

/// Appelle un appel système du noyau avec `r0` pour argument et rend `r0`.
fn svc(emu: &mut Emulator, number: u8, r0: u32) -> u32 {
    let mut cpu = Cpu::new();
    cpu.regs.r[0] = r0;
    emu.kernel.on_svc(number, &mut cpu, &mut emu.device.memory);
    cpu.regs.r[0]
}

#[test]
fn versions_are_rewritten_in_the_headers() {
    let mut emu = emulator();
    emu.settings().set_version("27.1.0").unwrap();
    let s = read_structures(&mut emu.device.memory, false);
    assert_eq!(s.kernel_header.unwrap().value.software_version, "27.1.0");
    assert_eq!(
        s.userland_header.unwrap().value.expected_software_version,
        "27.1.0"
    );

    // Les deux versions peuvent différer.
    emu.settings()
        .set_expected_userland_version("26.4.0")
        .unwrap();
    emu.settings().set_commit_hash("abcdef0").unwrap();
    let s = read_structures(&mut emu.device.memory, false);
    assert_eq!(
        s.kernel_header.as_ref().unwrap().value.software_version,
        "27.1.0"
    );
    assert_eq!(s.kernel_header.unwrap().value.commit_hash, "abcdef0");
    assert_eq!(
        s.userland_header.unwrap().value.expected_software_version,
        "26.4.0"
    );
}

#[test]
fn device_name_can_be_set_and_cleared() {
    let mut emu = emulator();
    emu.settings()
        .set_device_name("Calculatrice de Tom")
        .unwrap();
    let s = read_structures(&mut emu.device.memory, false);
    assert_eq!(s.device_name.as_deref(), Some("Calculatrice de Tom"));

    emu.settings().set_device_name("").unwrap();
    let s = read_structures(&mut emu.device.memory, false);
    assert_eq!(s.device_name.as_deref(), Some(""));
}

#[test]
fn an_invalid_setting_changes_nothing() {
    let mut emu = emulator();
    let before = emu.device.profile.clone();
    let cursor = emu.device.memory.change_cursor();
    assert!(
        emu.settings()
            .set_software_version("26.4.0.1234567")
            .is_err()
    );
    assert!(emu.settings().set_serial_number("trop court").is_err());
    assert!(emu.settings().set_fcc_id("é").is_err());
    assert_eq!(emu.device.profile, before);
    assert!(emu.device.memory.changes_since(cursor).is_empty());
}

#[test]
fn identifiers_reach_the_app_through_system_calls() {
    let mut emu = emulator();
    emu.settings().set_fcc_id("TEST-FCC").unwrap();
    emu.settings().set_pcb_version(7).unwrap();
    emu.settings()
        .set_serial_number("ABCDEFGH12345678")
        .unwrap();

    let address = svc(&mut emu, 29, 0);
    assert_eq!(
        address,
        emu.device.profile.active().userland_header + FCC_ID_OFFSET
    );
    let text = emu.device.memory.read_bytes(address, 9).unwrap();
    assert_eq!(&text, b"TEST-FCC\0");
    assert_eq!(svc(&mut emu, 39, 0), 7);

    let buffer = 0x2002_0000;
    svc(&mut emu, 47, buffer);
    let serial = emu.device.memory.read_bytes(buffer, 17).unwrap();
    assert_eq!(&serial, b"ABCDEFGH12345678\0");
}

#[test]
fn battery_usb_and_flags_are_answered_live() {
    let mut emu = emulator();
    {
        let mut settings = emu.settings();
        settings.set_battery(2, 3.6, true);
        settings.set_usb_plugged(true);
        settings.set_clearance_level(2);
        settings.set_compilation_flags(0x8008);
        settings.set_last_reset_type(1);
        settings.set_brightness(99);
    }
    assert_eq!(svc(&mut emu, 4, 0), 2);
    assert_eq!(svc(&mut emu, 3, 0), 1);
    assert_eq!(svc(&mut emu, 52, 0), 1);
    assert_eq!(svc(&mut emu, 0, 0), 2);
    assert_eq!(svc(&mut emu, 56, 0), 0x8008);
    assert_eq!(svc(&mut emu, 59, 0), 1);
    assert_eq!(svc(&mut emu, 1, 0), 99);
    assert_eq!(emu.settings().status().battery_voltage, 3.6);
}

#[test]
fn switching_the_active_slot_moves_the_published_headers() {
    let mut emu = emulator();
    let slot_b = emu.device.profile.slot(Slot::B).userland_header;
    emu.settings().set_active_slot(Slot::B).unwrap();
    let s = read_structures(&mut emu.device.memory, false);
    assert_eq!(s.slot_info.unwrap().value.userland_header, slot_b);
    assert_eq!(s.userland_header.unwrap().address, slot_b);
}

#[test]
fn exam_mode_hides_the_apps_and_blocks_launching() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (_, nwa) = &apps[0];
    let mut emu = emulator();
    emu.install_nwa(nwa, Some(&[])).ok();
    assert_eq!(emu.device.apps().len(), 1);

    emu.settings().set_exam_mode(true);
    assert!(emu.settings().exam_mode());
    assert!(emu.device.apps().is_empty());
    assert!(matches!(emu.launch(), Err(EmulatorError::ExamMode)));

    emu.settings().set_exam_mode(false);
    emu.launch().unwrap();
}
