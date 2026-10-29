//! Les fichiers de chaque format de Nwasabi, tels que `tests/apps/build.sh` les range dans `tests/apps/bin/` à côté des `.nwa` :
//! ils sont lisibles, se chargent, et redonnent ce pour quoi ils ont été faits. On ne les recompile pas à chaque test.

use std::path::PathBuf;

use nwasabi::formats::{nds, nfd, nfs, nrp};
use nwasabi::json::{self, Value, ValueExt};
use nwasabi::session::Session;

fn bin(name: &str) -> PathBuf {
    PathBuf::from(format!(
        "{}/tests/apps/bin/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(bin(name)).unwrap_or_else(|e| panic!("{name} : {e} (voir tests/apps/build.sh)"))
}

fn document(name: &str) -> Value {
    json::parse(&String::from_utf8(read(name)).unwrap()).unwrap()
}

fn cmd(name: &str) -> Value {
    Value::object().with("cmd", name)
}

fn call(session: &mut Session, request: Value) -> Value {
    let reply = session.handle(&request);
    assert_eq!(reply["ok"], true, "{request} -> {reply}");
    reply
}

fn load(session: &mut Session, app: &str) {
    call(
        session,
        cmd("load").with("nwa", json::base64_encode(&read(app))),
    );
}

fn screen_crc(session: &mut Session) -> String {
    call(session, cmd("screen"))["crc32"]
        .as_str()
        .unwrap()
        .to_owned()
}

// ---- .nfd ------------------------------------------------------------------------------------------------------------

#[test]
fn the_nfd_fixture_gives_the_app_its_auxiliary_data() {
    let data = document("aux_flash.nfd");
    let segments = nfd::parse(&data).unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].address, 0x9040_0000);
    assert_eq!(&segments[0].data[..5], b"NWAUX");
    assert_eq!(segments[1].data.len(), 64, "seize mots de quatre octets");

    let mut session = Session::new();
    call(
        &mut session,
        cmd("load")
            .with("nwa", json::base64_encode(&read("aux_flash.nwa")))
            .with("nfd", data),
    );
    let reply = call(&mut session, cmd("run").with("instructions", 5_000_000u64));
    assert_eq!(
        (
            reply["stop"]["kind"].as_str(),
            reply["stop"]["code"].as_i64()
        ),
        (Some("exited"), Some(0)),
        "{reply}"
    );
}

// ---- .nds ------------------------------------------------------------------------------------------------------------

#[test]
fn the_nds_fixture_of_an_app_with_auxiliary_data_resumes_and_finishes() {
    let state = document("aux_flash.nds");
    let restored = nds::restore(&state, None).unwrap();
    assert!(restored.parts.contains(&"app") && restored.parts.contains(&"state"));
    assert_eq!(
        restored.emulator.flash_segments().len(),
        2,
        "les données auxiliaires reviennent avec l'état"
    );

    let mut session = Session::new();
    call(&mut session, cmd("restore").with("nds", state));
    let reply = call(&mut session, cmd("run").with("instructions", 5_000_000u64));
    assert_eq!(
        (
            reply["stop"]["kind"].as_str(),
            reply["stop"]["code"].as_i64()
        ),
        (Some("exited"), Some(0)),
        "{reply}"
    );
}

#[test]
fn resuming_the_nds_fixture_gives_exactly_the_uninterrupted_run() {
    // keys.nds a été pris après trois images, avec la touche OK enfoncée : trois images de plus donnent l'écran de six.
    let mut resumed = Session::new();
    call(
        &mut resumed,
        cmd("restore").with("nds", document("keys.nds")),
    );
    call(&mut resumed, cmd("run").with("frames", 3u64));

    let mut straight = Session::new();
    load(&mut straight, "keys.nwa");
    call(
        &mut straight,
        cmd("key").with("key", "ok").with("action", "press"),
    );
    call(&mut straight, cmd("run").with("frames", 6u64));
    assert_eq!(screen_crc(&mut resumed), screen_crc(&mut straight));
    assert_eq!(
        call(&mut resumed, cmd("cpu"))["pc"],
        call(&mut straight, cmd("cpu"))["pc"]
    );
}

// ---- .nrp ------------------------------------------------------------------------------------------------------------

fn replay(recording: &str, app: &str) -> nrp::Report {
    let nwa = read(app);
    let material = nrp::Material {
        nwa: &nwa,
        data: None,
        flash: None,
    };
    nrp::replay(&document(recording), &material, &mut Session::new()).unwrap()
}

#[test]
fn the_nrp_fixtures_replay_identically() {
    for (recording, app) in [
        ("keys.nrp", "keys.nwa"),
        ("flash_rw.nrp", "flash_rw.nwa"),
        ("debugme.nrp", "debugme.nwa"),
    ] {
        let report = replay(recording, app);
        assert!(
            report.mismatches.is_empty(),
            "{recording} : {:?}",
            report.mismatches
        );
        assert!(
            report.warnings.is_empty(),
            "{recording} : {:?}",
            report.warnings
        );
        assert!(
            report.checks >= 2,
            "{recording} vérifie au moins l'arrêt et l'écran"
        );
    }
}

#[test]
fn a_nrp_does_not_replay_on_another_application() {
    let report = replay("keys.nrp", "integer.nwa");
    assert!(!report.mismatches.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("n'est pas celle"))
    );
}

