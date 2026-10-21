//! Un serveur HTTP minimal (GET seulement), sans dépendance.
//!
//! Il n'a besoin que de servir quelques fichiers embarqués : une requête, une réponse, la
//! connexion est fermée ensuite.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Une réponse prête à être envoyée.
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type,
            body,
        }
    }

    pub fn text(status: u16, message: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: message.as_bytes().to_vec(),
        }
    }

    fn reason(&self) -> &'static str {
        match self.status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            405 => "Method Not Allowed",
            _ => "Error",
        }
    }
}

/// Type MIME d'après l'extension de l'URL.
pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Lit la ligne de requête et rend `(méthode, chemin sans paramètres)`. Les en-têtes sont lus
/// puis ignorés ; leur taille totale est bornée.
fn read_request(stream: &TcpStream) -> Option<(String, String)> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut first = String::new();
    reader.by_ref().take(8192).read_line(&mut first).ok()?;
    let mut total = 0;
    loop {
        let mut line = String::new();
        let n = reader.by_ref().take(8192).read_line(&mut line).ok()?;
        total += n;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if total > 64 * 1024 {
            return None;
        }
    }
    let mut parts = first.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?;
    let path = target.split(['?', '#']).next()?.to_owned();
    Some((method, path))
}

use std::io::Read;

/// Traite une connexion avec `route(chemin)`.
pub fn serve_connection(mut stream: TcpStream, route: &dyn Fn(&str) -> Response) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let response = match read_request(&stream) {
        None => Response::text(400, "requête illisible"),
        Some((method, path)) if method == "GET" || method == "HEAD" => {
            let mut r = route(&path);
            if method == "HEAD" {
                r.body.clear();
            }
            r
        }
        Some(_) => Response::text(405, "seul GET est accepté"),
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        response.status,
        response.reason(),
        response.content_type,
        response.body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}

