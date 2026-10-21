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
    request(server, &format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n"))
}

#[test]
fn serves_a_fallback_page_and_health() {
    let server = start(&[]);
    let (code, body) = get(&server, "/");
    assert_eq!(code, 200);
    assert!(String::from_utf8_lossy(&body).contains("nwasabi"));
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
    let nwa = json::base64_decode(v.get("nwa_base64").and_then(Value::as_str).unwrap()).unwrap();
    assert_eq!(nwa, [1, 2, 3, 250]);
    assert_eq!(v.get("data_base64"), Some(&Value::Null));
    let (_, page) = get(&server, "/");
    assert!(String::from_utf8_lossy(&page).contains("demo.nwa"));
}

