//! Les formats `.nfs` (fichiers de la calculatrice) et `.nds` (état d'une émulation), vus par la session.

use nwasabi::json::{self, Value, ValueExt};
use nwasabi::session::Session;

fn app(name: &str) -> String {
    let path = format!("{}/tests/apps/bin/{name}.nwa", env!("CARGO_MANIFEST_DIR"));
    json::base64_encode(&std::fs::read(path).unwrap())
}

fn call(session: &mut Session, request: Value) -> Value {
    let reply = session.handle(&request);
    assert_eq!(reply["ok"], true, "{request} -> {reply}");
    reply
}

fn cmd(name: &str) -> Value {
    Value::object().with("cmd", name)
}

fn loaded(model: &str, name: &str) -> Session {
    let mut session = Session::new();
    call(
        &mut session,
        cmd("load")
            .with("nwa_base64", app(name))
            .with("model", model),
    );
    session
}

fn write(session: &mut Session, name: &str, text: &str) {
    call(
        session,
        cmd("fs_write").with("name", name).with("text", text),
    );
}

fn files(session: &mut Session) -> Vec<(String, usize)> {
    let reply = call(session, cmd("fs_list"));
    reply["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["name"].as_str().unwrap().to_owned(),
                f["size"].as_u64().unwrap() as usize,
            )
        })
        .collect()
}

// ---- .nfs --------------------------------------------------------------------------------------

#[test]
fn nfs_round_trips_between_models() {
    let mut a = loaded("n0110", "integer");
    write(&mut a, "main.py", "print(1)");
    write(&mut a, "second.py", "x = 2\nprint(x)");
    let exported = call(&mut a, cmd("fs_export"));
    let doc = exported["nfs"].clone();
    assert_eq!(doc["nfs"], 1);
    let before = files(&mut a);
    assert!(before.len() >= 2);

    let mut b = loaded("n0120", "integer");
    write(&mut b, "old.py", "stale");
    let imported = call(&mut b, cmd("fs_import").with("nfs", doc.clone()));
    assert_eq!(imported["files"].as_u64().unwrap() as usize, before.len());
    assert_eq!(files(&mut b), before);
    let read = call(&mut b, cmd("fs_read").with("name", "main.py"));
    assert_eq!(read["base64"], json::base64_encode(b"print(1)"));
}

#[test]
fn nfs_import_can_keep_existing_files() {
    let mut a = loaded("n0110", "integer");
    write(&mut a, "a.py", "1");
    let doc = call(&mut a, cmd("fs_export"))["nfs"].clone();
    let mut b = loaded("n0110", "integer");
    write(&mut b, "keep.py", "2");
    call(
        &mut b,
        cmd("fs_import").with("nfs", doc).with("replace", false),
    );
    let names: Vec<_> = files(&mut b).into_iter().map(|f| f.0).collect();
    assert!(names.contains(&"a.py".to_owned()) && names.contains(&"keep.py".to_owned()));
}

#[test]
fn a_failed_nfs_import_changes_nothing() {
    let mut a = loaded("n0110", "integer");
    write(&mut a, "keep.py", "kept");
    let before = files(&mut a);
    // Plus gros que le tampon : l'import échoue à la fin, et tout doit être défait.
    let big = json::base64_encode(&vec![b'x'; 40_000]);
    let doc = Value::object().with("nfs", 1).with(
        "files",
        vec![
            Value::object().with("name", "a.py").with("base64", "eA=="),
            Value::object()
                .with("name", "big1.py")
                .with("base64", big.as_str()),
            Value::object()
                .with("name", "big2.py")
                .with("base64", big.as_str()),
        ],
    );
    let reply = a.handle(&cmd("fs_import").with("nfs", doc));
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(files(&mut a), before);
}

