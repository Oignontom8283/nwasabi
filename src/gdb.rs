//! Serveur GDB : un débogueur (GDB, LLDB, VS Code…) se branche par TCP comme sur une sonde ST-Link et pilote un appareil émulé, par les
//! commandes JSON de la session : soit une [`Session`] du même processus, soit celle de la page web, par un [`Bridge`].
//!
//! Le protocole est le « GDB Remote Serial Protocol » : des paquets `$données#somme`. Le sous-ensemble géré suffit pour poser des
//! points d'arrêt et d'observation, avancer pas à pas, lire et écrire registres et mémoire, et reprendre ou interrompre l'exécution.
//!
//! Tout passe par les commandes JSON de la session (`run`, `step`, `break`, `watch`, `cpu`, `memory_read`…) : le débogueur voit
//! exactement ce que voit l'interface web, sans second chemin d'exécution à maintenir.

use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::json::{self, Value, ValueExt};
use crate::session::Session;

/// Taille maximale d'un paquet annoncée au débogueur.
const PACKET_SIZE: usize = 0x4000;
/// Instructions exécutées entre deux vérifications d'une demande d'interruption (Ctrl+C).
const SLICE: u64 = 200_000;
/// Registres dans l'ordre du paquet `g` : r0 à r12, sp, lr, pc, xpsr.
const REGISTER_COUNT: usize = 17;

/// La description des registres donnée au débogueur (profil M d'ARM, numérotation simple 0 à 16).
const TARGET_XML: &str = r#"<?xml version="1.0"?>
<!DOCTYPE target SYSTEM "gdb-target.dtd">
<target version="1.0">
  <architecture>arm</architecture>
  <feature name="org.gnu.gdb.arm.m-profile">
    <reg name="r0" bitsize="32" regnum="0"/>
    <reg name="r1" bitsize="32" regnum="1"/>
    <reg name="r2" bitsize="32" regnum="2"/>
    <reg name="r3" bitsize="32" regnum="3"/>
    <reg name="r4" bitsize="32" regnum="4"/>
    <reg name="r5" bitsize="32" regnum="5"/>
    <reg name="r6" bitsize="32" regnum="6"/>
    <reg name="r7" bitsize="32" regnum="7"/>
    <reg name="r8" bitsize="32" regnum="8"/>
    <reg name="r9" bitsize="32" regnum="9"/>
    <reg name="r10" bitsize="32" regnum="10"/>
    <reg name="r11" bitsize="32" regnum="11"/>
    <reg name="r12" bitsize="32" regnum="12"/>
    <reg name="sp" bitsize="32" type="data_ptr" regnum="13"/>
    <reg name="lr" bitsize="32" regnum="14"/>
    <reg name="pc" bitsize="32" type="code_ptr" regnum="15"/>
    <reg name="xpsr" bitsize="32" regnum="16"/>
  </feature>
</target>
"#;

/// Somme de contrôle d'un paquet : la somme des octets modulo 256.
fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |sum, &b| sum.wrapping_add(b))
}

fn hex_byte(b: u8) -> [u8; 2] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    [DIGITS[usize::from(b >> 4)], DIGITS[usize::from(b & 15)]]
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .flat_map(|&b| hex_byte(b))
        .map(char::from)
        .collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok())
        .collect()
}

fn word_hex(value: u32) -> String {
    to_hex(&value.to_le_bytes())
}

/// Un point d'observation posé par le débogueur : (sorte `Z2`/`Z3`/`Z4`, adresse, longueur) → identifiant du cœur.
type WatchKey = (u8, u32, u32);

/// Ce que pilote le serveur GDB : quelque chose qui répond à des commandes JSON de session, une ligne par commande.
pub trait Debuggee {
    /// Exécute une commande (une ligne JSON) et rend la réponse (une ligne JSON).
    fn exchange(&mut self, request: &str) -> String;
}

impl Debuggee for Session {
    fn exchange(&mut self, request: &str) -> String {
        self.handle_line(request)
    }
}

