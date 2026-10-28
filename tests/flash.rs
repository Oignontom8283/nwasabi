//! Les octets placés en flash (`.nfd`), vus par la session : chargés avec l'application ou après, vérifiés,
//! gardés par un état enregistré (`.nds`).

use nwasabi::json::{self, Value, ValueExt};
use nwasabi::session::Session;

fn app(name: &str) -> String {
    let path = format!("{}/tests/apps/bin/{name}.nwa", env!("CARGO_MANIFEST_DIR"));
    json::base64_encode(&std::fs::read(path).unwrap())
}

fn cmd(name: &str) -> Value {
    Value::object().with("cmd", name)
}

fn call(session: &mut Session, request: Value) -> Value {
    let reply = session.handle(&request);
    assert_eq!(reply["ok"], true, "{request} -> {reply}");
    reply
}

fn segment(address: &str, bytes: &[u8]) -> Value {
    Value::object()
        .with("name", "morceau")
        .with("address", address)
        .with("content", json::base64_encode(bytes))
}

fn nfd(segments: Vec<Value>) -> Value {
    Value::object().with("nfd", 1u64).with("segments", segments)
}

fn read(session: &mut Session, address: u32, length: u32) -> String {
    let reply = call(
        session,
        cmd("memory_read")
            .with("address", address)
            .with("length", length),
    );
    reply["hex"].as_str().unwrap().to_owned()
}

fn load(document: Value) -> Value {
    Session::new().handle(
        &cmd("load")
            .with("nwa", app("integer"))
            .with("model", "n0110")
            .with("nfd", document),
    )
}

/// Un document `.nfd` qui demande le slot A, où le firmware n'occupe pas le slot B.
fn slot_a(segments: Vec<Value>) -> Value {
    nfd(segments).with("active_slot", "a")
}

#[test]
fn bytes_are_placed_with_the_application() {
    let mut session = Session::new();
    call(
        &mut session,
        cmd("load")
            .with("nwa", app("integer"))
            .with("model", "n0110")
            .with(
                "nfd",
                slot_a(vec![
                    segment("0x90400000", &[1, 2, 3, 4]),
                    segment("0x90260000", &[9, 8]),
                ]),
            ),
    );
    assert_eq!(read(&mut session, 0x9040_0000, 4), "01020304");
    assert_eq!(read(&mut session, 0x9026_0000, 3), "0908ff");
    // Les segments sont exécutables comme n'importe quelle flash et l'application tourne toujours.
    call(&mut session, cmd("run").with("instructions", 1000u64));
}

#[test]
fn a_document_asking_for_slot_a_moves_the_firmware() {
    let mut session = Session::new();
    call(
        &mut session,
        cmd("load")
            .with("nwa", app("integer"))
            .with("model", "n0110")
            .with("nfd", slot_a(vec![segment("0x90400000", &[7])])),
    );
    let inspected = call(&mut session, cmd("inspect"));
    let slot = inspected["structures"]["slot_info"]["kernel_header"]
        .as_u64()
        .unwrap();
    assert!(
        slot < 0x9040_0000,
        "le firmware doit être dans le slot A : {slot:#x}"
    );
}

#[test]
fn bytes_over_the_firmware_are_refused() {
    // Le N0110 relevé exécute le slot B : le segment à 0x90400000 recouvre le firmware.
    let reply = load(nfd(vec![segment("0x90400000", &[1])]));
    assert_eq!(reply["ok"], false);
    assert!(
        reply["error"].as_str().unwrap().contains("recouvre"),
        "{reply}"
    );
}

#[test]
fn bytes_over_the_application_are_refused() {
    // Avec le slot A, les applications commencent à 0x90170000 sur N0110.
    let reply = load(slot_a(vec![segment("0x90170000", &[1])]));
    assert_eq!(reply["ok"], false);
    assert!(
        reply["error"].as_str().unwrap().contains("recouvre"),
        "{reply}"
    );
}

#[test]
fn invalid_documents_are_explained() {
    for (document, expected) in [
        (Value::object().with("nfs", 1u64), ".nfd"),
        (nfd(vec![segment("0x20000000", &[1])]), "flash externe"),
        (
            nfd(vec![
                segment("0x90400000", &[1, 2]),
                segment("0x90400001", &[3]),
            ]),
            "recouvre",
        ),
        (nfd(vec![segment("pas une adresse", &[1])]), "adresse"),
        (nfd(vec![]).with("active_slot", "c"), "active_slot"),
    ] {
        let reply = load(document.clone());
        assert_eq!(reply["ok"], false, "{document}");
        assert!(
            reply["error"].as_str().unwrap().contains(expected),
            "{document} -> {reply}"
        );
    }
}

#[test]
fn flash_load_adds_segments_and_flash_export_gives_them_back() {
    let mut session = Session::new();
    call(
        &mut session,
        cmd("load")
            .with("nwa", app("integer"))
            .with("model", "n0110")
            .with("nfd", slot_a(vec![segment("0x90400000", &[1, 2])])),
    );
    let reply = call(
        &mut session,
        cmd("flash_load").with("nfd", nfd(vec![segment("0x90500000", &[5, 6, 7])])),
    );
    assert_eq!(reply["segments"], 1);
    assert_eq!(read(&mut session, 0x9050_0000, 3), "050607");
    let exported = call(&mut session, cmd("flash_export"));
    assert_eq!(exported["nfd"]["segments"].as_array().unwrap().len(), 2);
}

#[test]
fn a_dump_keeps_the_flash_segments() {
    let mut session = Session::new();
    call(
        &mut session,
        cmd("load")
            .with("nwa", app("integer"))
            .with("model", "n0110")
            .with(
                "nfd",
                slot_a(vec![segment("0x90400000", &[0xde, 0xad, 0xbe, 0xef])]),
            ),
    );
    call(&mut session, cmd("run").with("instructions", 5000u64));
    let dump = call(&mut session, cmd("dump"))["nds"].clone();
    assert!(dump["app"]["flash"]["segments"].is_array(), "{dump}");

    let mut other = Session::new();
    call(&mut other, cmd("restore").with("nds", dump));
    assert_eq!(read(&mut other, 0x9040_0000, 4), "deadbeef");
}
