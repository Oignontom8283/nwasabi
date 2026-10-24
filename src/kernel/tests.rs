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

#[test]
fn draw_string_reads_text_and_background_from_the_stack() {
    let (mut emu, mut cpu) = setup();
    emu.device.memory.write_bytes(SCRATCH, b"Hi\0").unwrap();
    emu.device.memory.write_u32(cpu.regs.sp, 0x1234).unwrap();
    call(
        &mut emu,
        &mut cpu,
        SVC_DRAW_STRING,
        &[SCRATCH, 5 | 6 << 16, 0, 0xffff],
    );
    assert_eq!(emu.kernel.display.get(5, 6), Some(0x1234));
    assert!((0..14).any(|y| (0..14).any(|x| emu.kernel.display.get(5 + x, 6 + y) == Some(0xffff))));
}

#[test]
fn sbrk_moves_the_program_break_inside_the_heap() {
    let (mut emu, mut cpu) = setup();
    emu.kernel.set_heap(0x2002_0000, 0x2002_1000);
    call(&mut emu, &mut cpu, SVC_SBRK, &[0x800]);
    assert_eq!(cpu.regs.r[0], 0x2002_0000);
    call(&mut emu, &mut cpu, SVC_SBRK, &[0]);
    assert_eq!(cpu.regs.r[0], 0x2002_0800);
    call(&mut emu, &mut cpu, SVC_SBRK, &[0x1000]);
    assert_eq!(cpu.regs.r[0], u32::MAX, "plus de place");
    call(&mut emu, &mut cpu, SVC_SBRK, &[(-0x800i32) as u32]);
    assert_eq!(emu.kernel.program_break(), 0x2002_0000);
}

#[test]
fn write_collects_stdout_and_shows_it_on_screen() {
    let (mut emu, mut cpu) = setup();
    emu.device.memory.write_bytes(SCRATCH, b"ok!").unwrap();
    call(&mut emu, &mut cpu, SVC_WRITE, &[1, SCRATCH, 3]);
    assert_eq!(cpu.regs.r[0], 3);
    assert_eq!(emu.kernel.stdout, b"ok!");
    assert_eq!(emu.kernel.display.get(0, 0), Some(0xffff), "fond blanc");
}

#[test]
fn exit_and_reset_stop_the_application() {
    let (mut emu, mut cpu) = setup();
    call(&mut emu, &mut cpu, SVC_EXIT, &[7]);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Exited(7)));
    call(&mut emu, &mut cpu, 46, &[]);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Reset));
}

#[test]
fn unknown_svc_stops_in_strict_mode_and_is_counted_otherwise() {
    let (mut emu, mut cpu) = setup();
    assert_eq!(
        call(&mut emu, &mut cpu, 0x70, &[]),
        SvcAction::HandledAndStop
    );
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::UnknownSvc(0x70)));

    emu.kernel.config.strict_svc = false;
    assert_eq!(call(&mut emu, &mut cpu, 0x70, &[]), SvcAction::Handled);
    assert_eq!(emu.kernel.unknown_svcs, 1);
    assert_eq!(emu.kernel.svc_counts[0x70], 2);
}

#[test]
fn home_key_ends_the_application_at_the_next_call() {
    let (mut emu, mut cpu) = setup();
    emu.kernel.keyboard.press(Key::Home);
    call(&mut emu, &mut cpu, 34, &[]);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::HomePressed));
}

// ---- horloge de l'hôte ---------------------------------------------------------------------------

/// Comme `setup`, avec l'horloge de l'hôte : le temps n'avance que par `advance_time`.
fn setup_host() -> (Emulator, Cpu) {
    let config = KernelConfig {
        clock: ClockMode::Host,
        ..KernelConfig::default()
    };
    let emu = Emulator::with_profile(
        crate::profile::FirmwareProfile::default_for(Model::N0110),
        config,
    )
    .unwrap();
    let mut cpu = Cpu::new();
    cpu.regs.sp = emu.device.profile.process_stack_start;
    (emu, cpu)
}

