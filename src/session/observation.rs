//! Regarder l'appareil : processeur, mémoire, zones, points d'observation, appels système, symboles, désassemblage.

use super::{MAX_TRANSFER, Reply, Session, debug, number, text, views};
use crate::elf::SymbolKind;
use crate::inspect::read_structures;
use crate::json;
use crate::json::Value;
use crate::json::ValueExt;
use crate::memory::WatchKind;

impl Session {
    /// La commande `cpu`.
    pub(super) fn cpu(&mut self) -> Reply {
        let emu = self.emu()?;
        let r = &emu.cpu.regs;
        Ok(Value::object()
            .with("r", r.r.iter().map(|&v| Value::from(v)).collect::<Vec<_>>())
            .with("sp", r.sp)
            .with("lr", r.lr)
            .with("pc", r.pc)
            .with("xpsr", r.xpsr)
            .with("fpscr", r.fpscr)
            .with("s", r.s.iter().map(|&v| Value::from(v)).collect::<Vec<_>>())
            .with("instructions", emu.cpu.cycles))
    }

    /// La commande `inspect`.
    pub(super) fn inspect(&mut self) -> Reply {
        let emu = self.emu()?;
        let exam = emu.device.exam_mode;
        let structures = read_structures(&mut emu.device.memory, exam);
        let mut out = Value::object().with("structures", views::structures(&structures));
        if let Some(app) = emu.app() {
            out.set("app", views::app(app));
        }
        Ok(out)
    }

    /// La commande `regions`.
    pub(super) fn regions(&mut self) -> Reply {
        let emu = self.emu()?;
        Ok(Value::object().with("regions", views::regions(&emu.device.memory.regions())))
    }

    /// La commande `usage`.
    pub(super) fn usage(&mut self) -> Reply {
        let emu = self.emu()?;
        let usage = emu.usage().ok_or("aucune application installée")?;
        Ok(views::usage(&usage))
    }

    /// La commande `memory_read`.
    pub(super) fn memory_read(&mut self, req: &Value) -> Reply {
        let (address, length) = (number(req, "address")?, number(req, "length")?);
        if length > MAX_TRANSFER {
            return Err(format!("lecture limitée à {MAX_TRANSFER} octets"));
        }
        let bytes = self
            .emu()?
            .device
            .memory
            .read_bytes(address, length)
            .map_err(|e| e.to_string())?;
        Ok(Value::object().with("hex", json::hex_encode(&bytes)))
    }

    /// La commande `memory_write`.
    pub(super) fn memory_write(&mut self, req: &Value) -> Reply {
        let address = number(req, "address")?;
        let bytes = json::hex_decode(text(req, "hex")?).ok_or("hexadécimal invalide")?;
        if bytes.len() as u32 > MAX_TRANSFER {
            return Err(format!("écriture limitée à {MAX_TRANSFER} octets"));
        }
        self.emu()?
            .device
            .memory
            .write_bytes(address, &bytes)
            .map_err(|e| e.to_string())?;
        Ok(Value::object().with("written", bytes.len()))
    }

