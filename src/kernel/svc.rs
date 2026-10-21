//! Les appels système (`svc #N`) : numéros du noyau d'Epsilon et appels privés de nwasabi.
//!
//! Convention d'appel : arguments dans `r0`–`r3` puis sur la pile, résultat dans `r0` (`r0:r1`
//! pour 64 bits, `s0` pour un flottant).

use super::display::Rect;
use super::{FRAME_US, Kernel, KernelStop, keyboard::EVENT_NONE};
use crate::cpu::{Bus, BusFault, Cpu, Hooks, SvcAction};
use crate::link::runtime::{SVC_DRAW_STRING, SVC_EXIT, SVC_SBRK, SVC_WRITE};

/// Longueur maximale d'une chaîne lue dans la mémoire de l'application.
const MAX_STRING: u32 = 4096;
/// Longueur maximale d'un `write`.
const MAX_WRITE: u32 = 1 << 16;
/// Taille du numéro de série copié, zéro final compris.
const SERIAL_BUFFER: usize = 17;

fn read_u16_block<B: Bus>(bus: &mut B, mut address: u32, count: u32) -> Result<Vec<u16>, BusFault> {
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        out.push(bus.read16(address)?);
        address = address.wrapping_add(2);
    }
    Ok(out)
}

fn read_bytes<B: Bus>(bus: &mut B, address: u32, count: u32) -> Result<Vec<u8>, BusFault> {
    (0..count)
        .map(|i| bus.read8(address.wrapping_add(i)))
        .collect()
}

fn read_c_string<B: Bus>(bus: &mut B, address: u32) -> Result<Vec<u8>, BusFault> {
    let mut out = Vec::new();
    for i in 0..MAX_STRING {
        match bus.read8(address.wrapping_add(i))? {
            0 => break,
            byte => out.push(byte),
        }
    }
    Ok(out)
}

fn set_u64(cpu: &mut Cpu, value: u64) {
    cpu.regs.r[0] = value as u32;
    cpu.regs.r[1] = (value >> 32) as u32;
}

impl Kernel {
    fn stop_with(&mut self, stop: KernelStop) -> SvcAction {
        self.stop = Some(stop);
        SvcAction::HandledAndStop
    }

    /// `eadk_event_get` : rend le plus ancien événement, ou attend (en refaisant l'appel à la
    /// reprise) jusqu'à l'expiration du délai.
    fn event_get<B: Bus>(&mut self, cpu: &mut Cpu, bus: &mut B) -> Result<SvcAction, BusFault> {
        let now = self.micros(cpu.cycles);
        let pointer = cpu.regs.r[0];
        let wait = match self.wait {
            Some(wait) => wait,
            None => {
                let timeout_ms = bus.read32(pointer)? as i32;
                let wait = super::EventWait {
                    started_us: now,
                    timeout_ms,
                };
                self.wait = Some(wait);
                wait
            }
        };
        let elapsed_ms = ((now - wait.started_us) / 1000) as i64;
        let remaining = i64::from(wait.timeout_ms) - elapsed_ms;
        let event = self.keyboard.pop_event();
        if event.is_some() || (wait.timeout_ms >= 0 && remaining <= 0) {
            self.wait = None;
            let left = if wait.timeout_ms < 0 {
                wait.timeout_ms
            } else {
                remaining.max(0) as i32
            };
            bus.write32(pointer, left as u32)?;
            cpu.regs.r[0] = u32::from(event.unwrap_or(EVENT_NONE));
            return Ok(SvcAction::Handled);
        }
        // Rien à rendre : on refera l'appel (l'instruction `svc` fait 2 octets).
        cpu.regs.pc = cpu.regs.pc.wrapping_sub(2);
        let remaining_ms = (wait.timeout_ms >= 0).then_some(remaining as u32);
        Ok(self.stop_with(KernelStop::WaitEvent { remaining_ms }))
    }

