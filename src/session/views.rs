//! Mise en JSON des vues de l'émulateur (structures, mémoire, fichiers…).

use crate::emulator::{Fault, RunStop, Usage};
use crate::inspect::{Found, ModelClass, Structures};
use crate::json::Value;
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

