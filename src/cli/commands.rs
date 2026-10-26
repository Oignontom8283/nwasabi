//! Les sous-commandes de `nwasabi-cli`.

use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use nwasabi::device::DeviceImage;
use nwasabi::json::{self, Value, ValueExt};
use nwasabi::kernel::Key;
use nwasabi::link::{LinkOptions, link};
use nwasabi::profile::{FirmwareProfile, Model};
use nwasabi::session::Session;
use nwasabi::storage::records;

use crate::options::{
    DeviceOpts, FirmwareOpt, FsArgs, FsOp, InspectArgs, KeysArgs, LinkArgs, RunArgs, SessionArgs,
    SurveyArgs,
};

/// Taille du tampon du système de fichiers d'un N0110 (octets).
const STORAGE_SIZE: usize = 0xa800;

pub type Outcome = Result<i32, String>;

fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, String> {
    let path = path.as_ref();
    fs::read(path).map_err(|e| format!("{} : {e}", path.display()))
}

fn write_file(path: impl AsRef<Path>, data: &[u8]) -> Result<(), String> {
    let path = path.as_ref();
    fs::write(path, data).map_err(|e| format!("{} : {e}", path.display()))
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

/// Requête `new` ou `load` pour l'appareil demandé.
fn device_request(command: &str, device: &DeviceOpts) -> Value {
    let request = Value::object()
        .with("cmd", command)
        .with("model", device.model.as_str())
        .with("firmware", device.firmware.as_str());
    match device.mhz {
        Some(mhz) => request.with("speed", "custom").with("mhz", mhz),
        None => request,
    }
}

fn load_request(device: &DeviceOpts, nwa: &[u8]) -> Result<Value, String> {
    let mut request = device_request("load", device).with("nwa_base64", json::base64_encode(nwa));
    if let Some(path) = &device.data {
        request.set("data_base64", json::base64_encode(&read_file(path)?));
    }
    Ok(request)
}

fn print(json: bool, value: &Value, text: impl FnOnce() -> String) {
    if json {
        println!("{value}");
    } else {
        println!("{}", text());
    }
}

/// Les pixels bruts d'une réponse `screen` (`rgba_base64`) encodés en PNG : l'encodage est fait ici,
/// côté hôte, le cœur ne rend que des pixels.
fn png_from_reply(screen: &Value) -> Result<Vec<u8>, String> {
    let rgba = screen
        .get("rgba_base64")
        .and_then(Value::as_str)
        .and_then(json::base64_decode)
        .ok_or("capture illisible")?;
    let width = screen.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
    let height = screen.get("height").and_then(Value::as_u64).unwrap_or(0) as u32;
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

// ---- run -----------------------------------------------------------------------------------------

pub fn run(args: RunArgs) -> Outcome {
    let mut session = Session::new();
    call(
        &mut session,
        load_request(&args.device, &read_file(&args.app)?)?,
    )?;

    for key in &args.press {
        call(
            &mut session,
            Value::object()
                .with("cmd", "key")
                .with("key", key.as_str())
                .with("action", "press"),
        )?;
    }
    let mut request = Value::object().with("cmd", "run");
    if let Some(n) = args.instructions {
        request.set("instructions", n);
    }
    if let Some(n) = args.frames {
        request.set("frames", n);
    }
    let ran = call(&mut session, request)?;
    let screen = call(
        &mut session,
        Value::object()
            .with("cmd", "screen")
            .with("rgba_base64", args.screenshot.is_some()),
    )?;
    if let Some(out) = &args.screenshot {
        write_file(out, &png_from_reply(&screen)?)?;
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
    print(args.json, &summary, || {
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

pub fn inspect(args: InspectArgs) -> Outcome {
    let mut session = Session::new();
    match &args.app {
        None => {
            call(&mut session, device_request("new", &args.device))?;
        }
        Some(path) => {
            call(&mut session, load_request(&args.device, &read_file(path)?)?)?;
        }
    }
    let reply = call(&mut session, Value::object().with("cmd", "inspect"))?;
    // Toujours en JSON : c'est le format de référence de cette vue.
    println!("{reply}");
    Ok(0)
}

// ---- link ----------------------------------------------------------------------------------------

pub fn link_cmd(args: LinkArgs) -> Outcome {
    let model = Model::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(args.device.model.as_str()))
        .expect("modèle connu de clap");
    let profile = match args.device.firmware {
        FirmwareOpt::V24 => FirmwareProfile::epsilon_24(model),
        FirmwareOpt::V26 => FirmwareProfile::epsilon_26_4_0(model),
    };
    let device = DeviceImage::new(profile).map_err(|e| e.to_string())?;
    let data = match &args.device.data {
        Some(p) => Some(read_file(p)?),
        None => None,
    };
    let app = link(
        &read_file(&args.app)?,
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
    if args.map {
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
    print(args.json, &out, || {
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
        if args.map {
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

// ---- fs ------------------------------------------------------------------------------------------

/// Manipule une image du système de fichiers (le tampon de 42 Kio) stockée dans un fichier.
pub fn fs_cmd(args: FsArgs) -> Outcome {
    let image_path = &args.image;
    let shown = image_path.display();
    let mut buffer = match fs::read(image_path) {
        Ok(bytes) if bytes.len() == STORAGE_SIZE => bytes,
        Ok(bytes) => {
            return Err(format!(
                "{shown} : {} octets au lieu de {STORAGE_SIZE}",
                bytes.len()
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![0; STORAGE_SIZE],
        Err(e) => return Err(format!("{shown} : {e}")),
    };
    let e = |e: nwasabi::storage::FsError| e.to_string();
    match &args.op {
        FsOp::List => {
            let parsed = records::parse(&buffer);
            let files: Vec<Value> = parsed
                .records
                .iter()
                .map(|r| {
                    Value::object()
                        .with("name", r.name.as_str())
                        .with("size", r.body_len)
                        .with("offset", r.offset)
                })
                .collect();
            let out = Value::object()
                .with("files", files)
                .with("corrupt_at", parsed.corrupt_at);
            print(args.json, &out, || {
                let mut t = String::new();
                for r in &parsed.records {
                    t += &format!("{:>6}  {}\n", r.body_len, r.name);
                }
                if let Some(at) = parsed.corrupt_at {
                    t += &format!("système de fichiers abîmé à l'octet {at}\n");
                }
                t.trim_end().to_owned()
            });
        }
        FsOp::Stats => {
            let parsed = records::parse(&buffer);
            let out = Value::object()
                .with("total", buffer.len())
                .with("used", parsed.used)
                .with("free", buffer.len() - parsed.used)
                .with("records", parsed.records.len());
            print(args.json, &out, || {
                format!(
                    "{} octets utilisés sur {}, {} fichier(s)",
                    parsed.used,
                    buffer.len(),
                    parsed.records.len()
                )
            });
        }
        FsOp::Read { name, out } => {
            let body = records::read(&buffer, name).map_err(e)?;
            match out {
                Some(path) => write_file(path, &body)?,
                None => std::io::stdout()
                    .write_all(&body)
                    .map_err(|e| e.to_string())?,
            }
        }
        FsOp::Write { name, from, text } => {
            let body = match (from, text) {
                (Some(path), _) => read_file(path)?,
                (None, Some(t)) => t.as_bytes().to_vec(),
                (None, None) => unreachable!("clap exige --from ou --text"),
            };
            buffer = records::write(&buffer, name, &body).map_err(e)?;
            write_file(image_path, &buffer)?;
        }
        FsOp::Remove { name } => {
            buffer = records::remove(&buffer, name).map_err(e)?;
            write_file(image_path, &buffer)?;
        }
        FsOp::Rename { from, to } => {
            buffer = records::rename(&buffer, from, to).map_err(e)?;
            write_file(image_path, &buffer)?;
        }
    }
    Ok(0)
}

// ---- survey --------------------------------------------------------------------------------------

fn collect_apps(paths: &[PathBuf]) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            let entries = fs::read_dir(path).map_err(|e| format!("{} : {e}", path.display()))?;
            let mut found: Vec<String> = entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "nwa"))
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            found.sort();
            files.extend(found);
        } else {
            files.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(files)
}

/// Lance chaque application sans touche et résume ce qui s'est passé (comme le test du corpus).
pub fn survey(args: SurveyArgs) -> Outcome {
    let files = collect_apps(&args.paths)?;
    if files.is_empty() {
        return Err("aucun fichier .nwa (donner des fichiers ou un dossier)".to_owned());
    }
    let budget = args.instructions.unwrap_or(40_000_000);
    let mut results = Vec::new();
    let mut faults = 0;
    for path in &files {
        let nwa = read_file(path)?;
        let attempt = |model: &str, data: Option<&[u8]>| -> Result<Value, String> {
            let mut s = Session::new();
            let mut load = Value::object()
                .with("cmd", "load")
                .with("model", model)
                .with("nwa_base64", json::base64_encode(&nwa));
            if let Some(d) = data {
                load.set("data_base64", json::base64_encode(d));
            }
            call(&mut s, load)?;
            let ran = call(
                &mut s,
                Value::object()
                    .with("cmd", "run")
                    .with("instructions", budget),
            )?;
            let screen = call(&mut s, Value::object().with("cmd", "screen"))?;
            Ok(Value::object()
                .with("model", model)
                .with("stop", ran.get("stop").cloned())
                .with("frames", ran.get("frames").cloned())
                .with("instructions", ran.get("instructions").cloned())
                .with("screen_crc32", screen.get("crc32").cloned()))
        };
        let mut result = attempt("n0110", None).or_else(|_| attempt("n0110", Some(&[])));
        // Une application faite pour le N0120 sonde la RAM à 0x24000000.
        let wants_n0120 = result
            .as_ref()
            .ok()
            .and_then(|r| r.get("stop"))
            .is_some_and(|s| {
                s.get("kind").and_then(Value::as_str) == Some("fault")
                    && s.get("address")
                        .and_then(Value::as_u32)
                        .is_some_and(|a| a >> 24 == 0x24)
            });
        if wants_n0120 {
            result = attempt("n0120", None).or_else(|_| attempt("n0120", Some(&[])));
        }
        let name = Path::new(path)
            .file_name()
            .map_or(path.clone(), |n| n.to_string_lossy().into_owned());
        let entry = match result {
            Ok(r) => r,
            Err(e) => Value::object().with("error", e),
        };
        let kind = entry
            .get("stop")
            .and_then(|s| s.get("kind"))
            .and_then(Value::as_str);
        if kind == Some("fault") || kind.is_none() {
            faults += 1;
        }
        results.push(entry.with("app", name));
    }
    if args.json {
        println!("{}", Value::object().with("apps", results));
    } else {
        for r in &results {
            let stop = r
                .get("stop")
                .and_then(|s| s.get("kind"))
                .and_then(Value::as_str)
                .unwrap_or("erreur");
            println!(
                "{:42} {:10} images={:<8} instructions={:<10} écran={}",
                r.get("app").and_then(Value::as_str).unwrap_or(""),
                stop,
                r.get("frames").map_or(String::new(), |v| v.to_string()),
                r.get("instructions")
                    .map_or(String::new(), |v| v.to_string()),
                r.get("screen_crc32").and_then(Value::as_str).unwrap_or("-"),
            );
        }
    }
    Ok(i32::from(faults > 0) * 3)
}

// ---- session -------------------------------------------------------------------------------------

/// Remplace les chemins de fichiers d'une requête par leur contenu, et inversement pour la réponse.
fn read_paths(request: &mut Value) -> Result<(), String> {
    for (path_key, content_key) in [
        ("nwa_path", "nwa_base64"),
        ("data_path", "data_base64"),
        ("from_path", "base64"),
    ] {
        if let Some(path) = request
            .get(path_key)
            .and_then(Value::as_str)
            .map(str::to_owned)
        {
            request.set(content_key, json::base64_encode(&read_file(&path)?));
        }
    }
    if request.get("save_png").is_some() {
        // Le PNG est demandé pour l'écrire dans le fichier ; il ne reste dans la réponse que si
        // l'appelant l'avait réclamé lui-même.
        let asked = request.get("rgba_base64").is_some();
        request.set("rgba_base64", true);
        request.set("_strip_rgba", !asked);
    }
    Ok(())
}

fn write_paths(request: &Value, reply: &mut Value) -> Result<(), String> {
    if reply.get("ok").and_then(Value::as_bool) != Some(true) {
        return Ok(());
    }
    if let Some(path) = request.get("save_png").and_then(Value::as_str) {
        write_file(path, &png_from_reply(reply)?)?;
        reply.set("saved", path);
        if request.get("_strip_rgba").and_then(Value::as_bool) == Some(true)
            && let Value::Object(entries) = reply
        {
            entries.remove("rgba_base64");
        }
    }
    if let Some(path) = request.get("save_path").and_then(Value::as_str) {
        let content = reply.get("base64").and_then(Value::as_str).unwrap_or("");
        write_file(
            path,
            &json::base64_decode(content).ok_or("contenu illisible")?,
        )?;
        reply.set("saved", path);
    }
    Ok(())
}

/// Une requête JSON par ligne sur l'entrée standard, une réponse par ligne sur la sortie.
pub fn session(_args: SessionArgs) -> Outcome {
    let mut session = Session::new();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match json::parse(&line) {
            Err(e) => Value::object()
                .with("ok", false)
                .with("error", format!("JSON invalide : {e}")),
            Ok(request) if request.get("cmd").and_then(Value::as_str) == Some("quit") => break,
            Ok(mut request) => match read_paths(&mut request) {
                Err(e) => Value::object().with("ok", false).with("error", e),
                Ok(()) => {
                    let mut reply = session.handle(&request);
                    if let Err(e) = write_paths(&request, &mut reply) {
                        reply = Value::object().with("ok", false).with("error", e);
                    }
                    reply
                }
            },
        };
        writeln!(stdout, "{reply}").map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
    }
    Ok(0)
}

// ---- keys ----------------------------------------------------------------------------------------

pub fn keys(args: KeysArgs) -> Outcome {
    let list: Vec<Value> = Key::ALL
        .iter()
        .map(|k| Value::object().with("name", k.name()).with("bit", k.bit()))
        .collect();
    print(args.json, &Value::object().with("keys", list), || {
        Key::ALL
            .iter()
            .map(|k| format!("{:>2}  {}", k.bit(), k.name()))
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(0)
}
