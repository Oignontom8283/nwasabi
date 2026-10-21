//! Les sous-commandes de `nwasabi-cli`.

use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;

use nwasabi::device::DeviceImage;
use nwasabi::json::{self, Value};
use nwasabi::kernel::Key;
use nwasabi::link::{LinkOptions, link};
use nwasabi::profile::{FirmwareProfile, Model};
use nwasabi::session::Session;
use nwasabi::storage::records;

use crate::args::Args;

/// Taille du tampon du système de fichiers d'un N0110 (octets).
const STORAGE_SIZE: usize = 0xa800;

pub type Outcome = Result<i32, String>;

fn model_option(args: &Args) -> Result<&str, String> {
    let name = args.value("model").unwrap_or("n0110");
    Model::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(name))
        .map(|_| name)
        .ok_or_else(|| format!("modèle inconnu : {name} (n0110, n0115 ou n0120)"))
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("{path} : {e}"))
}

fn write_file(path: &str, data: &[u8]) -> Result<(), String> {
    fs::write(path, data).map_err(|e| format!("{path} : {e}"))
}

/// Envoie une requête à la session ; une réponse « ok: false » devient une erreur.
fn call(session: &mut Session, request: Value) -> Result<Value, String> {
    let reply = session.handle(&request);
    if reply.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(reply)
    } else {
        Err(reply
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("erreur inconnue")
            .to_owned())
    }
}

fn load_request(args: &Args, nwa: &[u8]) -> Result<Value, String> {
    let mut request = Value::object()
        .with("cmd", "load")
        .with("model", model_option(args)?)
        .with("nwa_base64", json::base64_encode(nwa));
    if let Some(path) = args.value("data") {
        request.set("data_base64", json::base64_encode(&read_file(path)?));
    }
    Ok(request)
}

fn print(args: &Args, json_value: &Value, text: impl FnOnce() -> String) {
    if args.flag("json") {
        println!("{json_value}");
    } else {
        println!("{}", text());
    }
}

// ---- run -----------------------------------------------------------------------------------------

pub fn run(raw: &[String]) -> Outcome {
    let args = Args::parse(raw, &["json"])?;
    let path = args.one_positional("fichier .nwa")?;
    let mut session = Session::new();
    call(&mut session, load_request(&args, &read_file(path)?)?)?;

    for key in args.values("press") {
        call(
            &mut session,
            Value::object()
                .with("cmd", "key")
                .with("key", key.as_str())
                .with("action", "press"),
        )?;
    }
    let mut request = Value::object().with("cmd", "run");
    if let Some(n) = args.number("instructions")? {
        request.set("instructions", n);
    }
    if let Some(n) = args.number("frames")? {
        request.set("frames", n);
    }
    let ran = call(&mut session, request)?;
    let screen = call(
        &mut session,
        Value::object()
            .with("cmd", "screen")
            .with("png", args.value("screenshot").is_some()),
    )?;
    if let Some(out) = args.value("screenshot") {
        let png = json::base64_decode(
            screen
                .get("png_base64")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )
        .ok_or("capture illisible")?;
        write_file(out, &png)?;
    }

    let stop = ran.get("stop").cloned().unwrap_or(Value::Null);
    let kind = stop
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_owned();
    let stdout = call(&mut session, Value::object().with("cmd", "stdout"))?;
    let summary = Value::object()
        .with("stop", stop)
        .with("frames", ran.get("frames").cloned())
        .with("instructions", ran.get("instructions").cloned())
        .with("time_ms", ran.get("time_ms").cloned())
        .with("screen_crc32", screen.get("crc32").cloned())
        .with("stdout", stdout.get("text").cloned());
    print(&args, &summary, || {
        format!(
            "arrêt : {}\nimages : {}\ninstructions : {}\ntemps virtuel : {} ms\nécran (crc32) : {}\nsortie standard : {:?}",
            summary.get("stop").map_or(String::new(), |s| s.to_string()),
            summary
                .get("frames")
                .map_or(String::new(), |v| v.to_string()),
            summary
                .get("instructions")
                .map_or(String::new(), |v| v.to_string()),
            summary
                .get("time_ms")
                .map_or(String::new(), |v| v.to_string()),
            screen.get("crc32").and_then(Value::as_str).unwrap_or(""),
            stdout.get("text").and_then(Value::as_str).unwrap_or(""),
        )
    });
    Ok(if kind == "fault" { 3 } else { 0 })
}

