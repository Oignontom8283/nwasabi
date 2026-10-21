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

