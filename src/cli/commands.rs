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

// ---- inspect -------------------------------------------------------------------------------------

pub fn inspect(raw: &[String]) -> Outcome {
    let args = Args::parse(raw, &["json"])?;
    let mut session = Session::new();
    match args.positional.as_slice() {
        [] => {
            call(
                &mut session,
                Value::object()
                    .with("cmd", "new")
                    .with("model", model_option(&args)?),
            )?;
        }
        [path] => {
            call(&mut session, load_request(&args, &read_file(path)?)?)?;
        }
        _ => return Err("trop d'arguments".to_owned()),
    }
    let reply = call(&mut session, Value::object().with("cmd", "inspect"))?;
    // Toujours en JSON : c'est le format de référence de cette vue.
    println!("{reply}");
    Ok(0)
}

// ---- link ----------------------------------------------------------------------------------------

pub fn link_cmd(raw: &[String]) -> Outcome {
    let args = Args::parse(raw, &["json", "map"])?;
    let path = args.one_positional("fichier .nwa")?;
    let model = model_option(&args)?;
    let model = Model::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(model))
        .expect("vérifié");
    let device =
        DeviceImage::new(FirmwareProfile::default_for(model)).map_err(|e| e.to_string())?;
    let data = match args.value("data") {
        Some(p) => Some(read_file(p)?),
        None => None,
    };
    let app = link(
        &read_file(path)?,
        data.as_deref(),
        &device.app_layout(),
        &LinkOptions::default(),
    )
    .map_err(|e| e.to_string())?;

    let range = |a: u32, n: u32| Value::object().with("start", a).with("end", a + n);
    let mut out = Value::object()
        .with("name", app.name.as_str())
        .with("api_level", app.api_level)
        .with("flash_start", app.flash_start)
        .with("flash_size", app.flash.len())
        .with("installed_size", app.installed_size())
        .with("entry", app.entry)
        .with("data", range(app.data.address, app.data.size))
        .with("bss", range(app.bss.address, app.bss.size))
        .with("heap", range(app.heap.address, app.heap.size))
        .with(
            "warnings",
            app.warnings
                .iter()
                .map(|w| Value::from(format!("{w:?}")))
                .collect::<Vec<_>>(),
        );
    if args.flag("map") {
        out.set(
            "sections",
            app.sections
                .iter()
                .map(|s| {
                    Value::object()
                        .with("name", s.name.as_str())
                        .with("origin", format!("{:?}", s.origin).to_lowercase())
                        .with("class", format!("{:?}", s.class).to_lowercase())
                        .with("address", s.address)
                        .with("flash_address", s.flash_address)
                        .with("size", s.size)
                        .with("alignment", s.alignment)
                })
                .collect::<Vec<_>>(),
        );
        let mut symbols: Vec<_> = app.symbols.iter().collect();
        symbols.sort_by(|a, b| (a.address, &a.name).cmp(&(b.address, &b.name)));
        out.set(
            "symbols",
            symbols
                .iter()
                .map(|s| {
                    Value::object()
                        .with("name", s.name.as_str())
                        .with("address", s.address)
                        .with("size", s.size)
                        .with("origin", format!("{:?}", s.origin).to_lowercase())
                })
                .collect::<Vec<_>>(),
        );
    }
    print(&args, &out, || {
        let mut text = format!(
            "{} (API {})\nflash {:#010x}, {} octets (installé : {})\nentrée {:#010x}\ndata {:#010x}+{:#x}  bss {:#010x}+{:#x}  tas {:#010x}+{:#x}",
            app.name,
            app.api_level,
            app.flash_start,
            app.flash.len(),
            app.installed_size(),
            app.entry,
            app.data.address,
            app.data.size,
            app.bss.address,
            app.bss.size,
            app.heap.address,
            app.heap.size
        );
        for w in &app.warnings {
            text += &format!("\navertissement : {w:?}");
        }
        if args.flag("map") {
            text += "\n\nsections :";
            for s in &app.sections {
                text += &format!(
                    "\n  {:#010x} {:>7} {:?} {}",
                    s.address, s.size, s.class, s.name
                );
            }
            let mut symbols: Vec<_> = app.symbols.iter().collect();
            symbols.sort_by(|a, b| (a.address, &a.name).cmp(&(b.address, &b.name)));
            text += "\n\nsymboles :";
            for s in symbols {
                text += &format!("\n  {:#010x} {:>6} {}", s.address, s.size, s.name);
            }
        }
        text
    });
    Ok(0)
}

