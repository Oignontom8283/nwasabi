//! Les fichiers de l'hôte et ceux de l'application sont les mêmes : l'application lit le tampon
//! du système de fichiers en mémoire, et l'hôte le modifie pendant qu'elle tourne.

mod support;

use nwasabi::emulator::{Emulator, RunStop};
use nwasabi::profile::Model;
use nwasabi::storage::FsError;

/// Laisse tourner l'application pendant 20 millions d'instructions (elle dessine sa page, puis
/// attend en dormant).
fn settle(emu: &mut Emulator) {
    let end = emu.cpu.cycles + 20_000_000;
    while emu.cpu.cycles < end {
        if let RunStop::Fault(f) = emu.run(1_000_000) {
            panic!("{f:?}");
        }
        // Une application qui dort rend la main presque aussitôt : on fait passer du temps.
        emu.cpu.cycles += 10_000;
    }
}

fn screen(emu: &Emulator) -> Vec<u16> {
    emu.kernel.display.pixels().to_vec()
}

#[test]
fn host_files_show_up_in_the_storage_app_while_it_runs() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("storage-1.1.1"))
        .expect("storage-1.1.1.nwa dans le corpus");
    let mut emu = Emulator::new(Model::N0110).unwrap();
    emu.install_nwa(nwa, None).unwrap();

    settle(&mut emu);
    let before = screen(&emu);
    assert!(
        before.iter().any(|&p| p != 0),
        "la première page est dessinée"
    );
    assert_eq!(emu.fs().list().unwrap().files.len(), 0);

    emu.fs().write("hello.py", b"\x00print(1)\x00").unwrap();
    emu.fs().write("data.txt", &[0xaa; 500]).unwrap();
    assert_eq!(emu.fs().list().unwrap().files.len(), 2);

    // L'application, relancée, relit le stockage et affiche autre chose.
    emu.launch().unwrap();
    settle(&mut emu);
    assert!(
        before != screen(&emu),
        "l'application doit voir les fichiers de l'hôte"
    );

    emu.fs().remove("data.txt").unwrap();
    assert_eq!(
        emu.fs().remove("data.txt"),
        Err(FsError::NotFound("data.txt".into()))
    );
}

#[test]
fn python_scripts_have_a_source_view_and_generations_change() {
    let mut emu = Emulator::new(Model::N0110).unwrap();
    let g0 = emu.fs().generation().unwrap();
    assert_eq!(emu.fs().generation().unwrap(), g0, "rien n'a changé");

    let source = nwasabi::storage::PythonSource {
        auto_import: true,
        text: "print('é')".to_owned(),
    };
    emu.fs().write_python("main.py", &source).unwrap();
    assert_eq!(emu.fs().read_python("main.py").unwrap(), source);
    assert!(emu.fs().generation().unwrap() > g0);

    let stats = emu.fs().stats().unwrap();
    assert_eq!(stats.total, 43008);
    assert_eq!(stats.records, 1);
    assert_eq!(stats.used + stats.free, stats.total);
}

#[test]
fn damage_done_by_the_application_is_reported_not_fatal() {
    let mut emu = Emulator::new(Model::N0110).unwrap();
    emu.fs().write("a.py", b"A").unwrap();
    let at = emu.device.profile.storage_address + 4;
    emu.device.memory.write_bytes(at, &[0xff, 0xff]).unwrap();
    let listing = emu.fs().list().unwrap();
    assert_eq!(listing.corrupt_at, Some(0));
    assert!(listing.files.is_empty());
    assert_eq!(
        emu.fs().write("b.py", b""),
        Err(FsError::Corrupt { offset: 0 })
    );
}
