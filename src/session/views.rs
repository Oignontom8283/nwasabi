//! Mise en JSON des vues de l'émulateur (structures, mémoire, fichiers…) : chaque vue est un type qui dérive `Serialize`, converti
//! en valeur JSON (les clés sont triées, la sortie est donc la même quel que soit l'ordre des champs).

use serde::Serialize;

use crate::emulator::{Fault, RunStop, Usage};
use crate::inspect::{Found, ModelClass, Structures};
use crate::json::Value;
use crate::link::LinkedApp;
use crate::memory::RegionInfo;
use crate::storage::{FileKind, Listing, Stats};

/// Une valeur sérialisable en JSON ; ne peut échouer que pour une clé qui n'est pas du texte, ce qu'aucune vue ne fait.
fn json<T: Serialize>(view: &T) -> Value {
    serde_json::to_value(view).expect("une vue est toujours sérialisable")
}

/// Une plage `{"start": …, "end": …}`.
#[derive(Serialize)]
struct Range {
    start: u32,
    end: u32,
}

fn range(start: u32, end: u32) -> Range {
    Range { start, end }
}

/// Une structure lue en mémoire : ses champs, l'adresse où elle a été lue et si ses repères (magics) sont en place.
#[derive(Serialize)]
struct Located<T> {
    #[serde(flatten)]
    fields: T,
    address: u32,
    valid: bool,
}

fn located<'a, T, V>(f: &'a Found<T>, fields: impl FnOnce(&'a T) -> V) -> Located<V> {
    Located {
        fields: fields(&f.value),
        address: f.address,
        valid: f.valid,
    }
}

#[derive(Serialize)]
struct SlotInfoView {
    kernel_header: u32,
    userland_header: u32,
}

#[derive(Serialize)]
struct SlotHeaderView {
    security_level: u32,
    payload_length: u32,
}

#[derive(Serialize)]
struct KernelHeaderView<'a> {
    software_version: &'a str,
    commit_hash: &'a str,
}

#[derive(Serialize)]
struct UserlandHeaderView<'a> {
    expected_software_version: &'a str,
    storage_address: u32,
    storage_size: u32,
    external_apps_flash: Range,
    external_apps_ram: Range,
    device_name_area: Range,
}

#[derive(Serialize)]
struct VectorTableView {
    initial_stack: u32,
    start: u32,
    draw_string: u32,
}

#[derive(Serialize)]
struct StorageView {
    address: u32,
    size: u32,
    header_magic_ok: bool,
    footer_magic_ok: bool,
    first_record_size: u32,
}

#[derive(Serialize)]
struct AppEntryView<'a> {
    address: u32,
    name: Option<&'a str>,
    api_level: u32,
    size: u32,
}

#[derive(Serialize)]
struct StructuresView<'a> {
    sram_base: Option<u32>,
    model_class: Option<&'static str>,
    slot_info: Option<Located<SlotInfoView>>,
    slot_header: Option<Located<SlotHeaderView>>,
    kernel_header: Option<Located<KernelHeaderView<'a>>>,
    userland_header: Option<Located<UserlandHeaderView<'a>>>,
    vector_table: Option<Located<VectorTableView>>,
    storage: Option<StorageView>,
    device_name: Option<&'a str>,
    apps: Vec<AppEntryView<'a>>,
}

