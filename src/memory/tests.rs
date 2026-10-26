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

#[test]
fn watchpoints_report_matching_accesses_of_the_program_only() {
    let mut m = sample();
    let w = m.watch(0x2000_0100, 8, WatchKind::Write, false);
    m.write32(0x2000_00fc, 1).unwrap(); // juste avant : ignoré
    m.write32(0x2000_0104, 0xdead_beef).unwrap();
    m.write8(0x2000_0100, 7).unwrap();
    m.read32(0x2000_0104).unwrap(); // une lecture ne compte pas pour `Write`
    m.write_bytes(0x2000_0100, &[1, 2, 3]).unwrap(); // l'hôte non plus
    let hits = m.take_watch_hits();
    assert_eq!(hits.len(), 2);
    assert_eq!(
        hits[0],
        WatchHit {
            id: w,
            address: 0x2000_0104,
            width: 4,
            write: true,
            value: 0xdead_beef
        }
    );
    assert_eq!((hits[1].width, hits[1].value), (1, 7));
    assert!(m.take_watch_hits().is_empty());
}

#[test]
fn read_watchpoints_report_the_value_read_and_stop_requests_are_remembered() {
    let mut m = sample();
    m.write_u32(0x2000_0200, 0x1234_5678).unwrap();
    let w = m.watch(0x2000_0200, 4, WatchKind::ReadWrite, true);
    assert!(m.has_stopping_watch());
    assert_eq!(m.read16(0x2000_0200), Ok(0x5678));
    let stop = m.take_watch_stop().expect("arrêt demandé");
    assert_eq!((stop.id, stop.value, stop.write), (w, 0x5678, false));
    assert_eq!(m.take_watch_stop(), None);
    assert!(m.unwatch(w));
    assert!(!m.unwatch(w));
    m.read16(0x2000_0200).unwrap();
    assert!(
        m.take_watch_hits().len() == 1,
        "plus de nouveau déclenchement"
    );
}

#[test]
fn access_extent_measures_the_used_part_of_a_range() {
    let mut m = sample();
    assert_eq!(
        m.access_extent(0x2000_0000, 0x2000_4000, access::WRITE),
        None
    );
    m.write8(0x2000_0100, 1).unwrap();
    m.write8(0x2000_0ff0, 1).unwrap();
    assert_eq!(
        m.access_extent(0x2000_0000, 0x2000_4000, access::WRITE),
        Some((0x2000_0100, 0x2000_1000))
    );
    assert_eq!(
        m.access_extent(0x2000_0200, 0x2000_4000, access::WRITE),
        Some((0x2000_0fe0, 0x2000_1000))
    );
}

#[test]
fn exclusive_accesses_fault_in_ram_but_not_in_flash() {
    let mut m = Memory::new();
    let region = |name: &str, base, kind| RegionSpec {
        kind,
        ..spec(name, base, 0x1000, Perms::READ_WRITE_EXECUTE, 0)
    };
    m.add_region(region("flash", 0x9000_0000, RegionKind::Flash))
        .unwrap();
    m.add_region(region("ram", 0x2000_0000, RegionKind::Ram))
        .unwrap();
    m.add_region(region("noyau", 0x2003_f000, RegionKind::Reserved))
        .unwrap();

    // La flash n'est pas partageable : les accès exclusifs y sont permis.
    assert_eq!(m.check_exclusive(0x9000_0000, false, Width::U32), Ok(()));
    // La RAM l'est, et le bus n'a pas de moniteur global : BusFault, en lecture comme en écriture.
    for (address, write, width) in [
        (0x2000_0010, false, Width::U32),
        (0x2000_0010, true, Width::U8),
        (0x2003_f000, false, Width::U16),
    ] {
        let fault = m.check_exclusive(address, write, width).unwrap_err();
        assert_eq!(fault.kind, FaultKind::ExclusiveUnsupported);
        assert_eq!(
            (fault.address, fault.write, fault.width),
            (address, write, width)
        );
    }
    // Hors de toute zone, c'est un accès à de la mémoire absente.
    assert_eq!(
        m.check_exclusive(0x4000_0000, false, Width::U32)
            .unwrap_err()
            .kind,
        FaultKind::Unmapped
    );
    // Les tests du processeur contre QEMU, qui n'a pas cette limite, les autorisent.
    m.allow_exclusive_accesses(true);
    assert_eq!(m.check_exclusive(0x2000_0010, true, Width::U32), Ok(()));
}