    fn handle<B: Bus>(
        &mut self,
        number: u8,
        cpu: &mut Cpu,
        bus: &mut B,
    ) -> Result<SvcAction, BusFault> {
        let [r0, r1, r2, r3] = [cpu.regs.r[0], cpu.regs.r[1], cpu.regs.r[2], cpu.regs.r[3]];
        if self.keyboard.exit_requested() {
            self.keyboard.clear_exit_request();
            return Ok(self.stop_with(KernelStop::HomePressed));
        }
        match number {
            0 => cpu.regs.r[0] = self.status.clearance_level,
            1 => cpu.regs.r[0] = u32::from(self.status.brightness),
            2 => self.status.brightness = r0 as u8,
            3 => cpu.regs.r[0] = u32::from(self.status.battery_charging),
            4 => cpu.regs.r[0] = u32::from(self.status.battery_level),
            5 => cpu.regs.s[0] = self.status.battery_voltage.to_bits(),
            18 => {
                let rect = Rect::from_registers(r0, r1);
                for (i, pixel) in self.display.pull(rect).into_iter().enumerate() {
                    bus.write16(r2.wrapping_add(2 * i as u32), pixel)?;
                }
            }
            19 => {
                let rect = Rect::from_registers(r0, r1);
                let pixels = read_u16_block(bus, r2, rect.area())?;
                self.display.push(rect, &pixels);
            }
            20 => self.display.fill(Rect::from_registers(r0, r1), r2 as u16),
            21 => {
                let now = self.micros(cpu.cycles);
                self.advance_time(FRAME_US - now % FRAME_US);
                self.frames += 1;
                cpu.regs.r[0] = 1;
                return Ok(self.stop_with(KernelStop::Frame));
            }
            23 => return self.event_get(cpu, bus),
            24 => cpu.regs.r[0] = u32::from(r0 < u32::from(EVENT_NONE)),
            29 => cpu.regs.r[0] = self.status.fcc_id_address,
            33 | 34 => set_u64(cpu, self.keyboard.state()),
            39 => cpu.regs.r[0] = self.status.pcb_version,
            45 => cpu.regs.r[0] = self.random_next(),
            46 => return Ok(self.stop_with(KernelStop::Reset)),
            47 => {
                let mut text = self.status.serial_number.clone().into_bytes();
                text.resize(SERIAL_BUFFER - 1, 0);
                text.push(0);
                for (i, byte) in text.into_iter().enumerate() {
                    bus.write8(r0.wrapping_add(i as u32), byte)?;
                }
            }
            48 => set_u64(cpu, self.micros(cpu.cycles) / 1000),
            49 => {
                self.advance_time(u64::from(r0) * 1000);
                return Ok(self.stop_with(KernelStop::Sleep { ms: r0 }));
            }
            50 => {
                self.advance_time(u64::from(r0));
                return Ok(self.stop_with(KernelStop::Sleep {
                    ms: r0.div_ceil(1000),
                }));
            }
            52 => cpu.regs.r[0] = u32::from(self.status.usb_plugged),
            56 => cpu.regs.r[0] = u32::from(self.status.compilation_flags),
            57 => cpu.regs.r[0] = self.status.bootloader_crc,
            59 => cpu.regs.r[0] = u32::from(self.status.last_reset_type),
            // Sans effet visible dans l'émulateur : mise à jour des niveaux d'autorisation,
            // circuit breaker, CRC, texte d'événement, état de shift/alpha, LED, mise en veille,
            // USB, appui long. Ils rendent 0.
            6..=17 | 22 | 25..=28 | 30..=32 | 35..=38 | 40..=44 | 51 | 53..=55 | 58 => {
                cpu.regs.r[0] = 0;
            }
            SVC_DRAW_STRING => {
                let text = read_c_string(bus, r0)?;
                let (x, y) = (r1 & 0xffff, r1 >> 16);
                let background = bus.read32(cpu.regs.sp)? as u16;
                self.display
                    .draw_text(&text, x, y, r2 & 0xff != 0, r3 as u16, background);
            }
            SVC_SBRK => {
                let wanted = i64::from(self.brk) + i64::from(r0 as i32);
                if wanted < i64::from(self.heap.0) || wanted > i64::from(self.heap.1) {
                    cpu.regs.r[0] = u32::MAX;
                } else {
                    cpu.regs.r[0] = self.brk;
                    self.brk = wanted as u32;
                }
            }
            SVC_WRITE => {
                let length = r2.min(MAX_WRITE);
                let bytes = read_bytes(bus, r1, length)?;
                self.write_stdout(&bytes);
                cpu.regs.r[0] = length;
            }
            SVC_EXIT => return Ok(self.stop_with(KernelStop::Exited(r0 as i32))),
            _ if self.config.strict_svc => {
                return Ok(self.stop_with(KernelStop::UnknownSvc(number)));
            }
            _ => {
                self.unknown_svcs += 1;
                cpu.regs.r[0] = 0;
            }
        }
        Ok(SvcAction::Handled)
    }
}

impl Hooks for Kernel {
    fn on_svc<B: Bus>(&mut self, imm8: u8, cpu: &mut Cpu, bus: &mut B) -> SvcAction {
        match self.handle(imm8, cpu, bus) {
            Ok(action) => action,
            Err(fault) => self.stop_with(KernelStop::BusFault(fault)),
        }
    }
}