/// Un client GDB connecté à un appareil émulé.
struct Client<'a> {
    stream: TcpStream,
    session: &'a mut dyn Debuggee,
    watches: HashMap<WatchKey, u32>,
    /// Octets lus du réseau, pas encore formant un paquet.
    pending: Vec<u8>,
    /// Pas d'accusés de réception (`QStartNoAckMode`).
    no_ack: bool,
}

impl Client<'_> {
    /// Exécute une commande de la session et rend sa réponse.
    fn ask(&mut self, request: &str) -> Value {
        json::parse(&self.session.exchange(request)).unwrap_or_else(|_| Value::object())
    }

    fn ok(reply: &Value) -> bool {
        reply.get("ok") == Some(&Value::Bool(true))
    }

    fn send(&mut self, data: &str) -> io::Result<()> {
        let packet = format!("${data}#{:02x}", checksum(data.as_bytes()));
        self.stream.write_all(packet.as_bytes())
    }

    /// Lit des octets du réseau ; `block` : attendre (sinon rend `Ok(false)` s'il n'y a rien).
    fn fill(&mut self, block: bool) -> io::Result<bool> {
        self.stream.set_read_timeout(if block {
            None
        } else {
            Some(Duration::from_millis(1))
        })?;
        let mut buffer = [0u8; 4096];
        match self.stream.read(&mut buffer) {
            Ok(0) => Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                self.pending.extend_from_slice(&buffer[..n]);
                Ok(true)
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Le prochain paquet complet (sans `$`, `#` ni somme), ou `None` s'il manque des octets. Traite au passage les `+`, `-`
    /// et l'octet d'interruption `0x03`.
    fn take_packet(&mut self) -> Option<String> {
        loop {
            let start = self.pending.iter().position(|&b| b == b'$' || b == 0x03)?;
            if self.pending[start] == 0x03 {
                self.pending.drain(..=start);
                return Some(String::from("\u{3}"));
            }
            let hash = self.pending[start..].iter().position(|&b| b == b'#')? + start;
            if self.pending.len() < hash + 3 {
                return None;
            }
            let body = self.pending[start + 1..hash].to_vec();
            let sum = std::str::from_utf8(&self.pending[hash + 1..hash + 3])
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok());
            self.pending.drain(..hash + 3);
            if sum == Some(checksum(&body)) {
                if !self.no_ack {
                    let _ = self.stream.write_all(b"+");
                }
                return Some(String::from_utf8_lossy(&body).into_owned());
            }
            if !self.no_ack {
                let _ = self.stream.write_all(b"-");
            }
        }
    }

    /// Attend le prochain paquet, en ignorant les accusés de réception.
    fn read_packet(&mut self) -> io::Result<String> {
        loop {
            while matches!(self.pending.first(), Some(b'+' | b'-')) {
                self.pending.remove(0);
            }
            if let Some(packet) = self.take_packet() {
                return Ok(packet);
            }
            self.fill(true)?;
        }
    }

    /// Pendant l'exécution : le débogueur a-t-il demandé l'interruption ?
    fn interrupt_requested(&mut self) -> io::Result<bool> {
        self.fill(false)?;
        if self.pending.contains(&0x03) {
            self.pending.retain(|&b| b != 0x03);
            return Ok(true);
        }
        Ok(false)
    }

    fn registers(&mut self) -> Option<[u32; REGISTER_COUNT]> {
        let cpu = self.ask(r#"{"cmd":"cpu"}"#);
        if !Self::ok(&cpu) {
            return None;
        }
        let mut out = [0u32; REGISTER_COUNT];
        for (i, v) in cpu.get("r")?.as_array()?.iter().enumerate().take(13) {
            out[i] = v.as_u32()?;
        }
        out[13] = cpu.get("sp")?.as_u32()?;
        out[14] = cpu.get("lr")?.as_u32()?;
        out[15] = cpu.get("pc")?.as_u32()?;
        out[16] = cpu.get("xpsr")?.as_u32()?;
        Some(out)
    }

    fn set_register(&mut self, index: usize, value: u32) -> bool {
        let name = match index {
            0..=12 => format!("r{index}"),
            13 => "sp".to_owned(),
            14 => "lr".to_owned(),
            15 => "pc".to_owned(),
            16 => "xpsr".to_owned(),
            _ => return false,
        };
        let reply = self.ask(&format!(
            r#"{{"cmd":"cpu_set","register":"{name}","value":{value}}}"#
        ));
        Self::ok(&reply)
    }

    fn read_memory(&mut self, address: u32, length: u32) -> Option<Vec<u8>> {
        let reply = self.ask(&format!(
            r#"{{"cmd":"memory_read","address":{address},"length":{length}}}"#
        ));
        if !Self::ok(&reply) {
            return None;
        }
        json::hex_decode(reply.get("hex")?.as_str()?)
    }

    /// Réponse d'arrêt (`T05…`, `S0b`, `W00`) d'après la raison d'arrêt de la session.
    fn stop_reply(&mut self, stop: &Value) -> String {
        let kind = stop.get("kind").and_then(Value::as_str).unwrap_or("");
        let regs = self.registers().unwrap_or([0; REGISTER_COUNT]);
        // Les registres utiles au débogueur sont joints : il évite ainsi de les redemander (sp = 0d, lr = 0e, pc = 0f).
        let extra = format!(
            "0d:{};0e:{};0f:{};",
            word_hex(regs[13]),
            word_hex(regs[14]),
            word_hex(regs[15])
        );
        match kind {
            "exited" => format!(
                "W{:02x}",
                stop.get("code").and_then(Value::as_i64).unwrap_or(0) & 0xff
            ),
            "breakpoint" => format!("T05swbreak:;{extra}"),
            "watchpoint" => {
                let address = stop.get("address").and_then(Value::as_u32).unwrap_or(0);
                let write = stop.get("write").and_then(Value::as_bool).unwrap_or(true);
                let name = if write { "watch" } else { "rwatch" };
                format!("T05{name}:{address:x};{extra}")
            }
            "fault" => {
                // SIGSEGV pour un accès mémoire invalide, SIGILL pour une instruction interdite, SIGTRAP pour `bkpt`.
                let signal = match stop.get("fault").and_then(Value::as_str) {
                    Some("bus") => 0x0b,
                    Some("breakpoint") => 0x05,
                    _ => 0x04,
                };
                format!("T{signal:02x}{extra}")
            }
            "home_pressed" | "reset" => String::from("S0f"),
            _ => format!("T05{extra}"),
        }
    }

    /// `bs` et `bc` : en arrière d'une instruction, ou jusqu'au point d'arrêt ou d'observation précédent. Au début de l'historique,
    /// la réponse porte `replaylog:begin`, que GDB comprend comme « on ne peut pas remonter plus loin ».
    fn reverse(&mut self, request: &str) -> String {
        let reply = self.ask(request);
        if !Self::ok(&reply) {
            let begin = reply
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|e| e.contains("pas d'historique") || e.contains("déjà au début"));
            return if begin {
                String::from("T05replaylog:begin;")
            } else {
                String::from("E01")
            };
        }
        let stop = reply.get("stop").cloned().unwrap_or_else(Value::object);
        match stop.get("kind").and_then(Value::as_str) {
            Some("history_start") => String::from("T05replaylog:begin;"),
            Some("breakpoint" | "watchpoint") => self.stop_reply(&stop),
            _ => self.stop_reply(&Value::object().with("kind", "step")),
        }
    }

    /// `c` : reprend jusqu'à un arrêt qui intéresse le débogueur, ou jusqu'à son interruption.
    fn resume(&mut self) -> io::Result<String> {
        loop {
            if self.interrupt_requested()? {
                return Ok(String::from("T02"));
            }
            let reply = self.ask(&format!(r#"{{"cmd":"run","instructions":{SLICE}}}"#));
            if !Self::ok(&reply) {
                return Ok(String::from("E01"));
            }
            let stop = reply.get("stop").cloned().unwrap_or_else(Value::object);
            match stop.get("kind").and_then(Value::as_str).unwrap_or("") {
                "budget" | "frame" | "sleep" => {}
                // Une application qui attend une touche : le temps passe, sans occuper tout le processeur.
                "wait_event" => {
                    self.ask(r#"{"cmd":"advance","ms":16}"#);
                    std::thread::sleep(Duration::from_millis(4));
                }
                _ => return Ok(self.stop_reply(&stop)),
            }
        }
    }

    fn step(&mut self) -> String {
        let reply = self.ask(r#"{"cmd":"step"}"#);
        let stop = reply.get("stop").cloned().unwrap_or_else(Value::object);
        match stop.get("kind").and_then(Value::as_str).unwrap_or("") {
            "budget" | "frame" | "sleep" | "wait_event" => {
                self.stop_reply(&Value::object().with("kind", "step"))
            }
            _ => self.stop_reply(&stop),
        }
    }

    /// `Z`/`z` : pose ou retire un point d'arrêt ou d'observation.
    fn breakpoint_packet(&mut self, set: bool, body: &str) -> String {
        let mut parts = body.split(',');
        let (Some(kind), Some(address), Some(length)) = (parts.next(), parts.next(), parts.next())
        else {
            return String::from("E01");
        };
        let (Ok(kind), Ok(address)) = (kind.parse::<u8>(), u32::from_str_radix(address, 16)) else {
            return String::from("E01");
        };
        let length = u32::from_str_radix(length.split(';').next().unwrap_or("0"), 16).unwrap_or(1);
        match kind {
            // Points d'arrêt logiciels et matériels : la session les traite pareil.
            0 | 1 => {
                let cmd = if set { "break" } else { "unbreak" };
                let reply = self.ask(&format!(r#"{{"cmd":"{cmd}","address":{address}}}"#));
                if Self::ok(&reply) { "OK" } else { "E01" }.to_owned()
            }
            2..=4 => {
                let key = (kind, address, length);
                if set {
                    let sort = ["write", "read", "readwrite"][usize::from(kind - 2)];
                    let reply = self.ask(&format!(
                        r#"{{"cmd":"watch","address":{address},"length":{length},"kind":"{sort}","stop":true}}"#
                    ));
                    match reply.get("id").and_then(Value::as_u32) {
                        Some(id) => {
                            self.watches.insert(key, id);
                            "OK".to_owned()
                        }
                        None => "E01".to_owned(),
                    }
                } else if let Some(id) = self.watches.remove(&key) {
                    self.ask(&format!(r#"{{"cmd":"unwatch","id":{id}}}"#));
                    "OK".to_owned()
                } else {
                    "E01".to_owned()
                }
            }
            _ => String::new(),
        }
    }

    /// Répond à un paquet ; `None` : pas de réponse (le paquet `k` ferme la connexion).
    fn answer(&mut self, packet: &str) -> io::Result<Option<String>> {
        let (head, rest) = packet.split_at(packet.chars().next().map_or(0, char::len_utf8));
        let reply = match head {
            "?" => String::from("T05"),
            "g" => match self.registers() {
                Some(r) => r.iter().map(|&v| word_hex(v)).collect(),
                None => String::from("E01"),
            },
            "G" => match from_hex(rest) {
                Some(bytes) => {
                    for (i, chunk) in bytes.chunks_exact(4).enumerate().take(REGISTER_COUNT) {
                        self.set_register(
                            i,
                            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                        );
                    }
                    String::from("OK")
                }
                None => String::from("E01"),
            },
            "p" => match (usize::from_str_radix(rest, 16), self.registers()) {
                (Ok(i), Some(r)) if i < REGISTER_COUNT => word_hex(r[i]),
                _ => String::from("E01"),
            },
            "P" => {
                let Some((index, value)) = rest.split_once('=') else {
                    return Ok(Some(String::from("E01")));
                };
                match (usize::from_str_radix(index, 16), from_hex(value)) {
                    (Ok(i), Some(b))
                        if b.len() == 4
                            && self
                                .set_register(i, u32::from_le_bytes([b[0], b[1], b[2], b[3]])) =>
                    {
                        String::from("OK")
                    }
                    _ => String::from("E01"),
                }
            }
            "m" => match rest.split_once(',').and_then(|(a, l)| {
                Some((
                    u32::from_str_radix(a, 16).ok()?,
                    u32::from_str_radix(l, 16).ok()?,
                ))
            }) {
                Some((address, length)) => {
                    match self.read_memory(address, length.min(PACKET_SIZE as u32 / 2)) {
                        Some(bytes) => to_hex(&bytes),
                        None => String::from("E14"),
                    }
                }
                None => String::from("E01"),
            },
            "M" => {
                let parsed = rest.split_once(':').and_then(|(head, data)| {
                    let (a, _) = head.split_once(',')?;
                    Some((u32::from_str_radix(a, 16).ok()?, data))
                });
                match parsed {
                    Some((address, data)) => {
                        let reply = self.ask(&format!(
                            r#"{{"cmd":"memory_write","address":{address},"hex":"{data}"}}"#
                        ));
                        if Self::ok(&reply) { "OK" } else { "E14" }.to_owned()
                    }
                    None => String::from("E01"),
                }
            }
            "Z" => self.breakpoint_packet(true, rest),
            "z" => self.breakpoint_packet(false, rest),
            "b" => match rest {
                "s" => self.reverse(r#"{"cmd":"step_back"}"#),
                "c" => self.reverse(r#"{"cmd":"reverse_continue"}"#),
                _ => String::new(),
            },
            "c" => self.resume()?,
            "s" => self.step(),
            "v" => {
                if rest.starts_with("Cont?") {
                    String::from("vCont;c;s;t")
                } else if let Some(actions) = rest.strip_prefix("Cont;") {
                    match actions.chars().next() {
                        Some('s') => self.step(),
                        Some('c') => self.resume()?,
                        _ => String::from("E01"),
                    }
                } else {
                    // `vMustReplyEmpty` et les autres : la réponse vide dit « non pris en charge ».
                    String::new()
                }
            }
            "H" | "T" => String::from("OK"),
            "D" => {
                self.send("OK")?;
                return Ok(None);
            }
            "k" => return Ok(None),
            "q" => self.query(rest),
            "\u{3}" => String::from("T02"),
            _ => String::new(),
        };
        Ok(Some(reply))
    }

    fn query(&mut self, rest: &str) -> String {
        if rest.starts_with("Supported") {
            format!(
                "PacketSize={PACKET_SIZE:x};qXfer:features:read+;swbreak+;hwbreak+;vContSupported+;QStartNoAckMode+;ReverseStep+;ReverseContinue+"
            )
        } else if rest == "Attached" {
            String::from("1")
        } else if rest == "C" {
            String::from("QC1")
        } else if rest == "fThreadInfo" {
            String::from("m1")
        } else if rest == "sThreadInfo" {
            String::from("l")
        } else if let Some(args) = rest.strip_prefix("Xfer:features:read:target.xml:") {
            let (offset, length) = args.split_once(',').unwrap_or(("0", "0"));
            let offset = usize::from_str_radix(offset, 16).unwrap_or(0);
            let length = usize::from_str_radix(length, 16).unwrap_or(0);
            let xml = TARGET_XML.as_bytes();
            let end = (offset + length).min(xml.len());
            let chunk = String::from_utf8_lossy(&xml[offset.min(xml.len())..end]).into_owned();
            format!("{}{chunk}", if end >= xml.len() { 'l' } else { 'm' })
        } else {
            String::new()
        }
    }

    fn run(&mut self) -> io::Result<()> {
        loop {
            let packet = self.read_packet()?;
            if packet == "QStartNoAckMode" {
                self.send("OK")?;
                self.no_ack = true;
                continue;
            }
            match self.answer(&packet)? {
                Some(reply) => self.send(&reply)?,
                None => return Ok(()),
            }
        }
    }
}

/// Sert un débogueur sur `stream` jusqu'à sa déconnexion. La session garde son état (points d'arrêt compris) entre deux connexions.
pub fn serve_connection(session: &mut dyn Debuggee, stream: TcpStream) -> io::Result<()> {
    stream.set_nodelay(true)?;
    // Le débogueur peut remonter le temps (reverse-step, reverse-continue) : l'historique tourne pendant la connexion.
    let recording = session
        .exchange(r#"{"cmd":"history"}"#)
        .contains(r#""recording":true"#);
    if !recording {
        session.exchange(r#"{"cmd":"history","action":"start"}"#);
    }
    let mut client = Client {
        stream,
        session,
        watches: HashMap::new(),
        pending: Vec::new(),
        no_ack: false,
    };
    let result = client.run();
    if !recording {
        client.ask(r#"{"cmd":"history","action":"stop"}"#);
    }
    // À la déconnexion, les points posés par ce débogueur ne restent pas.
    for (_, id) in client.watches.drain().collect::<Vec<_>>() {
        client.ask(&format!(r#"{{"cmd":"unwatch","id":{id}}}"#));
    }
    match result {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
        other => other,
    }
}

// ---- l'appareil de la page web ----------------------------------------------------------------------------------------------------

/// Combien de temps sans interrogation de la page avant de la croire partie.
const ATTACH_TIMEOUT: Duration = Duration::from_secs(25);
/// Combien de temps le débogueur attend la réponse de la page à une commande.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default)]
struct BridgeState {
    next_id: u64,
    requests: VecDeque<(u64, String)>,
    answers: HashMap<u64, String>,
    last_poll: Option<Instant>,
    clients: usize,
}

/// Le pont entre un débogueur GDB et l'émulation de la page web : l'émulation tourne dans le navigateur, pas dans le serveur. Le
/// débogueur (côté serveur) dépose ses commandes, la page les prend (`next_request`), les exécute et rend la réponse (`answer`).
#[derive(Default)]
pub struct Bridge {
    state: Mutex<BridgeState>,
    request_ready: Condvar,
    answer_ready: Condvar,
}

impl Bridge {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BridgeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// La page interroge-t-elle le pont (elle accepte un débogueur) ?
    pub fn attached(&self) -> bool {
        self.lock()
            .last_poll
            .is_some_and(|t| t.elapsed() < ATTACH_TIMEOUT)
    }

    /// Nombre de débogueurs connectés.
    pub fn clients(&self) -> usize {
        self.lock().clients
    }

    pub fn client_connected(&self) {
        self.lock().clients += 1;
    }

    pub fn client_gone(&self) {
        let mut state = self.lock();
        state.clients = state.clients.saturating_sub(1);
    }

    /// Côté page : la prochaine commande du débogueur, attendue au plus `wait`. Chaque appel dit que la page est là.
    pub fn next_request(&self, wait: Duration) -> Option<(u64, String)> {
        let mut state = self.lock();
        state.last_poll = Some(Instant::now());
        if state.requests.is_empty() {
            state = self
                .request_ready
                .wait_timeout(state, wait)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        state.last_poll = Some(Instant::now());
        state.requests.pop_front()
    }

    /// Côté page : la réponse à la commande `id`.
    pub fn answer(&self, id: u64, reply: String) {
        self.lock().answers.insert(id, reply);
        self.answer_ready.notify_all();
    }

    /// Côté page : elle n'accepte plus de débogueur.
    pub fn detach(&self) {
        self.lock().last_poll = None;
    }

    fn error(message: &str) -> String {
        Value::object()
            .with("ok", false)
            .with("error", message)
            .to_string()
    }

    /// Côté débogueur : envoie une commande à la page et attend sa réponse.
    fn exchange(&self, request: &str) -> String {
        let mut state = self.lock();
        if state
            .last_poll
            .is_none_or(|t| t.elapsed() >= ATTACH_TIMEOUT)
        {
            return Self::error(
                "la page de l'émulateur n'accepte pas de débogueur (activez-le dans l'onglet Débogage)",
            );
        }
        let id = state.next_id;
        state.next_id += 1;
        state.requests.push_back((id, request.to_owned()));
        self.request_ready.notify_all();
        let deadline = Instant::now() + ANSWER_TIMEOUT;
        loop {
            if let Some(reply) = state.answers.remove(&id) {
                return reply;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                state.requests.retain(|(i, _)| *i != id);
                return Self::error("la page n'a pas répondu");
            }
            state = self
                .answer_ready
                .wait_timeout(state, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
}

/// L'appareil de la page, vu par le serveur GDB.
pub struct PageDebuggee(pub Arc<Bridge>);

impl Debuggee for PageDebuggee {
    fn exchange(&mut self, request: &str) -> String {
        self.0.exchange(request)
    }
}