#[test]
fn the_nrp_of_an_app_that_writes_the_flash_records_its_exit() {
    let doc = document("flash_rw.nrp");
    let entries = nrp::parse(&doc).unwrap();
    let run = entries
        .iter()
        .find(|e| e["request"]["cmd"] == "run")
        .unwrap();
    assert_eq!(run["expect"]["stop"], "exited");
}

// ---- .nfs ------------------------------------------------------------------------------------------------------------

#[test]
fn the_nfs_fixture_loads_into_the_calculator_files() {
    let doc = document("files.nfs");
    let files = nfs::parse(&doc).unwrap();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["main.py", "scores.py"]);

    let mut session = Session::new();
    load(&mut session, "keys.nwa");
    let imported = call(&mut session, cmd("fs_import").with("nfs", doc));
    assert_eq!(imported["files"], 2);
    let script = call(&mut session, cmd("fs_read").with("name", "main.py"));
    // Le corps d'un script commence par l'octet « importé automatiquement », puis le texte (voir docs/reference/formats/nfs.md).
    let body = json::base64_decode(script["content"].as_str().unwrap()).unwrap();
    assert!(
        String::from_utf8_lossy(&body).contains("bonjour"),
        "{script}"
    );
    // Ils survivent à un état enregistré.
    let dump = call(&mut session, cmd("dump"))["nds"].clone();
    let mut other = Session::new();
    call(&mut other, cmd("restore").with("nds", dump));
    assert_eq!(
        call(&mut other, cmd("fs_list"))["files"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

// ---- le contrôle de tout le dossier --------------------------------------------------------------------------------

#[test]
fn every_fixture_has_a_known_format() {
    let mut seen = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(bin("")).unwrap().flatten() {
        let extension = entry
            .path()
            .extension()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            ["nwa", "nfd", "nds", "nrp", "nfs"].contains(&extension.as_str()),
            "{:?}",
            entry.path()
        );
        seen.insert(extension);
    }
    assert_eq!(seen.len(), 5, "un fichier de chaque format : {seen:?}");
}

#[cfg(feature = "cli")]
mod cli {
    use std::process::Command;

    use super::*;

    fn check(paths: &[String]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_nwasabi-cli"))
            .arg("check")
            .args(paths)
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    }

    #[test]
    fn check_accepts_every_file_of_the_folder() {
        let paths: Vec<String> = std::fs::read_dir(bin(""))
            .unwrap()
            .flatten()
            .map(|e| e.path().display().to_string())
            .collect();
        let (code, output) = check(&paths);
        assert_eq!(code, 0, "{output}");
        assert_eq!(output.matches("ok ").count(), paths.len(), "{output}");
    }

    #[test]
    fn check_refuses_damaged_files_and_names_the_problem() {
        let dir = std::env::temp_dir().join(format!("nwasabi-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "aux_flash.nfd",
            "aux_flash.nds",
            "keys.nrp",
            "files.nfs",
            "keys.nwa",
        ] {
            let mut bytes = read(name);
            bytes.truncate(bytes.len() / 2);
            let damaged = dir.join(name);
            std::fs::write(&damaged, bytes).unwrap();
            let (code, output) = check(&[damaged.display().to_string()]);
            assert_eq!(code, 1, "{name} abîmé doit être refusé : {output}");
            assert!(output.contains("ERREUR"), "{output}");
        }
        // Un fichier d'un autre format que celui de son extension.
        let wrong = dir.join("faux.nfd");
        std::fs::write(&wrong, read("files.nfs")).unwrap();
        assert!(check(&[wrong.display().to_string()]).1.contains(".nfd"));
        let unknown = dir.join("x.xyz");
        std::fs::write(&unknown, b"x").unwrap();
        assert!(
            check(&[unknown.display().to_string()])
                .1
                .contains("extension inconnue")
        );
    }

    #[test]
    fn the_mappings_of_tests_nmk_are_checked_like_their_expected_results() {
        for entry in std::fs::read_dir(format!("{}/tests/nmk", env!("CARGO_MANIFEST_DIR")))
            .unwrap()
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "nmk") {
                continue;
            }
            let wrong = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("error_");
            let (code, output) = check(&[path.display().to_string()]);
            assert_eq!(code, i32::from(wrong), "{} : {output}", path.display());
        }
    }

    #[test]
    fn the_cli_runs_with_each_fixture() {
        let run = |args: &[&str]| {
            let out = Command::new(env!("CARGO_BIN_EXE_nwasabi-cli"))
                .args(args)
                .output()
                .unwrap();
            (
                out.status.code().unwrap(),
                String::from_utf8_lossy(&out.stdout).into_owned(),
            )
        };
        let (nwa, nfd, nds, nrp, nfs) = (
            bin("aux_flash.nwa"),
            bin("aux_flash.nfd"),
            bin("aux_flash.nds"),
            bin("keys.nrp"),
            bin("files.nfs"),
        );
        let s = |p: &PathBuf| p.display().to_string();
        assert_eq!(run(&["run", &s(&nwa), "--flash", &s(&nfd), "--json"]).0, 0);
        assert_eq!(
            run(&["run", "--nds", &s(&nds), "--json"]).0,
            0,
            "l'état suffit : il contient l'application"
        );
        assert_eq!(run(&["replay", &s(&nrp), &s(&bin("keys.nwa"))]).0, 0);
        let (code, out) = run(&[
            "run",
            &s(&bin("keys.nwa")),
            "--nfs",
            &s(&nfs),
            "--frames",
            "2",
            "--json",
        ]);
        assert_eq!(code, 0, "{out}");
    }
}
