//! Relecture des structures de l'OS dans la mémoire de l'appareil fabriqué.

mod support;

use nwasabi::device::DeviceImage;
use nwasabi::inspect::{ModelClass, detect_sram_base, read_structures};
use nwasabi::link::{LinkOptions, link};
use nwasabi::memory::Memory;
use nwasabi::profile::{FirmwareProfile, Model};
use support::synthetic_app;

fn device(model: Model) -> DeviceImage {
    DeviceImage::new(FirmwareProfile::default_for(model)).unwrap()
}

#[test]
fn a_fresh_device_exposes_every_structure_for_each_model() {
    for model in Model::ALL {
        let mut d = device(model);
        let s = read_structures(&mut d.memory, false);
        let class = if model == Model::N0120 {
            ModelClass::N0120
        } else {
            ModelClass::N0110OrN0115
        };
        assert_eq!(s.model_class, Some(class), "{}", model.name());
        assert_eq!(s.sram_base, Some(model.sram_base()));

        let slot = s.slot_info.expect("SlotInfo");
        assert!(slot.valid);
        assert_eq!(slot.address, model.sram_base());

        let kernel = s.kernel_header.expect("KernelHeader");
        assert!(kernel.valid);
        assert_eq!(kernel.value.software_version, "26.4.0");
        assert_eq!(kernel.value.commit_hash, "cc9262e");
        assert_eq!(
            s.slot_header.expect("en-tête de slot").value.security_level,
            2
        );

        let userland = s.userland_header.expect("UserlandHeader");
        assert!(userland.valid);
        assert_eq!(userland.address, 0x9002_0000);
        assert_eq!(userland.value.storage_address, d.profile.storage_address);
        assert_eq!(
            s.vector_table.expect("vecteurs").value.initial_stack,
            d.profile.process_stack_start
        );

        let storage = s.storage.expect("système de fichiers");
        assert!(storage.header_magic_ok && storage.footer_magic_ok);
        assert_eq!((storage.size, storage.first_record_size), (0xa800, 0));
        assert_eq!(s.device_name.as_deref(), Some(""));
        assert!(s.apps.is_empty());
    }
}

#[test]
fn installed_apps_and_the_device_name_show_up() {
    let mut profile = FirmwareProfile::default_for(Model::N0110);
    profile.device_name = "Salle 12".into();
    let mut d = DeviceImage::new(profile).unwrap();
    let app = link(
        &synthetic_app().finish(),
        None,
        &d.app_layout(),
        &LinkOptions::default(),
    )
    .unwrap();
    d.install(&app).unwrap();
    let s = read_structures(&mut d.memory, false);
    assert_eq!(s.device_name.as_deref(), Some("Salle 12"));
    assert_eq!(s.apps.len(), 1);
    assert_eq!(s.apps[0].name.as_deref(), Some("Test"));
    // En mode examen, les applications externes sont masquées.
    assert!(read_structures(&mut d.memory, true).apps.is_empty());
}

#[test]
fn the_inspection_reads_memory_so_corruption_is_visible() {
    let mut d = device(Model::N0110);
    // L'application (ou l'hôte) abîme le pied de page du UserlandHeader.
    d.memory.write_u32(0x9002_0000 + 44, 0).unwrap();
    let s = read_structures(&mut d.memory, false);
    let userland = s.userland_header.expect("l'en-tête est toujours lisible");
    assert!(!userland.valid, "le magic de fin est cassé");
    // Une structure invalide n'est pas suivie : ni système de fichiers ni applications.
    assert!(s.storage.is_none() && s.apps.is_empty());
    // Le reste reste visible.
    assert!(s.kernel_header.unwrap().valid);

    // Un KernelHeader cassé est signalé sans empêcher le reste.
    let mut d = device(Model::N0110);
    d.memory.write_u32(0x9000_0008, 0).unwrap();
    let s = read_structures(&mut d.memory, false);
    assert!(!s.kernel_header.unwrap().valid);
    assert!(s.userland_header.unwrap().valid);

    // Un système de fichiers dont le magic de fin est écrasé est montré comme tel.
    let mut d = device(Model::N0110);
    let end = d.profile.storage_address + 4 + d.profile.storage_size;
    d.memory.write_u32(end, 0x1234_5678).unwrap();
    let storage = read_structures(&mut d.memory, false).storage.unwrap();
    assert!(storage.header_magic_ok && !storage.footer_magic_ok);
}

#[test]
fn without_a_slot_info_nothing_is_found() {
    let mut d = device(Model::N0110);
    d.memory.write_u32(0x2000_0000, 0).unwrap();
    assert_eq!(detect_sram_base(&mut d.memory), None);
    let s = read_structures(&mut d.memory, false);
    assert!(s.slot_info.is_none() && s.userland_header.is_none() && s.apps.is_empty());
    // Une mémoire vide ne fait pas échouer l'inspection.
    assert!(
        read_structures(&mut Memory::new(), false)
            .sram_base
            .is_none()
    );
}

#[test]
fn a_slot_info_pointing_outside_the_flash_is_ignored() {
    let mut d = device(Model::N0110);
    d.memory.write_u32(0x2000_0008, 0x2000_1000).unwrap(); // pointeur du UserlandHeader hors flash
    assert_eq!(detect_sram_base(&mut d.memory), None);
}

/// La détection de modèle d'eadkp : sonder la flash aux adresses où le `UserlandHeader` se trouvait
/// sur les anciens firmwares (`0x9001_0000` pour N0110/N0115, `0x9002_0000` pour N0120) et compter
/// les magics trouvés.
fn eadkp_probe(memory: &mut Memory) -> ModelClass {
    const MAGIC: u32 = 0xdec0_edfe;
    let mut count = |slots: [u32; 2]| {
        slots
            .iter()
            .filter(|&&a| memory.read_u32(a) == Ok(MAGIC))
            .count()
    };
    let old = count([0x9001_0000, 0x9041_0000]);
    let new = count([0x9002_0000, 0x9042_0000]);
    if old > new {
        ModelClass::N0110OrN0115
    } else {
        ModelClass::N0120
    }
}

#[test]
fn probing_flash_addresses_cannot_tell_n0110_26_4_0_from_an_n0120() {
    // Sur le N0110 en Epsilon 26.4.0 (mesuré), le UserlandHeader est à 0x9002_0000 : la sonde par
    // adresses de flash conclut « N0120 » alors que la SRAM est en 0x2000_0000.
    let mut d = device(Model::N0110);
    assert_eq!(eadkp_probe(&mut d.memory), ModelClass::N0120);
    // La lecture du SlotInfo, elle, ne se trompe pas.
    assert_eq!(
        read_structures(&mut d.memory, false).model_class,
        Some(ModelClass::N0110OrN0115)
    );
}

#[test]
fn probing_flash_addresses_works_on_a_firmware_with_the_older_layout() {
    // Disposition plus ancienne : pas de copie du bootloader, UserlandHeader à 0x9001_0000.
    let mut profile = FirmwareProfile::default_for(Model::N0110);
    profile.slot_a.userland_header = 0x9001_0000;
    profile.slot_b.userland_header = 0x9041_0000;
    let mut d = DeviceImage::new(profile).unwrap();
    assert_eq!(eadkp_probe(&mut d.memory), ModelClass::N0110OrN0115);
    let s = read_structures(&mut d.memory, false);
    assert_eq!(s.userland_header.unwrap().address, 0x9001_0000);
    assert_eq!(s.model_class, Some(ModelClass::N0110OrN0115));
}
