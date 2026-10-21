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

