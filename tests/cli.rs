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