#[test]
fn invalid_nfs_documents_are_refused_with_the_location() {
    let mut a = loaded("n0110", "integer");
    let try_doc = |a: &mut Session, doc: Value| {
        let reply = a.handle(&cmd("fs_import").with("nfs", doc));
        assert_eq!(reply["ok"], false);
        reply["error"].as_str().unwrap().to_owned()
    };
    assert!(try_doc(&mut a, json::parse("{}").unwrap()).contains("nfs"));
    assert!(try_doc(&mut a, json::parse(r#"{"nfs":2,"files":[]}"#).unwrap()).contains("version"));
    let e = try_doc(
        &mut a,
        json::parse(r#"{"nfs":1,"files":[{"name":"a.py","base64":"eA=="},{"name":"sans_extension","base64":""}]}"#).unwrap(),
    );
    assert!(e.contains("files[1]"), "{e}");
    let e = try_doc(
        &mut a,
        json::parse(r#"{"nfs":1,"files":[{"name":"a.py","base64":"???"}]}"#).unwrap(),
    );
    assert!(e.contains("files[0]") && e.contains("base64"), "{e}");
    let e = try_doc(
        &mut a,
        json::parse(
            r#"{"nfs":1,"files":[{"name":"a.py","base64":""},{"name":"a.py","base64":""}]}"#,
        )
        .unwrap(),
    );
    assert!(e.contains("double"), "{e}");
}

// ---- .nds --------------------------------------------------------------------------------------

fn run_frames(session: &mut Session, frames: u64) -> Value {
    call(session, cmd("run").with("frames", frames))
}

fn screen_crc(session: &mut Session) -> String {
    call(session, cmd("screen"))["crc32"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn press(session: &mut Session, key: &str) {
    call(session, cmd("key").with("key", key).with("action", "press"));
}

#[test]
fn a_dump_resumes_exactly_where_the_emulation_stopped() {
    let mut original = loaded("n0110", "keys");
    run_frames(&mut original, 3);
    press(&mut original, "up");
    run_frames(&mut original, 2);
    write(&mut original, "notes.py", "print('hi')");

    let dump = call(&mut original, cmd("dump"))["nds"].clone();
    assert_eq!(dump["nds"], 1);
    for part in ["app", "memory", "fs", "cpu", "kernel", "device"] {
        assert!(dump.get(part).is_some(), "{part}");
    }
    // Seules les pages de mémoire qui diffèrent d'un appareil neuf sont enregistrées.
    assert!(
        dump.to_string().len() < 200_000,
        "{} octets",
        dump.to_string().len()
    );

    let mut copy = Session::new();
    let reply = call(&mut copy, cmd("restore").with("nds", dump));
    assert_eq!(
        reply["parts"],
        json::parse(r#"["app","memory","fs","state"]"#).unwrap()
    );
    assert_eq!(screen_crc(&mut copy), screen_crc(&mut original));
    assert_eq!(files(&mut copy), files(&mut original));
    assert_eq!(call(&mut copy, cmd("cpu")), call(&mut original, cmd("cpu")));
    assert_eq!(
        call(&mut copy, cmd("time")),
        call(&mut original, cmd("time"))
    );

    // Les deux exécutions continuent de la même façon, jusqu'au dernier registre.
    for session in [&mut original, &mut copy] {
        call(
            session,
            cmd("key").with("key", "up").with("action", "release"),
        );
        press(session, "left");
    }
    let (a, b) = (run_frames(&mut original, 6), run_frames(&mut copy, 6));
    assert_eq!(a, b);
    assert_eq!(screen_crc(&mut copy), screen_crc(&mut original));
    assert_eq!(call(&mut copy, cmd("cpu")), call(&mut original, cmd("cpu")));
}

#[test]
fn memory_edits_and_settings_survive_a_dump() {
    let mut a = loaded("n0115", "integer");
    call(
        &mut a,
        cmd("settings_set")
            .with("name", "device_name")
            .with("value", "Nwasabi"),
    );
    call(
        &mut a,
        cmd("settings_set")
            .with("name", "brightness")
            .with("value", 32),
    );
    // Une écriture dans la RAM de l'application.
    let usage = call(&mut a, cmd("usage"));
    let address = usage["heap"]["start"].as_u64().unwrap();
    call(
        &mut a,
        cmd("memory_write")
            .with("address", address)
            .with("hex", "cafebabe"),
    );

    let dump = call(&mut a, cmd("dump"))["nds"].clone();
    let mut b = Session::new();
    call(&mut b, cmd("restore").with("nds", dump));
    let read = call(
        &mut b,
        cmd("memory_read")
            .with("address", address)
            .with("length", 4),
    );
    assert_eq!(read["hex"], "cafebabe");
    let settings = call(&mut b, cmd("settings_get"));
    assert_eq!(settings["device_name"], "Nwasabi");
    assert_eq!(settings["brightness"], 32);
    assert_eq!(settings["model"], "N0115");
}

#[test]
fn unchecked_parts_are_left_out_and_the_current_device_is_kept() {
    let mut source = loaded("n0110", "keys");
    run_frames(&mut source, 2);
    write(&mut source, "dumped.py", "1");
    let dump = call(
        &mut source,
        cmd("dump").with("parts", json::parse(r#"["memory"]"#).unwrap()),
    )["nds"]
        .clone();
    for part in ["app", "fs", "cpu", "kernel"] {
        assert!(dump.get(part).is_none(), "{part}");
    }
    assert!(dump.get("memory").is_some());

    // Un appareil qui a déjà l'application et ses propres fichiers : ils restent.
    let mut target = loaded("n0110", "keys");
    write(&mut target, "mine.py", "2");
    let reply = call(&mut target, cmd("restore").with("nds", dump));
    assert_eq!(reply["parts"], json::parse(r#"["memory"]"#).unwrap());
    let names: Vec<_> = files(&mut target).into_iter().map(|f| f.0).collect();
    assert!(names.contains(&"mine.py".to_owned()), "{names:?}");
    assert!(!names.contains(&"dumped.py".to_owned()), "{names:?}");
}

#[test]
fn invalid_nds_documents_are_refused_and_keep_the_current_device() {
    let mut session = loaded("n0110", "integer");
    let before = call(&mut session, cmd("cpu"));
    for (doc, expected) in [
        (r#"{}"#, "nds"),
        (r#"{"nds":9}"#, "version"),
        (r#"{"nds":1}"#, "device"),
        (
            r#"{"nds":1,"device":{"model":"n9999","firmware":"24"}}"#,
            "modèle",
        ),
        (
            r#"{"nds":1,"device":{"model":"n0110","firmware":"1"}}"#,
            "firmware",
        ),
    ] {
        let reply = session.handle(&cmd("restore").with("nds", json::parse(doc).unwrap()));
        assert_eq!(reply["ok"], false, "{doc}");
        assert!(
            reply["error"].as_str().unwrap().contains(expected),
            "{doc} -> {reply}"
        );
    }
    assert_eq!(call(&mut session, cmd("cpu")), before);
}
