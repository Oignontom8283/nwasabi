//! Tests des appels système, appelés directement avec des registres choisis.

use super::*;
use crate::cpu::{Cpu, Hooks, SvcAction};
use crate::emulator::Emulator;
use crate::link::runtime::{SVC_DRAW_STRING, SVC_EXIT, SVC_SBRK, SVC_WRITE};
use crate::profile::Model;

/// Un appareil prêt, et un processeur dont la pile pointe dans la RAM des applications.
fn setup() -> (Emulator, Cpu) {
    let emu = Emulator::new(Model::N0110).unwrap();
    let mut cpu = Cpu::new();
    cpu.regs.sp = emu.device.profile.process_stack_start;
    (emu, cpu)
}

fn call(emu: &mut Emulator, cpu: &mut Cpu, number: u8, args: &[u32]) -> SvcAction {
    for (i, &a) in args.iter().enumerate() {
        cpu.regs.r[i] = a;
    }
    emu.kernel.on_svc(number, cpu, &mut emu.device.memory)
}

/// Adresse de RAM libre pour y placer des arguments.
const SCRATCH: u32 = 0x2002_0000;

#[test]
fn rectangle_fill_push_and_pull_go_through_memory() {
    let (mut emu, mut cpu) = setup();
    let rect = [10 | 20 << 16, 2 | 2 << 16];
    assert_eq!(
        call(&mut emu, &mut cpu, 20, &[rect[0], rect[1], 0xf800]),
        SvcAction::Handled
    );
    assert_eq!(emu.kernel.display.get(11, 21), Some(0xf800));

    for i in 0..4u32 {
        emu.device
            .memory
            .write_bytes(SCRATCH + 2 * i, &(0x100 + i as u16).to_le_bytes())
            .unwrap();
    }
    call(&mut emu, &mut cpu, 19, &[rect[0], rect[1], SCRATCH]);
    assert_eq!(emu.kernel.display.get(11, 21), Some(0x103));

    call(&mut emu, &mut cpu, 18, &[rect[0], rect[1], SCRATCH + 64]);
    let back = emu.device.memory.read_bytes(SCRATCH + 64, 8).unwrap();
    assert_eq!(back, [0, 1, 1, 1, 2, 1, 3, 1]);
}

#[test]
fn a_bad_pointer_stops_with_a_bus_fault() {
    let (mut emu, mut cpu) = setup();
    let action = call(
        &mut emu,
        &mut cpu,
        19,
        &[1 | 1 << 16, 1 | 1 << 16, 0x7000_0000],
    );
    assert_eq!(action, SvcAction::HandledAndStop);
    assert!(
        matches!(emu.kernel.take_stop(), Some(KernelStop::BusFault(f)) if f.address == 0x7000_0000)
    );
}

#[test]
fn vblank_aligns_time_on_the_next_frame_and_counts_it() {
    let (mut emu, mut cpu) = setup();
    assert_eq!(call(&mut emu, &mut cpu, 21, &[]), SvcAction::HandledAndStop);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Frame));
    assert_eq!(cpu.regs.r[0], 1);
    assert_eq!(emu.kernel.frames, 1);
    assert_eq!(emu.kernel.micros(cpu.cycles), FRAME_US);
    call(&mut emu, &mut cpu, 21, &[]);
    assert_eq!(emu.kernel.micros(cpu.cycles), 2 * FRAME_US);
}

#[test]
fn time_follows_cycles_and_sleeps() {
    let (mut emu, mut cpu) = setup();
    cpu.cycles = 216_000 * 5; // 5 ms à 216 cycles par microseconde
    call(&mut emu, &mut cpu, 48, &[]);
    assert_eq!((cpu.regs.r[0], cpu.regs.r[1]), (5, 0));
    call(&mut emu, &mut cpu, 49, &[100]);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Sleep { ms: 100 }));
    call(&mut emu, &mut cpu, 48, &[]);
    assert_eq!(cpu.regs.r[0], 105);
    call(&mut emu, &mut cpu, 50, &[2500]);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Sleep { ms: 3 }));
}

#[test]
fn keyboard_scan_returns_the_pressed_keys() {
    let (mut emu, mut cpu) = setup();
    emu.kernel.keyboard.press(Key::Ok);
    emu.kernel.keyboard.press(Key::Exe);
    call(&mut emu, &mut cpu, 34, &[]);
    assert_eq!(cpu.regs.r[0], 1 << 4);
    assert_eq!(cpu.regs.r[1], 1 << (52 - 32));
}