#[test]
fn the_host_clock_ignores_executed_instructions() {
    let (mut emu, mut cpu) = setup_host();
    cpu.cycles = 216_000_000; // une seconde à 216 MHz, qui ne doit pas compter
    call(&mut emu, &mut cpu, 48, &[]);
    assert_eq!((cpu.regs.r[0], cpu.regs.r[1]), (0, 0));
    emu.kernel.advance_time(1_500_000);
    call(&mut emu, &mut cpu, 48, &[]);
    assert_eq!(cpu.regs.r[0], 1500);
}

#[test]
fn the_host_clock_is_not_moved_by_a_vblank() {
    let (mut emu, mut cpu) = setup_host();
    call(&mut emu, &mut cpu, 21, &[]);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Frame));
    assert_eq!(emu.kernel.micros(cpu.cycles), 0);
}

#[test]
fn msleep_lasts_the_requested_host_time_and_never_ends_in_a_pause() {
    let (mut emu, mut cpu) = setup_host();
    cpu.regs.pc = 0x9020_0102; // juste après un `svc` de 2 octets
    // Premier appel : l'application dort 100 ms, l'appel sera refait.
    assert_eq!(
        call(&mut emu, &mut cpu, 49, &[100]),
        SvcAction::HandledAndStop
    );
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Sleep { ms: 100 }));
    assert_eq!(cpu.regs.pc, 0x9020_0100);
    assert_eq!(
        cpu.regs.r[0], 100,
        "l'argument est conservé pour la reprise"
    );

    // La simulation est mise en pause : l'hôte ne fait pas passer de temps, même pendant très longtemps
    // (et même si le processeur exécutait des instructions entre-temps).
    for _ in 0..5 {
        cpu.cycles += 10_000_000;
        cpu.regs.pc = 0x9020_0102;
        assert_eq!(
            call(&mut emu, &mut cpu, 49, &[100]),
            SvcAction::HandledAndStop
        );
        assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Sleep { ms: 100 }));
    }

    emu.kernel.advance_time(40_000);
    cpu.regs.pc = 0x9020_0102;
    call(&mut emu, &mut cpu, 49, &[100]);
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Sleep { ms: 60 }));

    emu.kernel.advance_time(60_000);
    cpu.regs.pc = 0x9020_0102;
    assert_eq!(call(&mut emu, &mut cpu, 49, &[100]), SvcAction::Handled);
    assert_eq!(emu.kernel.take_stop(), None);

    // L'attente suivante repart de zéro.
    cpu.regs.pc = 0x9020_0102;
    assert_eq!(
        call(&mut emu, &mut cpu, 50, &[2000]),
        SvcAction::HandledAndStop
    );
    assert_eq!(emu.kernel.take_stop(), Some(KernelStop::Sleep { ms: 2 }));
}

#[test]
fn event_timeouts_follow_the_host_clock_only() {
    let (mut emu, mut cpu) = setup_host();
    cpu.regs.pc = 0x9020_0102;
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
    // Des millions d'instructions sans temps hôte : toujours 50 ms à attendre.
    cpu.cycles += 500_000_000;
    cpu.regs.pc = 0x9020_0102;
    call(&mut emu, &mut cpu, 23, &[SCRATCH]);
    assert_eq!(
        emu.kernel.take_stop(),
        Some(KernelStop::WaitEvent {
            remaining_ms: Some(50)
        })
    );
    emu.kernel.advance_time(50_000);
    assert_eq!(call(&mut emu, &mut cpu, 23, &[SCRATCH]), SvcAction::Handled);
}

#[test]
fn relaunching_forgets_pending_waits() {
    let (mut emu, mut cpu) = setup_host();
    cpu.regs.pc = 0x9020_0102;
    call(&mut emu, &mut cpu, 49, &[1000]);
    emu.kernel.reset_waits();
    assert_eq!(emu.kernel.take_stop(), None);
    // Une nouvelle attente de 10 ms n'hérite pas de l'ancienne échéance (1000 ms).
    cpu.regs.pc = 0x9020_0102;
    call(&mut emu, &mut cpu, 49, &[10]);
    emu.kernel.advance_time(10_000);
    cpu.regs.pc = 0x9020_0102;
    assert_eq!(call(&mut emu, &mut cpu, 49, &[10]), SvcAction::Handled);
}
