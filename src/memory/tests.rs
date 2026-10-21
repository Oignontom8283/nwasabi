use super::*;
use crate::cpu::{Bus, FaultKind, Width};

fn spec(name: &str, base: u32, size: u32, perms: Perms, fill: u8) -> RegionSpec {
    RegionSpec {
        name: name.into(),
        base,
        size,
        perms,
        kind: RegionKind::Ram,
        fill,
    }
}

/// Une flash de 16 Kio à 0x9000_0000 (lecture/exécution) et une RAM de 16 Kio à 0x2000_0000.
fn sample() -> Memory {
    let mut m = Memory::new();
    m.add_region(spec(
        "flash",
        0x9000_0000,
        0x4000,
        Perms::READ_EXECUTE,
        0xff,
    ))
    .unwrap();
    m.add_region(spec(
        "ram",
        0x2000_0000,
        0x4000,
        Perms::READ_WRITE_EXECUTE,
        0x00,
    ))
    .unwrap();
    m
}

#[test]
fn unwritten_memory_reads_as_the_fill_value_without_allocating() {
    let mut m = sample();
    assert_eq!(m.read32(0x9000_0100), Ok(0xffff_ffff));
    assert_eq!(m.read32(0x2000_0100), Ok(0));
    assert_eq!(m.allocated_pages("flash"), Some(0));
    assert_eq!(m.allocated_pages("ram"), Some(0));
}

#[test]
fn writes_allocate_only_the_touched_pages() {
    let mut m = sample();
    m.write32(0x2000_1000, 0xdead_beef).unwrap();
    m.write8(0x2000_3fff, 1).unwrap();
    assert_eq!(m.allocated_pages("ram"), Some(2));
    assert_eq!(m.read32(0x2000_1000), Ok(0xdead_beef));
    // Le reste d'une page allouée garde la valeur de remplissage.
    assert_eq!(m.read32(0x2000_1004), Ok(0));
}

#[test]
fn values_are_little_endian_and_unaligned_access_works() {
    let mut m = sample();
    m.write32(0x2000_0001, 0x4433_2211).unwrap();
    assert_eq!(m.read8(0x2000_0001), Ok(0x11));
    assert_eq!(m.read8(0x2000_0004), Ok(0x44));
    assert_eq!(m.read16(0x2000_0002), Ok(0x3322));
    assert_eq!(m.read32(0x2000_0001), Ok(0x4433_2211));
}

#[test]
fn accesses_may_cross_a_page_boundary() {
    let mut m = sample();
    m.write32(0x2000_0ffe, 0xaabb_ccdd).unwrap();
    assert_eq!(m.read32(0x2000_0ffe), Ok(0xaabb_ccdd));
    assert_eq!(m.read16(0x2000_1000), Ok(0xaabb));
    assert_eq!(m.allocated_pages("ram"), Some(2));
}

#[test]
fn unmapped_addresses_fault_with_their_details() {
    let mut m = sample();
    let fault = m.read32(0x4000_0000).unwrap_err();
    assert_eq!(fault.address, 0x4000_0000);
    assert_eq!(
        (fault.write, fault.width, fault.kind),
        (false, Width::U32, FaultKind::Unmapped)
    );
    let fault = m.write8(0x1234_5678, 0).unwrap_err();
    assert_eq!((fault.write, fault.width), (true, Width::U8));
    // Un accès qui déborde de la fin d'une zone échoue aussi.
    assert_eq!(m.read32(0x2000_3ffe).unwrap_err().kind, FaultKind::Unmapped);
}

#[test]
fn permissions_are_enforced_for_the_program() {
    let mut m = sample();
    // Écrire dans la flash est refusé, la lire est permis.
    assert_eq!(
        m.write32(0x9000_0000, 0).unwrap_err().kind,
        FaultKind::AccessDenied
    );
    assert!(m.read32(0x9000_0000).is_ok());
    // Une zone sans droit d'exécution refuse la lecture d'instructions.
    m.add_region(spec("rodata", 0x9100_0000, 0x1000, Perms::READ_ONLY, 0))
        .unwrap();
    assert_eq!(
        m.fetch16(0x9100_0000).unwrap_err().kind,
        FaultKind::AccessDenied
    );
    assert!(m.read16(0x9100_0000).is_ok());
    // Une zone réservée refuse tout.
    m.add_region(spec("kernel", 0x2004_0000, 0x1000, Perms::NONE, 0))
        .unwrap();
    assert_eq!(
        m.read8(0x2004_0000).unwrap_err().kind,
        FaultKind::AccessDenied
    );
}

