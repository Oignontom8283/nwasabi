//! Le binaire `nwasabi` (serveur web), lancé sur un port libre.

#![cfg(feature = "serve")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

use nwasabi::json::{self, Value};

struct Server {
    child: Child,
    address: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start(args: &[&str]) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nwasabi"))
        .args(["--port", "0"])
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("lancement de nwasabi");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let address = line
        .trim()
        .strip_prefix("http://")
        .and_then(|a| a.strip_suffix('/'))
        .unwrap_or_else(|| panic!("première ligne inattendue : {line:?}"))
        .to_owned();
    Server { child, address }
}

/// Envoie une requête brute et rend (code, corps).
fn request(server: &Server, raw: &str) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(&server.address).unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).unwrap();
    let split = reply
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("fin des en-têtes");
    let head = String::from_utf8_lossy(&reply[..split]).into_owned();
    let code = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    (code, reply[split + 4..].to_vec())
}

fn get(server: &Server, path: &str) -> (u16, Vec<u8>) {
    request(
        server,
        &format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"),
    )
}

#[test]
fn serves_the_site_and_health() {
    let server = start(&[]);
    // Le site pré-construit (`web-dist/`) est embarqué dans le binaire.
    let (code, body) = get(&server, "/");
    assert_eq!(code, 200);
    let page = String::from_utf8_lossy(&body);
    assert!(page.contains("<title>Nwasabi</title>"), "{page}");
    let script = page
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let (code, body) = get(&server, script.trim_start_matches('.'));
    assert_eq!(code, 200, "{script}");
    assert!(!body.is_empty());
    let (code, body) = get(&server, "/__nwasabi/health");
    assert_eq!((code, body.as_slice()), (200, &b"{\"ok\":true}"[..]));
    let (_, body) = get(&server, "/__nwasabi/preload");
    assert_eq!(body, b"null");
}

