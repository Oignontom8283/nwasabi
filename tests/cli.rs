//! Le binaire `nwasabi-cli`, lancé comme le ferait un script.

#![cfg(feature = "cli")]

mod support;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use nwasabi::json::{self, Value};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nwasabi-cli"))
        .args(args)
        .output()
        .expect("lancement de nwasabi-cli")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Dossier de travail propre à un test.
fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nwasabi-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn corpus_file(prefix: &str) -> Option<PathBuf> {
    std::fs::read_dir(support::corpus_dir()?)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(prefix))
        })
}

#[test]
fn usage_and_exit_codes() {
    let help = cli(&["help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(stdout(&help).contains("session"));
    assert_eq!(cli(&[]).status.code(), Some(2));
    assert_eq!(cli(&["nimporte"]).status.code(), Some(2));
    // Un argument obligatoire manquant est un mauvais usage (code 2), pas une erreur d'exécution.
    assert_eq!(cli(&["run"]).status.code(), Some(2));
    assert_eq!(
        cli(&["run", "a.nwa", "--model", "n9999"]).status.code(),
        Some(2)
    );
    assert_eq!(cli(&["run", "/inexistant.nwa"]).status.code(), Some(1));
}

#[test]
fn keys_are_listed_as_json() {
    let out = cli(&["keys", "--json"]);
    let v = json::parse(stdout(&out).trim()).unwrap();
    let keys = v.get("keys").and_then(Value::as_array).unwrap();
    assert_eq!(keys.len(), 46);
    assert_eq!(keys[4].get("name").and_then(Value::as_str), Some("ok"));
}

#[test]
fn a_filesystem_image_can_be_edited_from_the_command_line() {
    let image = workdir("fs").join("storage.bin");
    let image = image.to_str().unwrap();
    assert!(
        cli(&["fs", "--image", image, "write", "a.py", "--text", "hello"])
            .status
            .success()
    );
    assert!(
        cli(&["fs", "--image", image, "write", "b.txt", "--text", "xyz"])
            .status
            .success()
    );
    let list = cli(&["fs", "--image", image, "list", "--json"]);
    let v = json::parse(stdout(&list).trim()).unwrap();
    assert_eq!(v.get("files").and_then(Value::as_array).unwrap().len(), 2);
    assert_eq!(std::fs::metadata(image).unwrap().len(), 0xa800);

    let read = cli(&["fs", "--image", image, "read", "a.py"]);
    assert_eq!(read.stdout, b"hello");
    assert!(
        cli(&["fs", "--image", image, "rename", "a.py", "c.py"])
            .status
            .success()
    );
    assert!(
        cli(&["fs", "--image", image, "remove", "b.txt"])
            .status
            .success()
    );
    let stats = cli(&["fs", "--image", image, "stats", "--json"]);
    let v = json::parse(stdout(&stats).trim()).unwrap();
    assert_eq!(v.get("records").and_then(Value::as_i64), Some(1));

    let bad = cli(&["fs", "--image", image, "remove", "absent.py"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("introuvable"));
}

#[test]
fn session_talks_json_lines_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nwasabi-cli"))
        .args(["session", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            concat!(
                "{\"cmd\":\"new\",\"model\":\"n0120\",\"id\":1}\n",
                "\n",
                "{\"cmd\":\"settings_set\",\"name\":\"device_name\",\"value\":\"Test\"}\n",
                "{\"cmd\":\"inspect\"}\n",
                "pas du json\n",
                "{\"cmd\":\"quit\"}\n",
                "{\"cmd\":\"help\"}\n",
            )
            .as_bytes(),
        )
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let lines: Vec<String> = stdout(&out).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 4, "« quit » arrête la session : {lines:?}");
    let first = json::parse(&lines[0]).unwrap();
    assert_eq!(first.get("id").and_then(Value::as_i64), Some(1));
    assert_eq!(first.get("model").and_then(Value::as_str), Some("N0120"));
    let inspect = json::parse(&lines[2]).unwrap();
    let st = inspect.get("structures").unwrap();
    assert_eq!(st.get("device_name").and_then(Value::as_str), Some("Test"));
    assert_eq!(
        st.get("sram_base").and_then(Value::as_i64),
        Some(0x2400_0000)
    );
    assert!(lines[3].contains("\"ok\":false"));
}

#[test]
fn run_is_deterministic_and_matches_the_reference_screen() {
    let Some(app) = corpus_file("mandelbrot") else {
        return support::skip_without_corpus();
    };
    let app = app.to_str().unwrap();
    let args = ["run", app, "--instructions", "100000000", "--json"];
    let (a, b) = (cli(&args), cli(&args));
    assert_eq!(a.status.code(), Some(0));
    assert_eq!(a.stdout, b.stdout, "deux exécutions, mêmes octets");
    let v = json::parse(stdout(&a).trim()).unwrap();
    // Même empreinte que tests/golden/screens.tsv.
    assert_eq!(
        v.get("screen_crc32").and_then(Value::as_str),
        Some("8876b506")
    );

    let png = workdir("shot").join("mandelbrot.png");
    let shot = cli(&[
        "run",
        app,
        "--instructions",
        "100000000",
        "--screenshot",
        png.to_str().unwrap(),
    ]);
    assert!(shot.status.success());
    assert_eq!(&std::fs::read(&png).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn the_frequency_option_scales_the_virtual_time() {
    let Some(app) = corpus_file("mandelbrot") else {
        return support::skip_without_corpus();
    };
    let app = app.to_str().unwrap();
    let time = |extra: &[&str]| {
        let mut args = vec!["run", app, "--instructions", "19200000", "--json"];
        args.extend_from_slice(extra);
        let out = cli(&args);
        assert_eq!(out.status.code(), Some(0));
        let v = json::parse(stdout(&out).trim()).unwrap();
        v.get("time_ms").and_then(Value::as_u64).unwrap()
    };
    // 19,2 millions d'instructions : 100 ms à 192 MHz (la fréquence du N0110), 200 ms à 96 MHz.
    assert_eq!(time(&[]), 100);
    assert_eq!(time(&["--mhz", "96"]), 200);
}

#[test]
fn link_and_inspect_describe_an_application() {
    let Some(app) = corpus_file("conway-1.2.3") else {
        return support::skip_without_corpus();
    };
    let app = app.to_str().unwrap();
    let linked = cli(&["link", app, "--map", "--json"]);
    let v = json::parse(stdout(&linked).trim()).unwrap();
    assert_eq!(v.get("name").and_then(Value::as_str), Some("Conway"));
    let symbols = v.get("symbols").and_then(Value::as_array).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.get("name").and_then(Value::as_str) == Some("_start"))
    );
    assert!(
        !v.get("sections")
            .and_then(Value::as_array)
            .unwrap()
            .is_empty()
    );

    let inspect = cli(&["inspect", app]);
    let v = json::parse(stdout(&inspect).trim()).unwrap();
    assert_eq!(
        v.get("app")
            .and_then(|a| a.get("name"))
            .and_then(Value::as_str),
        Some("Conway")
    );
}

#[test]
fn survey_reports_every_application() {
    let Some(app) = corpus_file("sierpinski") else {
        return support::skip_without_corpus();
    };
    let out = cli(&[
        "survey",
        app.to_str().unwrap(),
        "--instructions",
        "2000000",
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(0));
    let v = json::parse(stdout(&out).trim()).unwrap();
    let apps = v.get("apps").and_then(Value::as_array).unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(
        apps[0].get("app").and_then(Value::as_str),
        Some("sierpinski.nwa")
    );
}

fn nmk_fixture(name: &str) -> String {
    format!("{}/tests/nmk/{name}.nmk", env!("CARGO_MANIFEST_DIR"))
}

fn app_fixture() -> String {
    format!("{}/tests/apps/bin/integer.nwa", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn keys_prints_a_mapping_completed_by_the_default() {
    let text = cli(&["keys", "--nmk", &nmk_fixture("wasd")]);
    assert!(text.status.success());
    let text = stdout(&text);
    assert!(text.contains("mapping : Jeu"), "{text}");
    assert!(
        text.contains("KeyQ") && text.contains("shift + alpha"),
        "{text}"
    );
    assert!(
        text.contains("swipe_right") && text.contains("tant que le doigt reste posé"),
        "{text}"
    );

    let json = cli(&["keys", "--nmk", &nmk_fixture("wasd"), "--json"]);
    let v = json::parse(stdout(&json).trim()).unwrap();
    assert_eq!(v["keyboard"]["KeyW"][0], "up");
    assert_eq!(v["touch"]["long_press_ms"], 400);
}

#[test]
fn an_invalid_mapping_is_reported_with_its_location() {
    let out = cli(&["keys", "--nmk", &nmk_fixture("error_unknown_calc_key")]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("keyboard.KeyW") && err.contains("haut"),
        "{err}"
    );
}

#[test]
fn press_accepts_a_host_key_through_the_mapping() {
    let app = app_fixture();
    let ok = cli(&[
        "run",
        &app,
        "--nmk",
        &nmk_fixture("wasd"),
        "--press",
        "KeyQ",
        "--press",
        "ok",
    ]);
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let unknown = cli(&[
        "run",
        &app,
        "--nmk",
        &nmk_fixture("wasd"),
        "--press",
        "KeyZ",
    ]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("KeyZ"));
    // Sans mapping, un nom de touche de l'ordinateur reste une touche inconnue.
    let without = cli(&["run", &app, "--press", "KeyQ"]);
    assert_eq!(without.status.code(), Some(1));
}

/// Empreinte de l'écran de `keys.nwa` après quelques images, avec les arguments donnés en plus.
fn keys_screen(extra: &[&str]) -> String {
    let app = format!("{}/tests/apps/bin/keys.nwa", env!("CARGO_MANIFEST_DIR"));
    let mut args = vec!["run", &app, "--frames", "4", "--json"];
    args.extend_from_slice(extra);
    let out = cli(&args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = json::parse(stdout(&out).trim()).unwrap();
    v["screen_crc32"].as_str().unwrap().to_owned()
}

#[test]
fn a_mapped_host_key_reaches_the_application() {
    let nmk = nmk_fixture("wasd");
    let idle = keys_screen(&[]);
    let up = keys_screen(&["--press", "up"]);
    assert_ne!(idle, up, "la touche haut change l'écran");
    // Position W du clavier → haut, avec le mapping de jeu.
    assert_eq!(keys_screen(&["--nmk", &nmk, "--press", "KeyW"]), up);
    assert_ne!(keys_screen(&["--nmk", &nmk, "--press", "KeyA"]), up);
    // Une combinaison (shift + alpha) n'est aucune des touches de l'application : écran noir comme au repos.
    assert_eq!(keys_screen(&["--nmk", &nmk, "--press", "KeyQ"]), idle);
}

#[test]
fn nfs_and_nds_files_go_through_the_cli() {
    let dir = workdir("formats");
    let app = format!("{}/tests/apps/bin/keys.nwa", env!("CARGO_MANIFEST_DIR"));
    let (nfs, nds) = (dir.join("files.nfs"), dir.join("state.nds"));
    std::fs::write(
        &nfs,
        r#"{"nfs":1,"files":[{"name":"hello.py","content":"cHJpbnQoMSk="}]}"#,
    )
    .unwrap();

    // Charge des fichiers, exécute, enregistre l'état puis les fichiers.
    let saved_nfs = dir.join("saved.nfs");
    let out = cli(&[
        "run",
        &app,
        "--frames",
        "3",
        "--nfs",
        nfs.to_str().unwrap(),
        "--save-nds",
        nds.to_str().unwrap(),
        "--save-nfs",
        saved_nfs.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let saved: Value = json::parse(&std::fs::read_to_string(&saved_nfs).unwrap()).unwrap();
    assert_eq!(saved["files"][0]["name"], "hello.py");

    // Reprend l'état : sans application en argument, et le temps continue où il s'était arrêté.
    let first = json::parse(stdout(&out).trim()).unwrap();
    let resumed = cli(&[
        "run",
        "--nds",
        nds.to_str().unwrap(),
        "--frames",
        "2",
        "--json",
    ]);
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    let second = json::parse(stdout(&resumed).trim()).unwrap();
    assert!(second["time_ms"].as_u64() > first["time_ms"].as_u64());

    // Sans --nds ni application : erreur d'usage.
    assert_eq!(cli(&["run"]).status.code(), Some(2));
}

#[test]
fn fs_export_and_import_use_the_nfs_format() {
    let dir = workdir("fs-nfs");
    let image = dir.join("image.bin");
    let image = image.to_str().unwrap();
    assert!(
        cli(&[
            "fs", "--image", image, "write", "a.py", "--text", "print(1)"
        ])
        .status
        .success()
    );
    let nfs = dir.join("all.nfs");
    assert!(
        cli(&["fs", "--image", image, "export", nfs.to_str().unwrap()])
            .status
            .success()
    );
    let document: Value = json::parse(&std::fs::read_to_string(&nfs).unwrap()).unwrap();
    assert_eq!(document["files"][0]["name"], "a.py");

    let other = dir.join("other.bin");
    let other = other.to_str().unwrap();
    assert!(
        cli(&["fs", "--image", other, "write", "old.py", "--text", "x"])
            .status
            .success()
    );
    assert!(
        cli(&["fs", "--image", other, "import", nfs.to_str().unwrap()])
            .status
            .success()
    );
    let list = cli(&["fs", "--image", other, "list", "--json"]);
    let names = stdout(&list);
    assert!(
        names.contains("a.py") && !names.contains("old.py"),
        "{names}"
    );
    assert!(
        cli(&[
            "fs",
            "--image",
            other,
            "import",
            nfs.to_str().unwrap(),
            "--keep"
        ])
        .status
        .success()
    );
}

// ---- enregistrer et rejouer -------------------------------------------------------------------------------------

fn test_app(name: &str) -> String {
    format!("{}/tests/apps/bin/{name}.nwa", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn a_run_is_recorded_and_replays_identically() {
    let dir = workdir("replay-run");
    let recording = dir.join("keys.nrp");
    let recorded = cli(&[
        "run",
        &test_app("keys"),
        "--press",
        "ok",
        "--frames",
        "6",
        "--record",
        recording.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        recorded.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&recorded.stderr)
    );

    let replayed = cli(&["replay", recording.to_str().unwrap(), &test_app("keys")]);
    assert_eq!(replayed.status.code(), Some(0), "{}", stdout(&replayed));
    assert!(
        stdout(&replayed).contains("0 différence"),
        "{}",
        stdout(&replayed)
    );

    // Une autre application ne donne pas la même chose : code 4, avec l'endroit de la différence.
    let other = cli(&[
        "replay",
        recording.to_str().unwrap(),
        &test_app("integer"),
        "--json",
    ]);
    assert_eq!(other.status.code(), Some(4), "{}", stdout(&other));
    let report = json::parse(&stdout(&other)).unwrap();
    assert!(!report["mismatches"].as_array().unwrap().is_empty());
    assert!(
        report["warnings"][0]
            .as_str()
            .unwrap()
            .contains("n'est pas celle")
    );
}

#[test]
fn a_session_is_recorded_with_its_keys_and_time() {
    let dir = workdir("replay-session");
    let recording = dir.join("session.nrp");
    let app = std::fs::read(test_app("keys")).unwrap();
    let nwa = json::base64_encode(&app);
    let script = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n",
        format_args!(r#"{{"cmd":"load","nwa":"{nwa}"}}"#),
        r#"{"cmd":"run","frames":2}"#,
        r#"{"cmd":"key","key":"left","action":"press"}"#,
        r#"{"cmd":"run","frames":4}"#,
        r#"{"cmd":"cpu"}"#,
        r#"{"cmd":"screen"}"#,
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_nwasabi-cli"))
        .args(["session", "--record", recording.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());

    let document = json::parse(&std::fs::read_to_string(&recording).unwrap()).unwrap();
    let entries = document["entries"].as_array().unwrap();
    let commands: Vec<&str> = entries
        .iter()
        .map(|e| e["request"]["cmd"].as_str().unwrap())
        .collect();
    assert_eq!(
        commands,
        ["run", "key", "run", "screen"],
        "la lecture du processeur n'est pas enregistrée"
    );

    let replayed = cli(&["replay", recording.to_str().unwrap(), &test_app("keys")]);
    assert_eq!(replayed.status.code(), Some(0), "{}", stdout(&replayed));
}

#[test]
fn a_tampered_recording_is_detected() {
    let dir = workdir("replay-tamper");
    let recording = dir.join("keys.nrp");
    cli(&[
        "run",
        &test_app("keys"),
        "--frames",
        "3",
        "--record",
        recording.to_str().unwrap(),
    ]);
    let text = std::fs::read_to_string(&recording).unwrap();
    let doc = json::parse(&text).unwrap();
    let crc = doc["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|e| e["expect"]["crc32"].as_str().map(str::to_owned))
        .unwrap();
    std::fs::write(&recording, text.replace(&crc, "deadbeef")).unwrap();
    let replayed = cli(&["replay", recording.to_str().unwrap(), &test_app("keys")]);
    assert_eq!(replayed.status.code(), Some(4));
    assert!(
        stdout(&replayed).contains("deadbeef"),
        "{}",
        stdout(&replayed)
    );
}

#[test]
fn an_invalid_recording_is_refused() {
    let dir = workdir("replay-invalid");
    let bad = dir.join("bad.nrp");
    std::fs::write(&bad, r#"{"nfs":1}"#).unwrap();
    let out = cli(&["replay", bad.to_str().unwrap(), &test_app("keys")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains(".nrp"));
}
