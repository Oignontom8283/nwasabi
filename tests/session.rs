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

#[test]
fn files_round_trip_through_the_protocol() {
    let mut s = Session::new();
    ok(&mut s, r#"{"cmd":"new","model":"n0110"}"#);
    ok(
        &mut s,
        r#"{"cmd":"fs_write","name":"a.py","python":true,"text":"print(1)","auto_import":true}"#,
    );
    ok(
        &mut s,
        r#"{"cmd":"fs_write","name":"b.txt","base64":"AAEC"}"#,
    );
    let list = ok(&mut s, r#"{"cmd":"fs_list"}"#);
    let files = list.get("files").and_then(Value::as_array).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].get("kind").and_then(Value::as_str), Some("python"));
    assert_eq!(files[1].get("size").and_then(Value::as_i64), Some(3));

    let r = ok(&mut s, r#"{"cmd":"fs_read","name":"a.py","python":true}"#);
    assert_eq!(r.get("text").and_then(Value::as_str), Some("print(1)"));
    assert_eq!(r.get("auto_import"), Some(&Value::Bool(true)));
    let r = ok(&mut s, r#"{"cmd":"fs_read","name":"b.txt"}"#);
    assert_eq!(r.get("base64").and_then(Value::as_str), Some("AAEC"));

    ok(&mut s, r#"{"cmd":"fs_rename","from":"b.txt","to":"c.txt"}"#);
    ok(&mut s, r#"{"cmd":"fs_remove","name":"a.py"}"#);
    assert!(error(&mut s, r#"{"cmd":"fs_remove","name":"a.py"}"#).contains("introuvable"));
    let stats = ok(&mut s, r#"{"cmd":"fs_stats"}"#);
    assert_eq!(stats.get("records").and_then(Value::as_i64), Some(1));
    assert_eq!(stats.get("total").and_then(Value::as_i64), Some(43008));
}

#[test]
fn settings_are_visible_in_the_memory_structures() {
    let mut s = Session::new();
    ok(&mut s, r#"{"cmd":"new"}"#);
    ok(
        &mut s,
        r#"{"cmd":"settings_set","name":"version","value":"27.0.1"}"#,
    );
    ok(
        &mut s,
        r#"{"cmd":"settings_set","name":"device_name","value":"Mon appareil"}"#,
    );
    ok(
        &mut s,
        r#"{"cmd":"settings_set","name":"battery","value":{"level":1,"voltage":3.5,"charging":true}}"#,
    );
    let inspect = ok(&mut s, r#"{"cmd":"inspect"}"#);
    let st = inspect.get("structures").unwrap();
    let kernel = st.get("kernel_header").unwrap();
    assert_eq!(
        kernel.get("software_version").and_then(Value::as_str),
        Some("27.0.1")
    );
    assert_eq!(kernel.get("valid"), Some(&Value::Bool(true)));
    assert_eq!(
        st.get("device_name").and_then(Value::as_str),
        Some("Mon appareil")
    );

    let got = ok(&mut s, r#"{"cmd":"settings_get"}"#);
    assert_eq!(
        got.get("battery")
            .and_then(|b| b.get("level"))
            .and_then(Value::as_i64),
        Some(1)
    );
    assert!(
        error(
            &mut s,
            r#"{"cmd":"settings_set","name":"serial_number","value":"court"}"#
        )
        .contains("serial_number")
    );
    assert!(error(&mut s, r#"{"cmd":"settings_set","name":"zzz","value":1}"#).contains("inconnu"));
}

