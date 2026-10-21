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

