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

