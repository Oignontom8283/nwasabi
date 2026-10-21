//! L'appareil fabriqué : structures en mémoire, installation et relecture des applications.

mod support;

use nwasabi::cpu::{Bus, FaultKind};
use nwasabi::device::{DeviceError, DeviceImage, list_apps};
use nwasabi::elf::{SHF_ALLOC, SHF_WRITE, SymbolBind, SymbolKind, SymbolSection};
use nwasabi::link::{LinkOptions, LinkedApp, link};
use nwasabi::profile::{FirmwareProfile, Model};
use nwasabi::structures::{
    KernelHeader, SLOT_INFO_SIZE, STORAGE_MAGIC, SlotHeader, SlotInfo, USERLAND_HEADER_SIZE,
    UserlandHeader, VectorTable,
};
use support::synthetic_app;

fn device(model: Model) -> DeviceImage {
    DeviceImage::new(FirmwareProfile::default_for(model)).unwrap()
}

fn read<const N: usize>(d: &mut DeviceImage, address: u32) -> [u8; N] {
    d.memory
        .read_bytes(address, N as u32)
        .unwrap()
        .try_into()
        .unwrap()
}

/// Lie une application synthétique pour le prochain emplacement libre de l'appareil.
fn linked_for(d: &DeviceImage, app: support::SyntheticApp) -> LinkedApp {
    link(
        &app.finish(),
        None,
        &d.app_layout(),
        &LinkOptions::default(),
    )
    .unwrap()
}

#[test]
fn slot_info_sits_at_the_start_of_the_sram_of_each_model() {
    for model in Model::ALL {
        let mut d = device(model);
        let info = SlotInfo::decode(&read::<SLOT_INFO_SIZE>(&mut d, model.sram_base()));
        assert!(info.is_valid(), "{}", model.name());
        assert_eq!(info.kernel_header, d.profile.slot_a.kernel_header);
        assert_eq!(info.userland_header, d.profile.slot_a.userland_header);
    }
}

#[test]
fn kernel_and_slot_headers_are_written_for_both_slots() {
    let mut d = device(Model::N0110);
    for slot in [d.profile.slot_a, d.profile.slot_b] {
        let header = SlotHeader::decode(&read::<8>(&mut d, slot.origin));
        assert_eq!(
            (header.security_level, header.payload_length),
            (2, 1_610_080)
        );
        let kernel = KernelHeader::decode(&read::<24>(&mut d, slot.kernel_header));
        assert!(kernel.is_valid());
        assert_eq!(
            (
                kernel.software_version.as_str(),
                kernel.commit_hash.as_str()
            ),
            ("26.4.0", "cc9262e")
        );
    }
    // Le slot A est à 0x9000_0000 : l'en-tête du noyau est à +8, comme sur le firmware officiel.
    assert_eq!(d.profile.slot_a.kernel_header, 0x9000_0008);
}

#[test]
fn userland_header_has_the_values_measured_on_the_official_firmware() {
    let mut d = device(Model::N0110);
    let header = UserlandHeader::decode(&read::<USERLAND_HEADER_SIZE>(&mut d, 0x9002_0000));
    assert!(header.is_valid());
    assert_eq!(header.expected_software_version, "26.4.0");
    assert_eq!(
        (header.storage_address, header.storage_size),
        (0x2000_657c, 0xa800)
    );
    assert_eq!(header.external_apps_flash, (0x9020_0000, 0x903f_0000));
    assert_eq!(header.external_apps_ram, (0x2001_18a4, 0x2003_7000));
    assert_eq!(header.device_name, (0x903f_0000, 0x903f_0400));
    // Le slot B publie sa propre zone d'applications et son propre nom d'appareil.
    let b = UserlandHeader::decode(&read::<USERLAND_HEADER_SIZE>(&mut d, 0x9042_0000));
    assert_eq!(b.external_apps_flash, (0x9060_0000, 0x907f_0000));
    assert_eq!(b.device_name, (0x907f_0000, 0x907f_0400));
    assert_eq!(b.storage_address, header.storage_address);
}

#[test]
fn the_vector_table_gives_the_stack_and_a_draw_string_stub() {
    let mut d = device(Model::N0110);
    let table = VectorTable::decode(&read::<12>(&mut d, 0x9002_0030));
    assert_eq!(table.initial_stack, 0x2003_eff8);
    // drawString : fonction Thumb dont le code est `svc #0x80 ; bx lr`.
    assert_eq!(table.draw_string & 1, 1);
    assert_eq!(
        read::<4>(&mut d, table.draw_string & !1),
        [0x80, 0xdf, 0x70, 0x47]
    );
    // Un lecteur de la trampoline (comme eadk_display_draw_string) lit ce même pointeur à l'adresse de la trampoline (+0x38).
    let tramp = d.profile.trampoline_address();
    assert_eq!(d.memory.read_u32(tramp).unwrap(), table.draw_string);
}

