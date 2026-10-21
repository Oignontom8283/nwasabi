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

