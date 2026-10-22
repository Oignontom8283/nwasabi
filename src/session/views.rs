//! Mise en JSON des vues de l'émulateur (structures, mémoire, fichiers…).

use crate::emulator::{Fault, RunStop, Usage};
use crate::inspect::{Found, ModelClass, Structures};
use crate::json::{Value, ValueExt};
use crate::link::LinkedApp;
use crate::memory::RegionInfo;
use crate::storage::{FileKind, Listing, Stats};

fn pair(a: u32, b: u32) -> Value {
    Value::object().with("start", a).with("end", b)
}

fn found<T>(f: &Found<T>, body: impl FnOnce(&T) -> Value) -> Value {
    body(&f.value)
        .with("address", f.address)
        .with("valid", f.valid)
}

pub fn structures(s: &Structures) -> Value {
    let mut out = Value::object().with("sram_base", s.sram_base).with(
        "model_class",
        s.model_class.map(|m| match m {
            ModelClass::N0110OrN0115 => "n0110_ou_n0115",
            ModelClass::N0120 => "n0120",
        }),
    );
    out.set(
        "slot_info",
        s.slot_info.as_ref().map(|f| {
            found(f, |v| {
                Value::object()
                    .with("kernel_header", v.kernel_header)
                    .with("userland_header", v.userland_header)
            })
        }),
    );
    out.set(
        "slot_header",
        s.slot_header.as_ref().map(|f| {
            found(f, |v| {
                Value::object()
                    .with("security_level", v.security_level)
                    .with("payload_length", v.payload_length)
            })
        }),
    );
    out.set(
        "kernel_header",
        s.kernel_header.as_ref().map(|f| {
            found(f, |v| {
                Value::object()
                    .with("software_version", v.software_version.as_str())
                    .with("commit_hash", v.commit_hash.as_str())
            })
        }),
    );
    out.set(
        "userland_header",
        s.userland_header.as_ref().map(|f| {
            found(f, |v| {
                Value::object()
                    .with(
                        "expected_software_version",
                        v.expected_software_version.as_str(),
                    )
                    .with("storage_address", v.storage_address)
                    .with("storage_size", v.storage_size)
                    .with(
                        "external_apps_flash",
                        pair(v.external_apps_flash.0, v.external_apps_flash.1),
                    )
                    .with(
                        "external_apps_ram",
                        pair(v.external_apps_ram.0, v.external_apps_ram.1),
                    )
                    .with("device_name_area", pair(v.device_name.0, v.device_name.1))
            })
        }),
    );
    out.set(
        "vector_table",
        s.vector_table.as_ref().map(|f| {
            found(f, |v| {
                Value::object()
                    .with("initial_stack", v.initial_stack)
                    .with("start", v.start)
                    .with("draw_string", v.draw_string)
            })
        }),
    );
    out.set(
        "storage",
        s.storage.as_ref().map(|v| {
            Value::object()
                .with("address", v.address)
                .with("size", v.size)
                .with("header_magic_ok", v.header_magic_ok)
                .with("footer_magic_ok", v.footer_magic_ok)
                .with("first_record_size", u32::from(v.first_record_size))
        }),
    );
    out.set("device_name", s.device_name.as_deref());
    out.set(
        "apps",
        Value::Array(
            s.apps
                .iter()
                .map(|a| {
                    Value::object()
                        .with("address", a.address)
                        .with("name", a.name.as_deref())
                        .with("api_level", a.header.api_level)
                        .with("size", a.header.size)
                })
                .collect(),
        ),
    );
    out
}

pub fn app(app: &LinkedApp) -> Value {
    Value::object()
        .with("name", app.name.as_str())
        .with("api_level", app.api_level)
        .with("flash_start", app.flash_start)
        .with("flash_size", app.flash.len())
        .with("entry", app.entry)
        .with("app_end", app.app_end)
        .with(
            "data",
            pair(app.data.address, app.data.address + app.data.size),
        )
        .with("bss", pair(app.bss.address, app.bss.address + app.bss.size))
        .with(
            "heap",
            pair(app.heap.address, app.heap.address + app.heap.size),
        )
        .with(
            "warnings",
            Value::Array(
                app.warnings
                    .iter()
                    .map(|w| Value::from(format!("{w:?}")))
                    .collect(),
            ),
        )
}

