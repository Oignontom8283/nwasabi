//! Le protocole de session JSON, de bout en bout (sans passer par un binaire).

mod support;

use nwasabi::json::{self, Value};
use nwasabi::session::Session;

fn ask(session: &mut Session, request: &str) -> Value {
    let reply = session.handle_line(request);
    json::parse(&reply).unwrap_or_else(|e| panic!("réponse invalide ({e}) : {reply}"))
}

fn ok(session: &mut Session, request: &str) -> Value {
    let reply = ask(session, request);
    assert_eq!(
        reply.get("ok"),
        Some(&Value::Bool(true)),
        "{request} -> {reply}"
    );
    reply
}

fn error(session: &mut Session, request: &str) -> String {
    let reply = ask(session, request);
    assert_eq!(
        reply.get("ok"),
        Some(&Value::Bool(false)),
        "{request} -> {reply}"
    );
    reply
        .get("error")
        .and_then(Value::as_str)
        .unwrap()
        .to_owned()
}

#[test]
fn errors_are_replies_not_failures_and_ids_are_echoed() {
    let mut s = Session::new();
    assert!(error(&mut s, "pas du json").contains("JSON invalide"));
    assert!(error(&mut s, r#"{"cmd":"inconnue"}"#).contains("commande inconnue"));
    assert!(error(&mut s, r#"{"cmd":"fs_list"}"#).contains("aucun appareil"));
    assert!(error(&mut s, r#"{"nocmd":1}"#).contains("cmd"));
    let r = ask(&mut s, r#"{"cmd":"help","id":"abc"}"#);
    assert_eq!(r.get("id").and_then(Value::as_str), Some("abc"));
    assert!(r.get("commands").and_then(Value::as_array).unwrap().len() > 20);
}