#[test]
fn the_filesystem_starts_empty_between_two_magics() {
    let mut d = device(Model::N0110);
    let p = d.profile.clone();
    assert_eq!(d.memory.read_u32(p.storage_address).unwrap(), STORAGE_MAGIC);
    assert_eq!(
        d.memory
            .read_u32(p.storage_address + 4 + p.storage_size)
            .unwrap(),
        STORAGE_MAGIC
    );
    // Le premier enregistrement a une taille nulle : le système de fichiers est vide.
    assert_eq!(
        d.memory.read_bytes(p.storage_address + 4, 2).unwrap(),
        [0, 0]
    );
}

#[test]
fn the_apps_area_and_the_persisting_bytes_start_erased() {
    let mut d = device(Model::N0110);
    let (start, end) = d.profile.slot_a.apps_flash;
    assert!(
        d.memory
            .read_bytes(start, end - start)
            .unwrap()
            .iter()
            .all(|&b| b == 0xff)
    );
    let (name, _) = d.profile.slot_a.device_name;
    assert!(
        d.memory
            .read_bytes(name, 0x1_0000)
            .unwrap()
            .iter()
            .all(|&b| b == 0xff)
    );
}

#[test]
fn a_device_name_is_written_in_the_persisting_bytes() {
    let mut profile = FirmwareProfile::default_for(Model::N0110);
    profile.device_name = "Ma calculatrice".into();
    let mut d = DeviceImage::new(profile).unwrap();
    assert_eq!(
        d.memory.read_bytes(0x903f_0000, 16).unwrap(),
        b"Ma calculatrice\0"
    );
}

#[test]
fn an_invalid_profile_is_refused() {
    let mut profile = FirmwareProfile::default_for(Model::N0110);
    profile.serial_number = "trop court".into();
    assert!(matches!(
        DeviceImage::new(profile),
        Err(DeviceError::Profile(_))
    ));
}

#[test]
fn the_program_cannot_write_flash_or_touch_kernel_ram() {
    let mut d = device(Model::N0110);
    let flash = d.memory.write32(0x9000_0000, 0).unwrap_err();
    assert_eq!(flash.kind, FaultKind::AccessDenied);
    let kernel_ram = d.memory.read32(0x2003_f000).unwrap_err();
    assert_eq!(kernel_ram.kind, FaultKind::AccessDenied);
    // La RAM des applications est librement utilisable, et le SlotInfo s'y lit.
    d.memory.write32(0x2001_18a4, 1).unwrap();
    assert_eq!(d.memory.read32(0x2000_0000).unwrap(), 0xefee_dbba);
    // Les périphériques ne sont pas (encore) émulés.
    assert_eq!(
        d.memory.read32(0x4000_0000).unwrap_err().kind,
        FaultKind::Unmapped
    );
}

#[test]
fn ram_starts_filled_with_the_requested_value() {
    use nwasabi::device::DeviceOptions;
    let profile = FirmwareProfile::default_for(Model::N0110);
    let mut d = DeviceImage::with_options(profile, DeviceOptions { ram_fill: 0xa5 }).unwrap();
    assert_eq!(d.memory.read32(0x2002_0000).unwrap(), 0xa5a5_a5a5);
}

#[test]
fn an_installed_app_is_found_again_by_scanning_memory() {
    let mut d = device(Model::N0110);
    assert!(d.apps().is_empty());
    let app = linked_for(&d, synthetic_app());
    let address = d.install(&app).unwrap();
    assert_eq!(address, 0x9020_0000);

    let apps = d.apps();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].address, address);
    assert_eq!(apps[0].name.as_deref(), Some("Test"));
    assert_eq!(apps[0].header.api_level, 0);
    assert_eq!(apps[0].header.size, app.app_end - app.flash_start);
    // Le point d'entrée est celui de `_start` (sans le bit Thumb, ajouté par le système).
    assert_eq!(address + apps[0].header.entry_offset + 1, app.entry);
}

#[test]
fn apps_are_installed_one_after_the_other_on_sector_boundaries() {
    let mut d = device(Model::N0110);
    let mut addresses = Vec::new();
    for _ in 0..3 {
        let app = linked_for(&d, synthetic_app());
        addresses.push(d.install(&app).unwrap());
    }
    assert_eq!(addresses, [0x9020_0000, 0x9021_0000, 0x9022_0000]);
    assert_eq!(d.apps().len(), 3);
    assert_eq!(d.next_app_address(), 0x9023_0000);
    // Le complément jusqu'au secteur est effacé (0xFF).
    let first = d.memory.read_bytes(0x9020_0000 + 0x2000, 16).unwrap();
    assert_eq!(first, [0xff; 16]);
}

#[test]
fn an_app_linked_for_another_place_is_refused() {
    let mut d = device(Model::N0110);
    let first = linked_for(&d, synthetic_app());
    d.install(&first).unwrap();
    // Reposer la même image (liée pour 0x9020_0000) à l'emplacement suivant est refusé.
    assert!(matches!(
        d.install(&first),
        Err(DeviceError::WrongPlace { .. })
    ));
}