pub fn structures(s: &Structures) -> Value {
    json(&StructuresView {
        sram_base: s.sram_base,
        model_class: s.model_class.map(|m| match m {
            ModelClass::N0110OrN0115 => "n0110_ou_n0115",
            ModelClass::N0120 => "n0120",
        }),
        slot_info: s.slot_info.as_ref().map(|f| {
            located(f, |v| SlotInfoView {
                kernel_header: v.kernel_header,
                userland_header: v.userland_header,
            })
        }),
        slot_header: s.slot_header.as_ref().map(|f| {
            located(f, |v| SlotHeaderView {
                security_level: v.security_level,
                payload_length: v.payload_length,
            })
        }),
        kernel_header: s.kernel_header.as_ref().map(|f| {
            located(f, |v| KernelHeaderView {
                software_version: v.software_version.as_str(),
                commit_hash: v.commit_hash.as_str(),
            })
        }),
        userland_header: s.userland_header.as_ref().map(|f| {
            located(f, |v| UserlandHeaderView {
                expected_software_version: v.expected_software_version.as_str(),
                storage_address: v.storage_address,
                storage_size: v.storage_size,
                external_apps_flash: range(v.external_apps_flash.0, v.external_apps_flash.1),
                external_apps_ram: range(v.external_apps_ram.0, v.external_apps_ram.1),
                device_name_area: range(v.device_name.0, v.device_name.1),
            })
        }),
        vector_table: s.vector_table.as_ref().map(|f| {
            located(f, |v| VectorTableView {
                initial_stack: v.initial_stack,
                start: v.start,
                draw_string: v.draw_string,
            })
        }),
        storage: s.storage.as_ref().map(|v| StorageView {
            address: v.address,
            size: v.size,
            header_magic_ok: v.header_magic_ok,
            footer_magic_ok: v.footer_magic_ok,
            first_record_size: u32::from(v.first_record_size),
        }),
        device_name: s.device_name.as_deref(),
        apps: s
            .apps
            .iter()
            .map(|a| AppEntryView {
                address: a.address,
                name: a.name.as_deref(),
                api_level: a.header.api_level,
                size: a.header.size,
            })
            .collect(),
    })
}

#[derive(Serialize)]
struct AppView<'a> {
    name: &'a str,
    api_level: u32,
    flash_start: u32,
    flash_size: usize,
    entry: u32,
    app_end: u32,
    data: Range,
    bss: Range,
    heap: Range,
    warnings: Vec<String>,
}

pub fn app(app: &LinkedApp) -> Value {
    json(&AppView {
        name: app.name.as_str(),
        api_level: app.api_level,
        flash_start: app.flash_start,
        flash_size: app.flash.len(),
        entry: app.entry,
        app_end: app.app_end,
        data: range(app.data.address, app.data.address + app.data.size),
        bss: range(app.bss.address, app.bss.address + app.bss.size),
        heap: range(app.heap.address, app.heap.address + app.heap.size),
        warnings: app.warnings.iter().map(|w| format!("{w:?}")).collect(),
    })
}

#[derive(Serialize)]
struct RegionView<'a> {
    name: &'a str,
    base: u32,
    size: u32,
    perms: String,
    kind: String,
}

pub fn regions(regions: &[RegionInfo]) -> Value {
    let views: Vec<RegionView> = regions
        .iter()
        .map(|r| RegionView {
            name: r.name.as_str(),
            base: r.base,
            size: r.size,
            perms: format!(
                "{}{}{}",
                if r.perms.read { 'r' } else { '-' },
                if r.perms.write { 'w' } else { '-' },
                if r.perms.execute { 'x' } else { '-' }
            ),
            kind: format!("{:?}", r.kind).to_lowercase(),
        })
        .collect();
    json(&views)
}

#[derive(Serialize)]
struct UsageView {
    heap: Range,
    heap_break: u32,
    heap_high_water: Option<u32>,
    stack: Range,
    stack_low_water: Option<u32>,
    sp: u32,
}

pub fn usage(u: &Usage) -> Value {
    json(&UsageView {
        heap: range(u.heap.start, u.heap.end),
        heap_break: u.heap_break,
        heap_high_water: u.heap_high_water,
        stack: range(u.stack.start, u.stack.end),
        stack_low_water: u.stack_low_water,
        sp: u.sp,
    })
}

#[derive(Serialize)]
struct FileView<'a> {
    name: &'a str,
    kind: String,
    size: usize,
    offset: usize,
}

#[derive(Serialize)]
struct ListingView<'a> {
    files: Vec<FileView<'a>>,
    corrupt_at: Option<usize>,
}