    /// La commande `changes`.
    pub(super) fn changes(&mut self, req: &Value) -> Reply {
        let memory = &self.emu()?.device.memory;
        let since = req
            .get("cursor")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0) as u64;
        let ranges = memory
            .changes_since(since)
            .iter()
            .map(|r| Value::object().with("start", r.start).with("len", r.len))
            .collect::<Vec<_>>();
        Ok(Value::object()
            .with("ranges", ranges)
            .with("cursor", memory.change_cursor()))
    }

    /// La commande `watch`.
    pub(super) fn watch(&mut self, req: &Value) -> Reply {
        let (start, len) = (number(req, "address")?, number(req, "length")?);
        let kind = match req.get("kind").and_then(Value::as_str).unwrap_or("write") {
            "read" => WatchKind::Read,
            "write" => WatchKind::Write,
            "readwrite" => WatchKind::ReadWrite,
            other => {
                return Err(format!("sorte inconnue : {other} (read, write, readwrite)"));
            }
        };
        let stop = req.get("stop").and_then(Value::as_bool).unwrap_or(false);
        let id = self.emu()?.device.memory.watch(start, len, kind, stop);
        Ok(Value::object().with("id", id.0))
    }

    /// La commande `unwatch`.
    pub(super) fn unwatch(&mut self, req: &Value) -> Reply {
        let id = crate::memory::WatchId(number(req, "id")?);
        let removed = self.emu()?.device.memory.unwatch(id);
        Ok(Value::object().with("removed", removed))
    }

    /// La commande `watch_list`.
    pub(super) fn watch_list(&mut self) -> Reply {
        let list = self.emu()?.device.memory.watch_list();
        Ok(Value::object().with(
            "watches",
            list.iter()
                .map(|(id, start, len, kind, stop)| {
                    Value::object()
                        .with("id", id.0)
                        .with("address", *start)
                        .with("length", *len)
                        .with(
                            "kind",
                            match kind {
                                WatchKind::Read => "read",
                                WatchKind::Write => "write",
                                WatchKind::ReadWrite => "readwrite",
                            },
                        )
                        .with("stop", *stop)
                })
                .collect::<Vec<_>>(),
        ))
    }

    /// La commande `watch_hits`.
    pub(super) fn watch_hits(&mut self) -> Reply {
        let hits = self.emu()?.device.memory.take_watch_hits();
        Ok(Value::object().with(
            "hits",
            hits.iter()
                .map(|h| {
                    Value::object()
                        .with("id", h.id.0)
                        .with("address", h.address)
                        .with("width", u32::from(h.width))
                        .with("write", h.write)
                        .with("value", h.value)
                        .with("pc", h.pc)
                })
                .collect::<Vec<_>>(),
        ))
    }

    /// La commande `svc_log`.
    pub(super) fn svc_log(&mut self, req: &Value) -> Reply {
        let action = req.get("action").and_then(Value::as_str).unwrap_or("get");
        let since = req
            .get("since")
            .and_then(Value::as_u32)
            .map_or(0, u64::from);
        let kernel = &mut self.emu()?.kernel;
        match action {
            "start" => kernel.svc_log = Some(crate::kernel::SvcLog::default()),
            "stop" => kernel.svc_log = None,
            "clear" => {
                if let Some(log) = kernel.svc_log.as_mut() {
                    log.calls.clear();
                }
            }
            "get" => {}
            other => {
                return Err(format!(
                    "action inconnue : {other} (start, stop, clear, get)"
                ));
            }
        }
        let (running, total, calls) = match &kernel.svc_log {
            Some(log) => (
                true,
                log.total,
                log.calls
                    .iter()
                    .filter(|c| c.seq >= since)
                    .map(|c| {
                        Value::object()
                            .with("seq", c.seq)
                            .with("svc", u32::from(c.number))
                            .with("pc", c.pc)
                            .with(
                                "args",
                                c.args.iter().map(|&a| Value::from(a)).collect::<Vec<_>>(),
                            )
                            .with("ret", c.ret)
                            .with("frame", c.frame)
                    })
                    .collect::<Vec<_>>(),
            ),
            None => (false, 0, Vec::new()),
        };
        Ok(Value::object()
            .with("running", running)
            .with("total", total)
            .with("calls", calls))
    }

    /// La commande `svc_stats`.
    pub(super) fn svc_stats(&mut self) -> Reply {
        let emu = self.emu()?;
        let counts = emu
            .kernel
            .svc_counts
            .iter()
            .enumerate()
            .filter(|(_, c)| **c > 0)
            .map(|(n, c)| Value::object().with("svc", n).with("calls", *c))
            .collect::<Vec<_>>();
        Ok(Value::object()
            .with("counts", counts)
            .with("frames", emu.kernel.frames))
    }

    /// La commande `symbols`.
    pub(super) fn symbols(&mut self, req: &Value) -> Reply {
        let query = req
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        let emu = self.emu()?;
        let found = emu
            .app()
            .map(|a| a.symbols.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|s| {
                s.kind == SymbolKind::Func
                    && (s.name.to_lowercase().contains(&query)
                        || crate::symbols::demangle(&s.name)
                            .to_lowercase()
                            .contains(&query))
            })
            .take(500)
            .map(|s| {
                let demangled = crate::symbols::demangle(&s.name);
                let mut v = Value::object()
                    .with("name", s.name.as_str())
                    .with("address", s.address & !1)
                    .with("size", s.size);
                if demangled != s.name {
                    v.set("demangled", demangled);
                    v.set("qualified", crate::symbols::qualified(&s.name));
                }
                v
            })
            .collect::<Vec<_>>();
        Ok(Value::object().with("symbols", found))
    }

    /// La commande `symbol`.
    pub(super) fn symbol(&mut self, req: &Value) -> Reply {
        let address = number(req, "address")?;
        let emu = self.emu()?;
        match emu.symbol_at(address) {
            Some((name, start, offset)) => {
                let mut reply = Value::object()
                    .with("name", name)
                    .with("start", start)
                    .with("offset", offset);
                let demangled = crate::symbols::demangle(name);
                if demangled != name {
                    reply.set("demangled", demangled);
                }
                Ok(reply)
            }
            None => Ok(Value::object()),
        }
    }

    /// La commande `disasm`.
    pub(super) fn disasm(&mut self, req: &Value) -> Reply {
        let address = number(req, "address")? & !1;
        let count = req
            .get("count")
            .and_then(Value::as_u32)
            .unwrap_or(16)
            .clamp(1, 256) as usize;
        let before = req
            .get("before")
            .and_then(Value::as_u32)
            .unwrap_or(0)
            .min(32) as usize;
        let memory = &mut self.emu()?.device.memory;
        let mut read = |a: u32| {
            let bytes = memory.read_bytes(a, 2).ok()?;
            Some(u16::from_le_bytes([bytes[0], bytes[1]]))
        };
        // Les instructions Thumb ont 2 ou 4 octets : pour montrer ce qui précède, on part de plus loin et l'on
        // garde le plus long départ dont la lecture tombe exactement sur `address`.
        let mut lines = Vec::new();
        let mut index = 0;
        for back in (0..=2 * before).rev() {
            let start = address.wrapping_sub(2 * back as u32);
            let attempt = crate::cpu::disassemble(&mut read, start, back + count);
            if let Some(at) = attempt.iter().position(|l| l.address == address) {
                index = at;
                lines = attempt;
                break;
            }
        }
        let from = index.saturating_sub(before);
        let emu = &*self.emu()?;
        let rows = lines
            .iter()
            .skip(from)
            .take(before.min(index) + count)
            .map(|l| {
                let hex = if l.size == 4 {
                    format!("{:04x} {:04x}", l.halfwords[0], l.halfwords[1])
                } else {
                    format!("{:04x}", l.halfwords[0])
                };
                let mut row = Value::object()
                    .with("address", l.address)
                    .with("size", u32::from(l.size))
                    .with("hex", hex.as_str())
                    .with("mnemonic", l.mnemonic.as_str())
                    .with("operands", l.operands.as_str());
                if let Some(target) = l.target {
                    row.set("target", Value::from(target));
                    if let Some(text) = debug::symbol_text(emu, target) {
                        row.set("target_symbol", Value::from(text));
                    }
                }
                if let Some((name, start, _)) = emu.symbol_at(l.address)
                    && start == l.address
                {
                    row.set("label", Value::from(crate::symbols::demangle(name)));
                }
                row
            })
            .collect::<Vec<_>>();
        Ok(Value::object().with("lines", rows))
    }
}