#[test]
fn installation_stops_when_the_apps_area_is_full() {
    let mut d = device(Model::N0110);
    let mut count = 0;
    loop {
        let layout = d.app_layout();
        let Ok(app) = link(
            &synthetic_app().finish(),
            None,
            &layout,
            &LinkOptions::default(),
        ) else {
            break; // l'éditeur de liens constate qu'il n'y a plus de place
        };
        d.install(&app).unwrap();
        count += 1;
    }
    // 0x1F0000 octets de zone / 0x10000 par application.
    assert_eq!(count, 31);
    assert_eq!(d.apps().len(), 31);
    // Installer de force une image trop grande est aussi refusé par l'appareil.
    let mut big = linked_for(&device(Model::N0110), synthetic_app());
    big.flash = vec![0; 0x20_0000];
    assert!(matches!(
        device(Model::N0110).install(&big),
        Err(DeviceError::NoRoom { .. })
    ));
}

#[test]
fn erasing_the_apps_empties_the_list() {
    let mut d = device(Model::N0110);
    let app = linked_for(&d, synthetic_app());
    d.install(&app).unwrap();
    d.erase_apps().unwrap();
    assert!(d.apps().is_empty());
    assert_eq!(d.next_app_address(), 0x9020_0000);
}

#[test]
fn exam_mode_hides_every_external_app() {
    let mut d = device(Model::N0110);
    let app = linked_for(&d, synthetic_app());
    d.install(&app).unwrap();
    let (start, end) = d.profile.slot_a.apps_flash;
    assert_eq!(list_apps(&mut d.memory, start, end, false).len(), 1);
    assert!(list_apps(&mut d.memory, start, end, true).is_empty());
}

#[test]
fn a_data_image_that_crosses_a_sector_hides_the_next_app_as_on_the_real_device() {
    // Première application : ses constantes finissent juste avant la limite d'un secteur, mais la
    // copie de `.data` la franchit. L'en-tête annonce donc une taille trop courte.
    let build = |d: &DeviceImage, rodata: usize| {
        let mut app = synthetic_app();
        let ro = app
            .writer
            .add_section(".rodata.pad", SHF_ALLOC, 1, &vec![0; rodata]);
        let pad = app.writer.add_symbol(
            "pad",
            0,
            rodata as u32,
            SymbolBind::Global,
            SymbolKind::Object,
            SymbolSection::Section(ro.0),
        );
        let data = app
            .writer
            .add_section(".data.d", SHF_ALLOC | SHF_WRITE, 4, &[7; 16]);
        let dsym = app.writer.add_symbol(
            "d",
            0,
            16,
            SymbolBind::Global,
            SymbolKind::Object,
            SymbolSection::Section(data.0),
        );
        app.word_ref(pad);
        app.word_ref(dsym);
        linked_for(d, app)
    };
    let mut d = device(Model::N0110);
    let probe = build(&d, 0);
    let rodata = 0x1_0000 - 8 - (probe.app_end - probe.flash_start) as usize;
    let first = build(&d, rodata);
    assert!(
        !first.warnings.is_empty(),
        "le scénario doit provoquer l'avertissement"
    );
    d.install(&first).unwrap();
    // La suivante est installée à l'emplacement que calcule l'outil de flashage…
    let second = linked_for(&d, synthetic_app());
    let second_address = d.install(&second).unwrap();
    assert_eq!(second_address, 0x9022_0000);
    // …mais le système, qui se fie à la taille de l'en-tête, la cherche à 0x9021_0000 et s'arrête.
    assert_eq!(d.apps().len(), 1);
}

#[test]
fn an_app_is_installed_in_the_area_of_the_active_slot() {
    // Sur les appareils relevés, le slot actif est tantôt A, tantôt B : tout suit le slot actif.
    let mut profile = FirmwareProfile::default_for(Model::N0110);
    profile.active_slot = nwasabi::profile::Slot::B;
    let mut d = DeviceImage::new(profile).unwrap();
    let info = SlotInfo::decode(&read::<SLOT_INFO_SIZE>(&mut d, 0x2000_0000));
    assert_eq!(
        (info.kernel_header, info.userland_header),
        (0x9040_0008, 0x9042_0000)
    );

    let app = linked_for(&d, synthetic_app());
    assert_eq!(app.flash_start, 0x9060_0000);
    assert_eq!(d.install(&app).unwrap(), 0x9060_0000);
    let apps = d.apps();
    assert_eq!((apps.len(), apps[0].name.as_deref()), (1, Some("Test")));
    // La trampoline donnée à l'éditeur de liens est celle du userland du slot B.
    assert_eq!(d.app_layout().trampoline_address, 0x9042_0038);
}

#[test]
fn the_size_of_the_apps_area_depends_on_the_model() {
    // N0110 et N0115 : 0x1F0000 octets ; N0120 : 0x270000 (userland plus petit).
    for (model, expected) in [(Model::N0110, 31), (Model::N0115, 31), (Model::N0120, 39)] {
        let mut d = device(model);
        let mut count = 0;
        while let Ok(app) = link(
            &synthetic_app().finish(),
            None,
            &d.app_layout(),
            &LinkOptions::default(),
        ) {
            d.install(&app).unwrap();
            count += 1;
        }
        assert_eq!(count, expected, "{}", model.name());
    }
}
