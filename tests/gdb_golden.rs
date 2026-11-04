//! Une transcription du serveur GDB : une série fixe de paquets envoyés par un faux débogueur à une session qui a chargé
//! l'application de test `debugme`, avec la réponse exacte à chacun. Garde-fou des remaniements de `gdb.rs` (découpage en
//! dossier, passage à une bibliothèque de protocole) : tant que le fichier de référence ne change pas, un débogueur voit les
//! mêmes réponses à l'octet.
//!
//! Pour régénérer le fichier après un changement voulu : `NWASABI_UPDATE_GOLDEN=1 cargo test --test gdb_golden`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use nwasabi::gdb::serve_connection;
use nwasabi::json::{self, Value};
use nwasabi::session::Session;

const GOLDEN: &str = "tests/golden/gdb.txt";

struct Debugger(TcpStream);

impl Debugger {
    fn ask(&mut self, data: &str) -> String {
        let sum = data.bytes().fold(0u8, |a, b| a.wrapping_add(b));
        self.0
            .write_all(format!("${data}#{sum:02x}").as_bytes())
            .unwrap();
        self.read()
    }

    /// Lit un paquet, vérifie sa somme, l'accuse et rend son corps.
    fn read(&mut self) -> String {
        let mut bytes = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            self.0.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
            if bytes.len() >= 3 && bytes[bytes.len() - 3] == b'#' {
                break;
            }
        }
        let text = String::from_utf8(bytes).unwrap();
        let start = text.find('$').expect("paquet");
        let hash = text.rfind('#').unwrap();
        let body = &text[start + 1..hash];
        let sum = body.bytes().fold(0u8, |a, b| a.wrapping_add(b));
        assert_eq!(&text[hash + 1..], format!("{sum:02x}"), "somme de {body}");
        self.0.write_all(b"+").unwrap();
        body.to_owned()
    }
}

fn function_address(session: &mut Session, name: &str) -> u32 {
    let request = format!(r#"{{"cmd":"symbols","query":"{name}"}}"#);
    let reply = json::parse(&session.handle_line(&request)).unwrap();
    reply
        .get("symbols")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .and_then(|symbol| symbol.get("address"))
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("symbole {name} introuvable")) as u32
}

/// Les paquets, dans l'ordre. `{F}` est l'adresse de `factorial`.
const PACKETS: &[&str] = &[
    "qSupported:multiprocess+;swbreak+;hwbreak+;vContSupported+",
    "qXfer:features:read:target.xml:0,1000",
    "?",
    "qAttached",
    "qC",
    "qfThreadInfo",
    "qsThreadInfo",
    "Hg0",
    "Hc-1",
    "g",
    "p0",
    "pf",
    "m20000000,10",
    "M20000000,4:deadbeef",
    "m20000000,4",
    "Z0,{F},2",
    "Z2,20000010,4",
    "vCont?",
    "c",
    "g",
    "pf",
    "s",
    "g",
    "bs",
    "g",
    "z0,{F},2",
    "z2,20000010,4",
    "vCont;s",
    "inconnu",
    "D",
];

fn transcript() -> String {
    let nwa = std::fs::read(format!(
        "{}/tests/apps/bin/debugme.nwa",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut session = Session::new();
    let loaded = session.handle_line(&format!(
        r#"{{"cmd":"load","nwa":"{}"}}"#,
        json::base64_encode(&nwa)
    ));
    assert!(loaded.contains(r#""ok":true"#), "{loaded}");
    let factorial = function_address(&mut session, "factorial");

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve_connection(&mut session, stream).unwrap();
    });
    let mut debugger = Debugger(TcpStream::connect(("127.0.0.1", port)).unwrap());

    let mut out = String::new();
    for packet in PACKETS {
        let packet = packet.replace("{F}", &format!("{factorial:x}"));
        let reply = debugger.ask(&packet);
        out.push_str(&format!("> {packet}\n< {reply}\n"));
    }
    drop(debugger);
    server.join().unwrap();
    out
}

#[test]
fn the_gdb_server_answers_exactly_as_recorded() {
    let actual = transcript();
    let path = format!("{}/{GOLDEN}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("NWASABI_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path} : {e} (NWASABI_UPDATE_GOLDEN=1 pour le créer)"));
    if actual != expected {
        let first = actual
            .lines()
            .zip(expected.lines())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
        panic!(
            "la transcription GDB a changé à la ligne {} :\n  attendu : {}\n  obtenu  : {}",
            first + 1,
            expected.lines().nth(first).unwrap_or("(fin)"),
            actual.lines().nth(first).unwrap_or("(fin)"),
        );
    }
}

#[test]
fn the_gdb_transcript_is_the_same_twice_in_a_row() {
    assert_eq!(transcript(), transcript());
}
