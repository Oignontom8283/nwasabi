//! Des applications de test qui lisent et écrivent la flash comme le fait KhiCAS (voir `tests/apps/src/`) : données auxiliaires
//! (`.nfd`), exécution de code placé en flash, effacement et programmation, état de shift et d'alpha ; et ce qu'en gardent
//! les états enregistrés (`.nds`).

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

/// Les données que `aux_flash` attend : le programme à 0x90400000 (signature puis `movs r0, #42 ; bx lr`) et la table à 0x90260000.
fn aux_data() -> Value {
    let mut program = b"NWAUX\0\0\0".to_vec();
    program.extend_from_slice(&[0x2a, 0x20, 0x70, 0x47]); // movs r0, #42 ; bx lr
    let table: Vec<u8> = (0..16u32)
        .flat_map(|n| (0x1000 + 7 * n).to_le_bytes())
        .collect();
    segments(&[(0x9040_0000, program), (0x9026_0000, table)])
}

fn segments(list: &[(u32, Vec<u8>)]) -> Value {
    let items: Vec<Value> = list
        .iter()
        .map(|(address, bytes)| {
            Value::object()
                .with("address", *address)
                .with("content", json::base64_encode(bytes))
        })
        .collect();
    Value::object()
        .with("nfd", 1u64)
        .with("active_slot", "a")
        .with("segments", items)
}

fn load(name: &str, nfd: Option<Value>) -> Session {
    let mut session = Session::new();
    let mut request = cmd("load").with("nwa", app(name)).with("model", "n0110");
    if let Some(nfd) = nfd {
        request.set("nfd", nfd);
    }
    call(&mut session, request);
    session
}

/// Exécute jusqu'à la fin de l'application ; rend son code de sortie (0 : toutes les vérifications ont réussi).
fn exit_code(session: &mut Session) -> i64 {
    let reply = call(session, cmd("run").with("instructions", 5_000_000u64));
    assert_eq!(reply["stop"]["kind"], "exited", "{reply}");
    reply["stop"]["code"].as_i64().unwrap()
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

#[test]
fn an_app_reads_and_runs_its_auxiliary_flash_data() {
    let mut session = load("aux_flash", Some(aux_data()));
    assert_eq!(exit_code(&mut session), 0);
}

#[test]
fn without_its_data_the_app_fails_on_its_first_check() {
    // La flash est vide : l'application ne trouve ni la signature ni la table, puis appelle du code qui n'existe pas (0xffff).
    let mut session = load("aux_flash", None);
    let reply = call(&mut session, cmd("run").with("instructions", 5_000_000u64));
    let exited_ok = reply["stop"]["kind"] == "exited" && reply["stop"]["code"] == 0;
    assert!(!exited_ok, "{reply}");
    assert_eq!(
        reply["stop"]["fault"], "usage",
        "instruction indéfinie dans la flash vide : {reply}"
    );
}

#[test]
fn wrong_data_is_noticed() {
    let mut data = aux_data();
    // Le code de la routine devient `movs r0, #7` : la dernière vérification échoue.
    let program = json::base64_encode(&[
        b'N', b'W', b'A', b'U', b'X', 0, 0, 0, 0x07, 0x20, 0x70, 0x47,
    ]);
    data["segments"][0].set("content", program);
    let mut session = load("aux_flash", Some(data));
    assert_ne!(exit_code(&mut session), 0);
}

#[test]
fn auxiliary_data_survives_a_dump_and_restore() {
    let mut session = load("aux_flash", Some(aux_data()));
    let dump = call(
        &mut session,
        cmd("dump").with("parts", vec!["app", "memory", "state"]),
    )["nds"]
        .clone();
    assert!(
        dump["app"]["flash"]["segments"].is_array(),
        "les données sont dans la partie app"
    );

    // Un appareil neuf, sans rien : l'état rétablit l'application et ses données.
    let mut other = Session::new();
    call(&mut other, cmd("restore").with("nds", dump));
    assert_eq!(read(&mut other, 0x9040_0000, 5), "4e57415558");
    assert_eq!(exit_code(&mut other), 0);
}

#[test]
fn flash_can_be_erased_and_programmed_but_the_firmware_is_protected() {
    let mut session = load("flash_rw", None);
    assert_eq!(exit_code(&mut session), 0);
    // Le dernier mot écrit est resté en flash.
    assert_eq!(
        read(&mut session, 0x9070_0000, 8),
        "0df0feca".to_owned() + "ffffffff"
    );
}

#[test]
fn what_an_app_wrote_in_flash_is_part_of_the_saved_state() {
    let mut session = load("flash_rw", None);
    assert_eq!(exit_code(&mut session), 0);
    let dump = call(&mut session, cmd("dump"))["nds"].clone();

    let mut other = Session::new();
    call(&mut other, cmd("restore").with("nds", dump));
    assert_eq!(read(&mut other, 0x9070_0000, 4), "0df0feca");
}

#[test]
fn shift_and_alpha_are_read_and_written_by_the_app() {
    let mut session = load("modifiers", None);
    assert_eq!(exit_code(&mut session), 0);
}
