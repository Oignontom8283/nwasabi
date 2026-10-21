//! Garde-fou : `src/cpu/` doit rester extractible dans un crate autonome.

use std::fs;
use std::path::Path;

fn visit(dir: &Path, offenders: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            visit(&path, offenders);
        } else if path.extension().is_some_and(|e| e == "rs") {
            for (n, line) in fs::read_to_string(&path).unwrap().lines().enumerate() {
                let t = line.trim_start();
                let code = t.split("//").next().unwrap_or("");
                if code.contains("crate::") || code.contains("nwasabi::") {
                    offenders.push(format!("{}:{}: {}", path.display(), n + 1, t));
                }
            }
        }
    }
}

#[test]
fn cpu_module_has_no_crate_dependencies() {
    let mut offenders = Vec::new();
    visit(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cpu"),
        &mut offenders,
    );
    assert!(
        offenders.is_empty(),
        "src/cpu/ ne doit pas dépendre du reste du crate :\n{}",
        offenders.join("\n")
    );
}
