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