#[test]
fn preloads_the_given_application() {
    let dir = std::env::temp_dir().join(format!("nwasabi-server-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("demo.nwa");
    std::fs::write(&file, [1u8, 2, 3, 250]).unwrap();
    let server = start(&["--nwa", file.to_str().unwrap(), "--model", "N0120"]);

    let (code, body) = get(&server, "/__nwasabi/preload");
    assert_eq!(code, 200);
    let v = json::parse(&String::from_utf8(body).unwrap()).unwrap();
    assert_eq!(v.get("name").and_then(Value::as_str), Some("demo.nwa"));
    assert_eq!(v.get("model").and_then(Value::as_str), Some("n0120"));
    assert_eq!(v.get("firmware").and_then(Value::as_str), Some("24"));
    let nwa = json::base64_decode(v.get("nwa").and_then(Value::as_str).unwrap()).unwrap();
    assert_eq!(nwa, [1, 2, 3, 250]);
    assert_eq!(v.get("data"), Some(&Value::Null));
}

#[test]
fn unknown_paths_and_methods_are_refused_and_nothing_is_read_from_disk() {
    let server = start(&[]);
    assert_eq!(get(&server, "/inexistant.js").0, 404);
    assert_eq!(get(&server, "/../Cargo.toml").0, 404);
    assert_eq!(get(&server, "/%2e%2e/Cargo.toml").0, 404);
    assert_eq!(
        request(
            &server,
            "POST / HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
        )
        .0,
        405
    );
    // Les paramètres d'URL ne changent pas la route.
    assert_eq!(get(&server, "/__nwasabi/health?x=1").0, 200);
}

#[test]
fn bad_arguments_are_reported() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_nwasabi"))
            .args(args)
            .output()
            .unwrap()
    };
    // Mauvais usage : code 2 (clap).
    assert_eq!(run(&["--model", "n9999"]).status.code(), Some(2));
    assert_eq!(
        run(&["--data", "x"]).status.code(),
        Some(2),
        "--data demande --nwa"
    );
    assert_eq!(run(&["--firmware", "1.0"]).status.code(), Some(2));
    // Fichier introuvable : erreur d'exécution, code 1.
    let out = run(&["--nwa", "/inexistant.nwa"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("/inexistant.nwa"));
}

#[test]
fn nmk_serves_the_checked_mapping_file() {
    let path = format!("{}/tests/nmk/wasd.nmk", env!("CARGO_MANIFEST_DIR"));
    let server = start(&["--nmk", &path]);
    let (code, body) = get(&server, "/__nwasabi/nmk");
    assert_eq!(code, 200);
    assert_eq!(body, std::fs::read(&path).unwrap());

    let without = start(&[]);
    assert_eq!(get(&without, "/__nwasabi/nmk").0, 404);
}

#[test]
fn an_invalid_nmk_stops_the_server_at_startup() {
    let path = format!(
        "{}/tests/nmk/error_while_held.nmk",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_nwasabi"))
        .args(["--port", "0", "--nmk", &path])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("touch.rules[0].while_held"), "{err}");
}

#[test]
fn nfs_and_nds_are_served_after_checking() {
    let nfs = format!("{}/nfs-test.json", std::env::temp_dir().display());
    std::fs::write(&nfs, r#"{"nfs":1,"files":[]}"#).unwrap();
    let server = start(&["--nfs", &nfs]);
    assert_eq!(
        get(&server, "/__nwasabi/nfs").1,
        std::fs::read(&nfs).unwrap()
    );
    assert_eq!(get(&server, "/__nwasabi/nds").0, 404);

    let bad = format!("{}/nfs-bad.json", std::env::temp_dir().display());
    std::fs::write(
        &bad,
        r#"{"nfs":1,"files":[{"name":"sans_extension","content":""}]}"#,
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_nwasabi"))
        .args(["--port", "0", "--nfs", &bad])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("files[0]"));

    let nds = format!("{}/nds-bad.json", std::env::temp_dir().display());
    std::fs::write(&nds, r#"{"nds":1}"#).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_nwasabi"))
        .args(["--port", "0", "--nds", &nds])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn given_files_are_read_again_and_their_stamp_changes_with_them() {
    let dir = std::env::temp_dir().join(format!("nwasabi-watch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let nwa = dir.join("demo.nwa");
    let nfs = dir.join("files.nfs");
    std::fs::write(&nwa, [1u8, 2, 3]).unwrap();
    std::fs::write(&nfs, r#"{"nfs":1,"files":[]}"#).unwrap();
    let server = start(&[
        "--nwa",
        nwa.to_str().unwrap(),
        "--nfs",
        nfs.to_str().unwrap(),
    ]);

    let stamps = |server: &Server| {
        let (code, body) = get(server, "/__nwasabi/version");
        assert_eq!(code, 200);
        json::parse(&String::from_utf8(body).unwrap()).unwrap()
    };
    let before = stamps(&server);
    assert!(before.get("nwa").and_then(Value::as_str).is_some());
    assert_eq!(before.get("nmk"), Some(&Value::Null));
    assert_eq!(stamps(&server), before, "rien n'a changé : même empreinte");

    // La taille change avec le contenu : l'empreinte aussi, même si l'horloge du disque est grossière.
    std::fs::write(&nwa, [9u8, 8, 7, 6, 5]).unwrap();
    let after = stamps(&server);
    assert_ne!(after.get("nwa"), before.get("nwa"));
    assert_eq!(after.get("nfs"), before.get("nfs"));

    let (_, body) = get(&server, "/__nwasabi/preload");
    let v = json::parse(&String::from_utf8(body).unwrap()).unwrap();
    let bytes = json::base64_decode(v.get("nwa").and_then(Value::as_str).unwrap()).unwrap();
    assert_eq!(
        bytes,
        [9, 8, 7, 6, 5],
        "le fichier est relu à chaque requête"
    );

    // Un fichier devenu invalide donne une erreur lisible, pas un arrêt du serveur.
    std::fs::write(&nfs, "pas du json").unwrap();
    let (code, body) = get(&server, "/__nwasabi/nfs");
    assert_eq!(code, 422);
    assert!(String::from_utf8_lossy(&body).contains("JSON invalide"));
    assert_eq!(get(&server, "/__nwasabi/health").0, 200);
}

#[test]
fn flash_is_served_after_checking_and_has_a_stamp() {
    let dir = std::env::temp_dir().join("nwasabi-flash-test");
    std::fs::create_dir_all(&dir).unwrap();
    let good = dir.join("khi.nfd");
    std::fs::write(
        &good,
        r#"{"nfd":1,"segments":[{"address":"0x90400000","content":"AQID"}]}"#,
    )
    .unwrap();
    let server = start(&["--flash", good.to_str().unwrap()]);
    assert_eq!(
        get(&server, "/__nwasabi/flash").1,
        std::fs::read(&good).unwrap()
    );
    let (_, version) = get(&server, "/__nwasabi/version");
    let version: serde_json::Value = serde_json::from_slice(&version).unwrap();
    assert!(version["flash"].is_string(), "{version}");
    assert_eq!(get(&start(&[]), "/__nwasabi/flash").0, 404);

    // Un segment hors de la flash arrête le serveur avant qu'il écoute.
    let bad = dir.join("bad.nfd");
    std::fs::write(
        &bad,
        r#"{"nfd":1,"segments":[{"address":"0x20000000","content":"AQID"}]}"#,
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_nwasabi"))
        .args(["--port", "0", "--flash", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("segments[0]"));
}

#[test]
fn sources_are_served_from_the_given_folder_only() {
    let dir = std::env::temp_dir().join(format!("nwasabi-src-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("proj/src")).unwrap();
    std::fs::write(
        dir.join("proj/src/main.c"),
        "int main(void) { return 0; }\n",
    )
    .unwrap();
    std::fs::write(dir.join("secret.txt"), "pas pour la page").unwrap();
    let server = start(&["--src", dir.join("proj").to_str().unwrap()]);

    // Le chemin des informations de débogage est relatif à la machine qui a compilé : la fin du chemin suffit.
    for name in [
        "src/main.c",
        "/home/moi/travail/proj/src/main.c",
        "main.c",
        "./src/main.c",
    ] {
        let (code, body) = get(&server, &format!("/__nwasabi/source?name={name}"));
        assert_eq!(code, 200, "{name}");
        assert_eq!(body, b"int main(void) { return 0; }\n");
    }
    // Rien d'autre n'est lisible : ni un fichier voisin, ni une remontée de dossier, ni un fichier inconnu.
    for name in [
        "secret.txt",
        "../secret.txt",
        "..%2Fsecret.txt",
        "%2e%2e/secret.txt",
        "src/absent.c",
        "",
    ] {
        assert_eq!(
            get(&server, &format!("/__nwasabi/source?name={name}")).0,
            404,
            "{name}"
        );
    }
    assert_eq!(get(&server, "/__nwasabi/source").0, 404);
    // Sans --src, aucun fichier n'est servi.
    let without = start(&[]);
    assert_eq!(get(&without, "/__nwasabi/source?name=src/main.c").0, 404);
}

/// Envoie une requête brute et rend (en-têtes en minuscules, corps brut).
fn request_with_headers(server: &Server, raw: &str) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(&server.address).unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).unwrap();
    let split = reply
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("fin des en-têtes");
    let head = String::from_utf8_lossy(&reply[..split]).to_lowercase();
    (head, reply[split + 4..].to_vec())
}

/// Recompose un corps envoyé par morceaux (`Transfer-Encoding: chunked`).
fn dechunk(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|w| w == b"\r\n")
            .expect("taille de morceau");
        let size =
            usize::from_str_radix(std::str::from_utf8(&body[..line_end]).unwrap().trim(), 16)
                .unwrap();
        body = &body[line_end + 2..];
        if size == 0 {
            return out;
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

#[test]
fn compresses_the_site_for_browsers_that_accept_gzip() {
    use std::io::Read as _;
    let server = start(&[]);
    // Le premier script de la page : un gros fichier de JavaScript.
    let (_, page) = get(&server, "/");
    let page = String::from_utf8_lossy(&page).into_owned();
    let script = page
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let path = script.trim_start_matches('.');
    let (plain_code, plain) = get(&server, path);
    assert_eq!(plain_code, 200);

    let raw = format!(
        "GET {path} HTTP/1.1\r\nHost: x\r\nAccept-Encoding: gzip\r\nConnection: close\r\n\r\n"
    );
    let (head, body) = request_with_headers(&server, &raw);
    assert!(head.contains("content-encoding: gzip"), "{head}");
    let packed = if head.contains("transfer-encoding: chunked") {
        dechunk(&body)
    } else {
        body
    };
    assert!(
        packed.len() < plain.len() / 2,
        "{} octets compressés pour {}",
        packed.len(),
        plain.len()
    );
    let mut unpacked = Vec::new();
    flate2::read::GzDecoder::new(&packed[..])
        .read_to_end(&mut unpacked)
        .unwrap();
    assert_eq!(
        unpacked, plain,
        "le contenu décompressé est celui qu'on obtient sans compression"
    );

    // Sans `Accept-Encoding`, rien n'est compressé.
    let raw = format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
    let (head, _) = request_with_headers(&server, &raw);
    assert!(!head.contains("content-encoding"), "{head}");
}
