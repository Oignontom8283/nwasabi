//! Une transcription complète du protocole de session : une série fixe de requêtes sur une application de test, avec
//! la réponse exacte de chacune. Elle sert de garde-fou aux remaniements du code de la session (découpage, types
//! pour le JSON…) : tant que le fichier de référence ne change pas, les réponses sont les mêmes à l'octet.
//!
//! Les réponses très longues (écran, état complet…) sont réduites à leur taille et à leur CRC-32.
//!
//! Pour mettre à jour le fichier de référence après un changement voulu : `INSTA_UPDATE=always cargo test --test protocol_golden`.

mod support;

use nwasabi::json;
use nwasabi::session::Session;

/// Au-delà, une réponse est réduite à sa taille et à son CRC-32.
const LONG: usize = 1500;

fn app() -> String {
    let path = format!("{}/tests/apps/bin/debugme.nwa", env!("CARGO_MANIFEST_DIR"));
    json::base64_encode(&std::fs::read(path).unwrap())
}

/// Les requêtes, dans l'ordre. `$APP` est remplacé par l'application de test en base64.
const SCRIPT: &[&str] = &[
    // Sans appareil, et les erreurs
    r#"{"cmd":"help"}"#,
    r#"{"cmd":"cpu"}"#,
    r#"{"cmd":"inconnue"}"#,
    r#"{"nocmd":1}"#,
    // Un appareil et une application
    r#"{"cmd":"new","model":"n0110"}"#,
    r#"{"cmd":"regions"}"#,
    r#"{"cmd":"load","nwa":"$APP","id":"un"}"#,
    r#"{"cmd":"inspect"}"#,
    r#"{"cmd":"regions"}"#,
    r#"{"cmd":"usage"}"#,
    r#"{"cmd":"cpu"}"#,
    r#"{"cmd":"settings_get"}"#,
    r#"{"cmd":"speed"}"#,
    r#"{"cmd":"time"}"#,
    r#"{"cmd":"stdout"}"#,
    r#"{"cmd":"screen"}"#,
    r#"{"cmd":"changes"}"#,
    // Symboles et débogage
    r#"{"cmd":"symbols"}"#,
    r#"{"cmd":"backtrace"}"#,
    r#"{"cmd":"break_function","name":"factorial"}"#,
    r#"{"cmd":"breaks"}"#,
    r#"{"cmd":"history","action":"start"}"#,
    r#"{"cmd":"run","instructions":5000000}"#,
    r#"{"cmd":"cpu"}"#,
    r#"{"cmd":"backtrace"}"#,
    r#"{"cmd":"eval","expr":"n"}"#,
    r#"{"cmd":"eval","expr":"global_counter"}"#,
    r#"{"cmd":"eval","expr":"factorial(4)"}"#,
    r#"{"cmd":"disasm","address":134217728}"#,
    r#"{"cmd":"step"}"#,
    r#"{"cmd":"cpu"}"#,
    r#"{"cmd":"step_back"}"#,
    r#"{"cmd":"cpu"}"#,
    r#"{"cmd":"history"}"#,
    r#"{"cmd":"unbreak","address":0}"#,
    r#"{"cmd":"breaks"}"#,
    r#"{"cmd":"break","address":134217728}"#,
    r#"{"cmd":"debug_info"}"#,
    r#"{"cmd":"line_at","address":134217728}"#,
    r#"{"cmd":"source_lines","file":"debugme.c"}"#,
    r#"{"cmd":"break_line","file":"debugme.c","line":14}"#,
    r#"{"cmd":"unbreak_line","file":"debugme.c","line":14}"#,
    r#"{"cmd":"frames"}"#,
    r#"{"cmd":"locals"}"#,
    r#"{"cmd":"globals"}"#,
    r#"{"cmd":"ptype","expr":"origin"}"#,
    r#"{"cmd":"assign","expr":"global_counter","value":9}"#,
    r#"{"cmd":"step_source"}"#,
    r#"{"cmd":"symbol","address":134217728}"#,
    r#"{"cmd":"cpu_set","register":"r0","value":42}"#,
    r#"{"cmd":"reverse_continue"}"#,
    r#"{"cmd":"history_goto","cycles":0}"#,
    // Mémoire, points d'observation, appels système
    r#"{"cmd":"memory_read","address":536870912,"length":32}"#,
    r#"{"cmd":"memory_write","address":536870912,"hex":"deadbeef"}"#,
    r#"{"cmd":"memory_read","address":536870912,"length":8}"#,
    r#"{"cmd":"watch","address":536870912,"kind":"write","length":4,"stop":true}"#,
    r#"{"cmd":"watch_list"}"#,
    r#"{"cmd":"watch_hits"}"#,
    r#"{"cmd":"unwatch","id":1}"#,
    r#"{"cmd":"svc_log","action":"start"}"#,
    r#"{"cmd":"advance","ms":20}"#,
    r#"{"cmd":"svc_stats"}"#,
    r#"{"cmd":"svc_log","action":"stop"}"#,
    // Clavier, fichiers, profils, état
    r#"{"cmd":"key","key":"ok","down":true}"#,
    r#"{"cmd":"key","key":"ok","down":false}"#,
    r#"{"cmd":"fs_write","name":"a.py","python":true,"text":"print(1)"}"#,
    r#"{"cmd":"fs_list"}"#,
    r#"{"cmd":"fs_stats"}"#,
    r#"{"cmd":"fs_read","name":"a.py"}"#,
    r#"{"cmd":"fs_rename","from":"a.py","to":"b.py"}"#,
    r#"{"cmd":"fs_remove","name":"b.py"}"#,
    r#"{"cmd":"fs_export"}"#,
    r#"{"cmd":"flash_export"}"#,
    r#"{"cmd":"profile","action":"start"}"#,
    r#"{"cmd":"advance","ms":5}"#,
    r#"{"cmd":"profile","action":"stop"}"#,
    r#"{"cmd":"profile"}"#,
    r#"{"cmd":"settings_set","name":"brightness","value":96}"#,
    r#"{"cmd":"settings_get"}"#,
    r#"{"cmd":"export_elf"}"#,
    r#"{"cmd":"flash_load","nfd":{}}"#,
    r#"{"cmd":"fs_import","nfs":{}}"#,
    r#"{"cmd":"restore"}"#,
    r#"{"cmd":"dump"}"#,
    r#"{"cmd":"launch"}"#,
    r#"{"cmd":"run","instructions":1000}"#,
    r#"{"cmd":"cpu"}"#,
];

fn shorten(reply: &str) -> String {
    if reply.len() <= LONG {
        return reply.to_owned();
    }
    format!(
        "<{} octets, crc32 {:08x}>",
        reply.len(),
        support::crc32(reply.as_bytes())
    )
}

fn transcript() -> String {
    let app = app();
    let mut session = Session::new();
    let mut out = String::new();
    for request in SCRIPT {
        let request = request.replace("$APP", &app);
        let shown = request.replace(&app, "<application>");
        let reply = session.handle_line(&request);
        out.push_str(&format!("> {shown}\n< {}\n", shorten(&reply)));
    }
    out
}

#[test]
fn the_session_protocol_answers_exactly_as_recorded() {
    support::snapshot("protocol", transcript());
}

#[test]
fn the_transcript_is_the_same_twice_in_a_row() {
    assert_eq!(transcript(), transcript());
}
