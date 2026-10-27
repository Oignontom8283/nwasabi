//! Le serveur GDB, de bout en bout : un faux débogueur parle le protocole sur une connexion locale.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use nwasabi::gdb::serve_connection;
use nwasabi::json::{self, Value};
use nwasabi::session::Session;

struct Debugger(TcpStream);

impl Debugger {
    fn send(&mut self, data: &str) {
        let sum = data.bytes().fold(0u8, |a, b| a.wrapping_add(b));
        self.0
            .write_all(format!("${data}#{sum:02x}").as_bytes())
            .unwrap();
    }

    /// Envoie un paquet et rend la réponse (sans l'enveloppe), en vérifiant son accusé et sa somme.
    fn ask(&mut self, data: &str) -> String {
        self.send(data);
        self.read()
    }

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

fn word(hex: &str, index: usize) -> u32 {
    let bytes = json::hex_decode(&hex[index * 8..index * 8 + 8]).unwrap();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[test]
fn a_debugger_can_break_step_read_and_write() {
    let nwa = include_bytes!("apps/bin/keys.nwa");
    let mut session = Session::new();
    let loaded = session.handle_line(&format!(
        r#"{{"cmd":"load","nwa":"{}"}}"#,
        json::base64_encode(nwa)
    ));
    assert!(loaded.contains(r#""ok":true"#), "{loaded}");
    let symbols = json::parse(&session.handle_line(r#"{"cmd":"symbols","query":"main"}"#)).unwrap();
    let main = symbols.get("symbols").and_then(Value::as_array).unwrap()[0]
        .get("address")
        .and_then(Value::as_i64)
        .unwrap() as u32;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve_connection(&mut session, stream).unwrap();
    });
    let mut d = Debugger(TcpStream::connect(("127.0.0.1", port)).unwrap());

    // Négociation : taille des paquets, description des registres.
    assert!(d.ask("qSupported:multiprocess+").contains("PacketSize="));
    let xml = d.ask("qXfer:features:read:target.xml:0,fff");
    assert!(xml.starts_with('l') && xml.contains(r#"name="pc""#), "{xml}");
    assert_eq!(d.ask("?"), "T05");

    // Registres : 17 mots (r0 à r12, sp, lr, pc, xpsr) ; pc est au début du programme.
    let regs = d.ask("g");
    assert_eq!(regs.len(), 17 * 8);
    let entry = word(&regs, 15);
    assert_ne!(entry, 0);

    // Écrire puis relire un registre et la mémoire.
    assert_eq!(d.ask("P3=78563412"), "OK");
    assert_eq!(word(&d.ask("g"), 3), 0x1234_5678);
    assert_eq!(d.ask("M20020000,4:deadbeef"), "OK");
    assert_eq!(d.ask("m20020000,4"), "deadbeef");
    assert_eq!(d.ask("m10,4"), "E14", "mémoire inexistante");

    // Point d'arrêt sur `main`, reprise : arrêt avant l'instruction, pc sur `main`.
    assert_eq!(d.ask(&format!("Z0,{main:x},2")), "OK");
    let stop = d.ask("c");
    assert!(stop.starts_with("T05swbreak:"), "{stop}");
    assert_eq!(word(&d.ask("g"), 15), main);
    // Un pas avance d'une instruction.
    assert!(d.ask("s").starts_with("T05"));
    assert_ne!(word(&d.ask("g"), 15), main);
    // Retirer le point, puis reprendre et interrompre (Ctrl+C).
    assert_eq!(d.ask(&format!("z0,{main:x},2")), "OK");
    d.send("c");
    thread::sleep(std::time::Duration::from_millis(100));
    d.0.write_all(&[0x03]).unwrap();
    assert_eq!(d.read(), "T02");
    // Un point d'observation en écriture arrête l'exécution sur l'instruction qui écrit. Programme en RAM :
    // `str r1, [r0]` puis `b .`, avec r0 sur la zone observée.
    assert_eq!(d.ask("M20020000,4:0160fee7"), "OK");
    assert_eq!(d.ask("P0=00500220"), "OK"); // r0 = 0x20025000
    assert_eq!(d.ask("Pf=00000220"), "OK"); // pc = 0x20020000
    assert_eq!(d.ask("Z2,20025000,4"), "OK");
    let stop = d.ask("c");
    assert!(stop.starts_with("T05watch:20025000;"), "{stop}");
    assert_eq!(d.ask("z2,20025000,4"), "OK");
    assert_eq!(d.ask("vCont?"), "vCont;c;s;t");
    assert_eq!(d.ask("D"), "OK");
    server.join().unwrap();
}
