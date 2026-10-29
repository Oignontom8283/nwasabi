//! Un vrai GDB contre le serveur GDB de Nwasabi, avec l'ELF de l'application : points d'arrêt sur une fonction, pile, variables, lignes
//! de source, `finish`, `watch`. Ignoré sans GDB (variable `NWASABI_GDB`, sinon `gdb-multiarch` ou `arm-none-eabi-gdb` dans le PATH ;
//! `NWASABI_GDB_DATA` donne son `--data-directory` si besoin).

use std::net::TcpListener;
use std::process::Command;

use nwasabi::json::{self, Value, ValueExt};
use nwasabi::session::Session;

fn gdb() -> Option<String> {
    if let Ok(path) = std::env::var("NWASABI_GDB") {
        return Some(path);
    }
    ["gdb-multiarch", "arm-none-eabi-gdb"]
        .into_iter()
        .find_map(|name| {
            Command::new(name)
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|_| name.to_owned())
        })
}

#[test]
fn a_real_gdb_debugs_the_source_of_an_application() {
    let Some(gdb) = gdb() else {
        eprintln!("GDB absent : test ignoré");
        return;
    };
    let dir = format!("{}/tests/apps", env!("CARGO_MANIFEST_DIR"));
    let nwa = std::fs::read(format!("{dir}/bin/debugme.nwa")).unwrap();
    let mut session = Session::new();
    let loaded = session.handle(
        &Value::object()
            .with("cmd", "load")
            .with("nwa", json::base64_encode(&nwa)),
    );
    assert_eq!(loaded["ok"], true);
    let elf = session.handle(&Value::object().with("cmd", "export_elf"));
    let work = std::env::temp_dir().join(format!("nwasabi-gdb-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let elf_path = work.join("debugme.elf");
    std::fs::write(
        &elf_path,
        json::base64_decode(elf["elf"].as_str().unwrap()).unwrap(),
    )
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let _ = nwasabi::gdb::serve_connection(&mut session, stream);
    });

    let script = work.join("script.gdb");
    std::fs::write(
        &script,
        format!(
            "set pagination off\nset confirm off\nfile {elf}\ndirectory {dir}\ntarget remote 127.0.0.1:{port}\n\
             break factorial\ncontinue\nbt\ninfo args\ndelete 1\nbreak fill\ncontinue\nprint *shape\nprint shape->flags\n\
             list\nnext\nnext\nnext\nnext\nprint total\nup\ninfo locals\ndown\nwatch total\ncontinue\nprint total\ndelete\nfinish\n\
             print global_counter\nset var global_counter = 99\nprint global_counter\nkill\n",
            elf = elf_path.display(),
        ),
    )
    .unwrap();
    let mut command = Command::new(&gdb);
    if let Ok(data) = std::env::var("NWASABI_GDB_DATA") {
        command.arg(format!("--data-directory={data}"));
    }
    let output = command
        .args(["-nx", "-q", "-batch", "-x"])
        .arg(&script)
        .output()
        .expect("lancement de GDB");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    drop(server);

    for expected in [
        "Breakpoint 1 at 0x",
        "src/debugme.c, line 20",
        "factorial (n=5)",
        "#1  0x",
        "in main () at src/debugme.c:40",
        "n = 5",
        "corner = {x = 1, y = 2}",
        "name = 0x",
        "\"box\"",
        "color = GREEN",
        "Hardware watchpoint",
        "Old value = 10",
        "New value = 21",
        "Value returned is $",
        "= 33",
        "$",
        " = 7",
        " = 99",
    ] {
        assert!(
            text.contains(expected),
            "« {expected} » absent de la sortie de GDB :\n{text}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
