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
    assert!(stdout(&help).contains("session --stdio"));
    assert_eq!(cli(&[]).status.code(), Some(2));
    assert_eq!(cli(&["nimporte"]).status.code(), Some(2));
    let missing = cli(&["run"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("fichier .nwa manquant"));
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