#[test]
fn event_get_returns_queued_events_and_the_remaining_timeout() {
    let (mut emu, mut cpu) = setup();
    emu.device.memory.write_u32(SCRATCH, 1000).unwrap();
    emu.kernel.keyboard.press(Key::Back);
    assert_eq!(call(&mut emu, &mut cpu, 23, &[SCRATCH]), SvcAction::Handled);
    assert_eq!(cpu.regs.r[0], 5);
    assert_eq!(emu.device.memory.read_u32(SCRATCH).unwrap(), 1000);
}

#[test]
fn event_get_waits_then_times_out() {
    let (mut emu, mut cpu) = setup();
    cpu.regs.pc = 0x9020_0102; // juste après un `svc` de 2 octets
    emu.device.memory.write_u32(SCRATCH, 50).unwrap();

    assert_eq!(
        call(&mut emu, &mut cpu, 23, &[SCRATCH]),
        SvcAction::HandledAndStop
    );
    assert_eq!(
        emu.kernel.take_stop(),
        Some(KernelStop::WaitEvent {
            remaining_ms: Some(50)
        })
    );
    assert_eq!(cpu.regs.pc, 0x9020_0100, "l'appel sera refait");

    emu.kernel.advance_time(20_000);
    call(&mut emu, &mut cpu, 23, &[SCRATCH]);
    assert_eq!(
        emu.kernel.take_stop(),
        Some(KernelStop::WaitEvent {
            remaining_ms: Some(30)
        })
    );

    emu.kernel.advance_time(40_000);
    assert_eq!(call(&mut emu, &mut cpu, 23, &[SCRATCH]), SvcAction::Handled);
    assert_eq!(cpu.regs.r[0], u32::from(keyboard::EVENT_NONE));
    assert_eq!(emu.device.memory.read_u32(SCRATCH).unwrap(), 0);
}

#[test]
fn event_get_wakes_up_when_a_key_arrives() {
    let (mut emu, mut cpu) = setup();
    cpu.regs.pc = 0x9020_0102;
    emu.device.memory.write_u32(SCRATCH, u32::MAX).unwrap(); // -1 : sans limite
    call(&mut emu, &mut cpu, 23, &[SCRATCH]);
    assert_eq!(
        emu.kernel.take_stop(),
        Some(KernelStop::WaitEvent { remaining_ms: None })
    );
    emu.kernel.keyboard.press(Key::Up);
    assert_eq!(call(&mut emu, &mut cpu, 23, &[SCRATCH]), SvcAction::Handled);
    assert_eq!(cpu.regs.r[0], 1);
}

#[test]
fn random_is_reproducible_for_a_given_seed() {
    let (mut a, mut cpu) = setup();
    let (mut b, _) = setup();
    call(&mut a, &mut cpu, 45, &[]);
    let first = cpu.regs.r[0];
    call(&mut b, &mut cpu, 45, &[]);
    assert_eq!(cpu.regs.r[0], first);
    call(&mut a, &mut cpu, 45, &[]);
    assert_ne!(cpu.regs.r[0], first);
}

#[test]
fn device_information_comes_from_the_profile_and_the_status() {
    let (mut emu, mut cpu) = setup();
    call(&mut emu, &mut cpu, 39, &[]);
    assert_eq!(cpu.regs.r[0], emu.device.profile.pcb_version);

    call(&mut emu, &mut cpu, 29, &[]);
    let fcc = emu.device.memory.read_bytes(cpu.regs.r[0], 16).unwrap();
    let text: Vec<u8> = fcc.into_iter().take_while(|&b| b != 0).collect();
    assert_eq!(text, emu.device.profile.fcc_id.as_bytes());

    call(&mut emu, &mut cpu, 47, &[SCRATCH]);
    let serial = emu.device.memory.read_bytes(SCRATCH, 17).unwrap();
    assert_eq!(&serial[..16], emu.device.profile.serial_number.as_bytes());
    assert_eq!(serial[16], 0);

    call(&mut emu, &mut cpu, 2, &[100]);
    call(&mut emu, &mut cpu, 1, &[]);
    assert_eq!(cpu.regs.r[0], 100);

    emu.kernel.status.battery_voltage = 3.5;
    call(&mut emu, &mut cpu, 5, &[]);
    assert_eq!(f32::from_bits(cpu.regs.s[0]), 3.5);
}

