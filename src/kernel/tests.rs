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