pub fn regions(regions: &[RegionInfo]) -> Value {
    Value::Array(
        regions
            .iter()
            .map(|r| {
                let perms = format!(
                    "{}{}{}",
                    if r.perms.read { 'r' } else { '-' },
                    if r.perms.write { 'w' } else { '-' },
                    if r.perms.execute { 'x' } else { '-' }
                );
                Value::object()
                    .with("name", r.name.as_str())
                    .with("base", r.base)
                    .with("size", r.size)
                    .with("perms", perms)
                    .with("kind", format!("{:?}", r.kind).to_lowercase())
            })
            .collect(),
    )
}

pub fn usage(u: &Usage) -> Value {
    Value::object()
        .with("heap", pair(u.heap.start, u.heap.end))
        .with("heap_break", u.heap_break)
        .with("heap_high_water", u.heap_high_water)
        .with("stack", pair(u.stack.start, u.stack.end))
        .with("stack_low_water", u.stack_low_water)
        .with("sp", u.sp)
}

pub fn listing(l: &Listing) -> Value {
    Value::object()
        .with(
            "files",
            Value::Array(
                l.files
                    .iter()
                    .map(|f| {
                        Value::object()
                            .with("name", f.name.as_str())
                            .with(
                                "kind",
                                match &f.kind {
                                    FileKind::Python => "python".to_owned(),
                                    FileKind::System => "system".to_owned(),
                                    FileKind::Other(ext) => format!("other:{ext}"),
                                },
                            )
                            .with("size", f.size)
                            .with("offset", f.offset)
                    })
                    .collect(),
            ),
        )
        .with("corrupt_at", l.corrupt_at)
}

pub fn stats(s: &Stats) -> Value {
    Value::object()
        .with("total", s.total)
        .with("used", s.used)
        .with("free", s.free)
        .with("records", s.records)
}

fn fault(f: &Fault) -> Value {
    match f {
        Fault::Bus(b) => Value::object()
            .with("fault", "bus")
            .with("address", b.address)
            .with("write", b.write)
            .with("access", format!("{:?}", b.kind).to_lowercase()),
        Fault::Unimplemented { pc, halfword } => Value::object()
            .with("fault", "unimplemented")
            .with("pc", *pc)
            .with("halfword", u32::from(*halfword)),
        Fault::UsageFault { pc, kind } => Value::object()
            .with("fault", "usage")
            .with("pc", *pc)
            .with("kind", format!("{kind:?}")),
        Fault::Breakpoint { imm8 } => Value::object()
            .with("fault", "breakpoint")
            .with("imm8", u32::from(*imm8)),
        Fault::WaitForInterrupt => Value::object().with("fault", "wait_for_interrupt"),
        Fault::UnknownSvc(n) => Value::object()
            .with("fault", "unknown_svc")
            .with("number", u32::from(*n)),
    }
}

/// Description de la raison d'arrêt : `{"kind": "...", …}`.
pub fn stop(stop: &RunStop) -> Value {
    let kind = |k: &str| Value::object().with("kind", k);
    match stop {
        RunStop::Frame => kind("frame"),
        RunStop::Sleep { ms } => kind("sleep").with("ms", *ms),
        RunStop::WaitEvent { remaining_ms } => {
            kind("wait_event").with("remaining_ms", *remaining_ms)
        }
        RunStop::Exited(code) => kind("exited").with("code", *code),
        RunStop::Reset => kind("reset"),
        RunStop::HomePressed => kind("home_pressed"),
        RunStop::Budget => kind("budget"),
        RunStop::Watchpoint(h) => kind("watchpoint")
            .with("id", h.id.0)
            .with("address", h.address)
            .with("width", u32::from(h.width))
            .with("write", h.write)
            .with("value", h.value),
        RunStop::Fault(f) => {
            let mut v = kind("fault");
            if let Value::Object(entries) = fault(f) {
                for (k, val) in entries {
                    v.set(&k, val);
                }
            }
            v
        }
    }
}