pub fn listing(l: &Listing) -> Value {
    json(&ListingView {
        files: l
            .files
            .iter()
            .map(|f| FileView {
                name: f.name.as_str(),
                kind: match &f.kind {
                    FileKind::Python => "python".to_owned(),
                    FileKind::System => "system".to_owned(),
                    FileKind::Other(ext) => format!("other:{ext}"),
                },
                size: f.size,
                offset: f.offset,
            })
            .collect(),
        corrupt_at: l.corrupt_at,
    })
}

#[derive(Serialize)]
struct StatsView {
    total: usize,
    used: usize,
    free: usize,
    records: usize,
}

pub fn stats(s: &Stats) -> Value {
    json(&StatsView {
        total: s.total,
        used: s.used,
        free: s.free,
        records: s.records,
    })
}

/// La faute qui arrête l'application : `{"fault": "...", …}`.
#[derive(Serialize)]
#[serde(tag = "fault", rename_all = "snake_case")]
enum FaultView {
    Bus {
        address: u32,
        write: bool,
        width: u32,
        access: String,
    },
    Unimplemented {
        pc: u32,
        halfword: u32,
    },
    Usage {
        pc: u32,
        kind: String,
    },
    Breakpoint {
        imm8: u32,
    },
    WaitForInterrupt,
    UnknownSvc {
        number: u32,
    },
}

fn fault(f: &Fault) -> FaultView {
    match f {
        Fault::Bus(b) => FaultView::Bus {
            address: b.address,
            write: b.write,
            width: match b.width {
                crate::cpu::Width::U8 => 1,
                crate::cpu::Width::U16 => 2,
                crate::cpu::Width::U32 => 4,
            },
            access: format!("{:?}", b.kind).to_lowercase(),
        },
        Fault::Unimplemented { pc, halfword } => FaultView::Unimplemented {
            pc: *pc,
            halfword: u32::from(*halfword),
        },
        Fault::UsageFault { pc, kind } => FaultView::Usage {
            pc: *pc,
            kind: format!("{kind:?}"),
        },
        Fault::Breakpoint { imm8 } => FaultView::Breakpoint {
            imm8: u32::from(*imm8),
        },
        Fault::WaitForInterrupt => FaultView::WaitForInterrupt,
        Fault::UnknownSvc(n) => FaultView::UnknownSvc {
            number: u32::from(*n),
        },
    }
}

/// La raison d'arrêt : `{"kind": "...", …}`.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StopView {
    Frame,
    Sleep {
        ms: u32,
    },
    WaitEvent {
        remaining_ms: Option<u32>,
    },
    Exited {
        code: i32,
    },
    Reset,
    HomePressed,
    Budget,
    Watchpoint {
        id: u32,
        address: u32,
        width: u32,
        write: bool,
        value: u32,
        pc: u32,
    },
    Breakpoint {
        pc: u32,
    },
    Fault(FaultView),
}

/// Description de la raison d'arrêt : `{"kind": "...", …}`.
pub fn stop(stop: &RunStop) -> Value {
    json(&match stop {
        RunStop::Frame => StopView::Frame,
        RunStop::Sleep { ms } => StopView::Sleep { ms: *ms },
        RunStop::WaitEvent { remaining_ms } => StopView::WaitEvent {
            remaining_ms: *remaining_ms,
        },
        RunStop::Exited(code) => StopView::Exited { code: *code },
        RunStop::Reset => StopView::Reset,
        RunStop::HomePressed => StopView::HomePressed,
        RunStop::Budget => StopView::Budget,
        RunStop::Watchpoint(h) => StopView::Watchpoint {
            id: h.id.0,
            address: h.address,
            width: u32::from(h.width),
            write: h.write,
            value: h.value,
            pc: h.pc,
        },
        RunStop::Breakpoint { pc } => StopView::Breakpoint { pc: *pc },
        RunStop::Fault(f) => StopView::Fault(fault(f)),
    })
}
