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
    let dir = PathBuf::from(std::env::var_os("NWASABI_CORPUS")?);
    std::fs::read_dir(dir)
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