#[test]
fn host_access_ignores_permissions() {
    let mut m = sample();
    m.write_bytes(0x9000_0010, &[1, 2, 3, 4]).unwrap();
    assert_eq!(m.read_bytes(0x9000_0010, 4).unwrap(), [1, 2, 3, 4]);
    assert_eq!(m.read32(0x9000_0010), Ok(0x0403_0201));
    m.write_u32(0x9000_0020, 0xcafe_f00d).unwrap();
    assert_eq!(m.read_u32(0x9000_0020), Ok(0xcafe_f00d));
    m.fill(0x9000_0100, 8, 0xab).unwrap();
    assert_eq!(m.read_bytes(0x9000_0100, 8).unwrap(), [0xab; 8]);
    assert_eq!(
        m.read_bytes(0x5000_0000, 4),
        Err(MemoryError::Unmapped {
            address: 0x5000_0000
        })
    );
    assert_eq!(
        m.write_bytes(0x2000_3ffe, &[0; 4]),
        Err(MemoryError::Unmapped {
            address: 0x2000_3ffe
        })
    );
}

#[test]
fn access_tracking_marks_lines_read_written_and_executed() {
    let mut m = sample();
    m.read32(0x2000_0000).unwrap();
    m.write8(0x2000_0040, 1).unwrap();
    m.fetch16(0x2000_0080).unwrap();
    // Un accès à cheval sur deux lignes les marque toutes les deux.
    m.read32(0x2000_00fe).unwrap();
    let map = m.access_map("ram").unwrap();
    assert_eq!(map[0], access::READ);
    assert_eq!(map[2], access::WRITE);
    assert_eq!(map[4], access::EXECUTE);
    assert_eq!((map[7], map[8]), (access::READ, access::READ));
    assert_eq!(map[1], 0);
    m.clear_access();
    assert!(m.access_map("ram").unwrap().iter().all(|&f| f == 0));
}

#[test]
fn host_accesses_do_not_count_as_program_activity() {
    let mut m = sample();
    m.write_bytes(0x2000_0000, &[1; 64]).unwrap();
    m.read_bytes(0x2000_0000, 64).unwrap();
    assert!(m.access_map("ram").unwrap().iter().all(|&f| f == 0));
}

#[test]
fn changes_are_reported_since_a_cursor_and_merged() {
    let mut m = sample();
    let start = m.change_cursor();
    assert!(m.changes_since(start).is_empty());
    m.write8(0x2000_0000, 1).unwrap();
    m.write8(0x2000_0020, 1).unwrap(); // ligne voisine : fusionnée
    m.write8(0x2000_0200, 1).unwrap();
    assert_eq!(
        m.changes_since(start),
        [
            ChangedRange {
                start: 0x2000_0000,
                len: 64
            },
            ChangedRange {
                start: 0x2000_0200,
                len: 32
            }
        ]
    );
    // Avec un curseur plus récent, seules les dernières modifications restent.
    let mid = m.change_cursor();
    m.write_bytes(0x9000_0000, &[0; 4]).unwrap(); // une écriture de l'hôte compte aussi
    assert_eq!(
        m.changes_since(mid),
        [ChangedRange {
            start: 0x9000_0000,
            len: 32
        }]
    );
    assert_eq!(m.changes_since(m.change_cursor()), []);
}

#[test]
fn regions_are_validated() {
    let mut m = sample();
    let overlap = m.add_region(spec("clash", 0x2000_2000, 0x2000, Perms::NONE, 0));
    assert_eq!(
        overlap,
        Err(MemoryError::Overlap {
            name: "clash".into(),
            other: "ram".into()
        })
    );
    for (base, size) in [
        (0x1000_0001, 0x1000),
        (0x1000_0000, 0x1001),
        (0x1000_0000, 0),
        (0xffff_f000, 0x2000),
    ] {
        assert!(
            matches!(
                m.add_region(spec("bad", base, size, Perms::NONE, 0)),
                Err(MemoryError::BadRegion { .. })
            ),
            "{base:#x}+{size:#x}"
        );
    }
    let names: Vec<String> = m.regions().into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["flash", "ram"]);
}

#[test]
fn a_whole_flash_is_cheap_until_it_is_written() {
    let mut m = Memory::new();
    m.add_region(spec(
        "flash",
        0x9000_0000,
        8 << 20,
        Perms::READ_EXECUTE,
        0xff,
    ))
    .unwrap();
    assert_eq!(m.allocated_pages("flash"), Some(0));
    m.write_bytes(0x9020_0000, &[0; 5000]).unwrap();
    assert_eq!(m.allocated_pages("flash"), Some(2));
}
