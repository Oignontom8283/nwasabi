//! L'appareil vu par `gdbstub` : la bibliothèque parle le protocole GDB (paquets, sommes, modes sans accusé, XML de la cible,
//! marche arrière…) et appelle les fonctions de ce fichier, qui répondent par les commandes JSON de la session. Le débogueur voit
//! ainsi exactement ce que voit l'interface web.

use std::collections::HashMap;
use std::net::TcpStream;
use std::time::Duration;

use gdbstub::common::Signal;
use gdbstub::conn::ConnectionExt;
use gdbstub::stub::run_blocking::{BlockingEventLoop, Event, WaitForStopReasonError};
use gdbstub::stub::{GdbStub, SingleThreadStopReason};
use gdbstub::target::ext::base::BaseOps;
use gdbstub::target::ext::base::reverse_exec::{ReplayLogPosition, ReverseCont, ReverseStep};
use gdbstub::target::ext::base::singlethread::{
    SingleThreadBase, SingleThreadResume, SingleThreadResumeOps, SingleThreadSingleStep,
};
use gdbstub::target::ext::breakpoints::{
    Breakpoints, BreakpointsOps, HwBreakpoint, HwWatchpoint, SwBreakpoint, WatchKind,
};
use gdbstub::target::{Target, TargetError, TargetResult};

use super::Debuggee;
use super::arch::{CoreRegister, CoreRegisters, CortexM, REGISTER_COUNT};
use crate::json::{self, Value, ValueExt};

/// Instructions exécutées entre deux vérifications d'une demande d'interruption (Ctrl+C).
const SLICE: u64 = 200_000;

/// Ce que le débogueur a demandé de faire au prochain arrêt de l'attente.
enum Action {
    Continue,
    Step,
    ReverseStep,
    ReverseContinue,
}

/// L'appareil piloté par un débogueur.
pub(super) struct Cible<'a> {
    session: &'a mut dyn Debuggee,
    /// Points d'observation posés : (genre, adresse, longueur) → identifiant dans la session.
    watches: HashMap<(u8, u32, u32), u32>,
    action: Option<Action>,
}

impl<'a> Cible<'a> {
    pub(super) fn new(session: &'a mut dyn Debuggee) -> Self {
        Self {
            session,
            watches: HashMap::new(),
            action: None,
        }
    }

    fn ask(&mut self, request: &str) -> Value {
        json::parse(&self.session.exchange(request)).unwrap_or_else(|_| Value::object())
    }

    fn ok(reply: &Value) -> bool {
        reply.get("ok") == Some(&Value::Bool(true))
    }

    /// Les points posés par ce débogueur ne restent pas après lui.
    pub(super) fn forget_watches(&mut self) {
        for (_, id) in self.watches.drain().collect::<Vec<_>>() {
            self.ask(&format!(r#"{{"cmd":"unwatch","id":{id}}}"#));
        }
    }

    pub(super) fn ask_raw(&mut self, request: &str) -> String {
        self.session.exchange(request)
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

    /// Traduit la raison d'arrêt de la session en raison d'arrêt GDB.
    fn stop_reason(stop: &Value) -> SingleThreadStopReason<u32> {
        let kind = stop.get("kind").and_then(Value::as_str).unwrap_or("");
        match kind {
            "exited" => SingleThreadStopReason::Exited(
                (stop.get("code").and_then(Value::as_i64).unwrap_or(0) & 0xff) as u8,
            ),
            "breakpoint" => SingleThreadStopReason::SwBreak(()),
            "watchpoint" => SingleThreadStopReason::Watch {
                tid: (),
                kind: if stop.get("write").and_then(Value::as_bool).unwrap_or(true) {
                    WatchKind::Write
                } else {
                    WatchKind::Read
                },
                addr: stop.get("address").and_then(Value::as_u32).unwrap_or(0),
            },
            // SIGSEGV pour un accès mémoire invalide, SIGILL pour une instruction interdite, SIGTRAP pour `bkpt`.
            "fault" => {
                SingleThreadStopReason::Signal(match stop.get("fault").and_then(Value::as_str) {
                    Some("bus") => Signal::SIGSEGV,
                    Some("breakpoint") => Signal::SIGTRAP,
                    _ => Signal::SIGILL,
                })
            }
            "home_pressed" | "reset" => SingleThreadStopReason::Signal(Signal::SIGTERM),
            _ => SingleThreadStopReason::DoneStep,
        }
    }

    /// Un pas. Une application qui dort ou attend est considérée comme ayant fini son pas.
    fn step_once(&mut self) -> SingleThreadStopReason<u32> {
        let reply = self.ask(r#"{"cmd":"step"}"#);
        let stop = reply.get("stop").cloned().unwrap_or_else(Value::object);
        Self::stop_reason(&stop)
    }

    /// Marche arrière : une instruction, ou jusqu'au point d'arrêt ou d'observation précédent. Au début de l'historique, GDB reçoit
    /// « on ne peut pas remonter plus loin ».
    fn reverse(&mut self, request: &str) -> SingleThreadStopReason<u32> {
        let begin = SingleThreadStopReason::ReplayLog {
            tid: None,
            pos: ReplayLogPosition::Begin,
        };
        let reply = self.ask(request);
        if !Self::ok(&reply) {
            let at_start = reply
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|e| e.contains("pas d'historique") || e.contains("déjà au début"));
            return if at_start {
                begin
            } else {
                SingleThreadStopReason::Signal(Signal::SIGABRT)
            };
        }
        let stop = reply.get("stop").cloned().unwrap_or_else(Value::object);
        match stop.get("kind").and_then(Value::as_str) {
            Some("history_start") => begin,
            _ => Self::stop_reason(&stop),
        }
    }
}

impl Target for Cible<'_> {
    type Arch = CortexM;
    type Error = String;

    fn base_ops(&mut self) -> BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }

    fn support_breakpoints(&mut self) -> Option<BreakpointsOps<'_, Self>> {
        Some(self)
    }

    /// Pas de compression des suites d'octets identiques : les réponses restent lisibles et acceptées par tout débogueur.
    fn use_rle(&self) -> bool {
        false
    }
}

impl SingleThreadBase for Cible<'_> {
    fn read_registers(&mut self, regs: &mut CoreRegisters) -> TargetResult<(), Self> {
        regs.0 = self.registers().ok_or(TargetError::NonFatal)?;
        Ok(())
    }

    fn write_registers(&mut self, regs: &CoreRegisters) -> TargetResult<(), Self> {
        for (i, &value) in regs.0.iter().enumerate() {
            self.set_register(i, value);
        }
        Ok(())
    }

    fn read_addrs(&mut self, start_addr: u32, data: &mut [u8]) -> TargetResult<usize, Self> {
        let reply = self.ask(&format!(
            r#"{{"cmd":"memory_read","address":{start_addr},"length":{}}}"#,
            data.len()
        ));
        if !Self::ok(&reply) {
            return Err(TargetError::NonFatal);
        }
        let bytes = reply
            .get("hex")
            .and_then(Value::as_str)
            .and_then(json::hex_decode)
            .ok_or(TargetError::NonFatal)?;
        let n = bytes.len().min(data.len());
        data[..n].copy_from_slice(&bytes[..n]);
        Ok(n)
    }

    fn write_addrs(&mut self, start_addr: u32, data: &[u8]) -> TargetResult<(), Self> {
        let reply = self.ask(&format!(
            r#"{{"cmd":"memory_write","address":{start_addr},"hex":"{}"}}"#,
            json::hex_encode(data)
        ));
        if Self::ok(&reply) {
            Ok(())
        } else {
            Err(TargetError::NonFatal)
        }
    }

    fn support_resume(&mut self) -> Option<SingleThreadResumeOps<'_, Self>> {
        Some(self)
    }

    fn support_single_register_access(
        &mut self,
    ) -> Option<
        gdbstub::target::ext::base::single_register_access::SingleRegisterAccessOps<'_, (), Self>,
    > {
        Some(self)
    }
}

/// Lire ou écrire un seul registre (paquets `p` et `P`), sans passer par les 17.
impl gdbstub::target::ext::base::single_register_access::SingleRegisterAccess<()> for Cible<'_> {
    fn read_register(
        &mut self,
        _tid: (),
        reg_id: CoreRegister,
        buf: &mut [u8],
    ) -> TargetResult<usize, Self> {
        let registers = self.registers().ok_or(TargetError::NonFatal)?;
        let bytes = registers[reg_id.0].to_le_bytes();
        buf[..4].copy_from_slice(&bytes);
        Ok(4)
    }

    fn write_register(
        &mut self,
        _tid: (),
        reg_id: CoreRegister,
        val: &[u8],
    ) -> TargetResult<(), Self> {
        let bytes: [u8; 4] = val.try_into().map_err(|_| TargetError::NonFatal)?;
        if self.set_register(reg_id.0, u32::from_le_bytes(bytes)) {
            Ok(())
        } else {
            Err(TargetError::NonFatal)
        }
    }
}

impl SingleThreadResume for Cible<'_> {
    fn resume(&mut self, _signal: Option<Signal>) -> Result<(), String> {
        self.action = Some(Action::Continue);
        Ok(())
    }

    fn support_single_step(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::singlethread::SingleThreadSingleStepOps<'_, Self>> {
        Some(self)
    }

    fn support_reverse_step(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::reverse_exec::ReverseStepOps<'_, (), Self>> {
        Some(self)
    }

    fn support_reverse_cont(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::reverse_exec::ReverseContOps<'_, (), Self>> {
        Some(self)
    }
}

impl SingleThreadSingleStep for Cible<'_> {
    fn step(&mut self, _signal: Option<Signal>) -> Result<(), String> {
        self.action = Some(Action::Step);
        Ok(())
    }
}

impl ReverseStep<()> for Cible<'_> {
    fn reverse_step(&mut self, _tid: ()) -> Result<(), String> {
        self.action = Some(Action::ReverseStep);
        Ok(())
    }
}

impl ReverseCont<()> for Cible<'_> {
    fn reverse_cont(&mut self) -> Result<(), String> {
        self.action = Some(Action::ReverseContinue);
        Ok(())
    }
}

impl Breakpoints for Cible<'_> {
    fn support_sw_breakpoint(
        &mut self,
    ) -> Option<gdbstub::target::ext::breakpoints::SwBreakpointOps<'_, Self>> {
        Some(self)
    }

    fn support_hw_breakpoint(
        &mut self,
    ) -> Option<gdbstub::target::ext::breakpoints::HwBreakpointOps<'_, Self>> {
        Some(self)
    }

    fn support_hw_watchpoint(
        &mut self,
    ) -> Option<gdbstub::target::ext::breakpoints::HwWatchpointOps<'_, Self>> {
        Some(self)
    }
}

impl Cible<'_> {
    fn set_breakpoint(&mut self, address: u32, set: bool) -> TargetResult<bool, Self> {
        let cmd = if set { "break" } else { "unbreak" };
        let reply = self.ask(&format!(r#"{{"cmd":"{cmd}","address":{address}}}"#));
        Ok(Self::ok(&reply))
    }
}

// Points d'arrêt logiciels et matériels : la session les traite pareil.
impl SwBreakpoint for Cible<'_> {
    fn add_sw_breakpoint(&mut self, addr: u32, _kind: usize) -> TargetResult<bool, Self> {
        self.set_breakpoint(addr, true)
    }

    fn remove_sw_breakpoint(&mut self, addr: u32, _kind: usize) -> TargetResult<bool, Self> {
        self.set_breakpoint(addr, false)
    }
}

impl HwBreakpoint for Cible<'_> {
    fn add_hw_breakpoint(&mut self, addr: u32, _kind: usize) -> TargetResult<bool, Self> {
        self.set_breakpoint(addr, true)
    }

    fn remove_hw_breakpoint(&mut self, addr: u32, _kind: usize) -> TargetResult<bool, Self> {
        self.set_breakpoint(addr, false)
    }
}

impl HwWatchpoint for Cible<'_> {
    fn add_hw_watchpoint(
        &mut self,
        addr: u32,
        len: u32,
        kind: WatchKind,
    ) -> TargetResult<bool, Self> {
        let (code, sort) = match kind {
            WatchKind::Write => (2u8, "write"),
            WatchKind::Read => (3, "read"),
            WatchKind::ReadWrite => (4, "readwrite"),
        };
        let reply = self.ask(&format!(
            r#"{{"cmd":"watch","address":{addr},"length":{len},"kind":"{sort}","stop":true}}"#
        ));
        match reply.get("id").and_then(Value::as_u32) {
            Some(id) => {
                self.watches.insert((code, addr, len), id);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn remove_hw_watchpoint(
        &mut self,
        addr: u32,
        len: u32,
        kind: WatchKind,
    ) -> TargetResult<bool, Self> {
        let code = match kind {
            WatchKind::Write => 2u8,
            WatchKind::Read => 3,
            WatchKind::ReadWrite => 4,
        };
        match self.watches.remove(&(code, addr, len)) {
            Some(id) => {
                self.ask(&format!(r#"{{"cmd":"unwatch","id":{id}}}"#));
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

/// L'attente d'un arrêt : l'appareil avance par tranches, et la connexion est regardée entre deux tranches (Ctrl+C).
struct Boucle<'a>(std::marker::PhantomData<&'a ()>);

impl<'a> BlockingEventLoop for Boucle<'a> {
    type Target = Cible<'a>;
    type Connection = TcpStream;
    type StopReason = SingleThreadStopReason<u32>;

    fn wait_for_stop_reason(
        target: &mut Cible<'a>,
        conn: &mut TcpStream,
    ) -> Result<Event<Self::StopReason>, WaitForStopReasonError<String, std::io::Error>> {
        match target.action.take() {
            Some(Action::Step) => Ok(Event::TargetStopped(target.step_once())),
            Some(Action::ReverseStep) => Ok(Event::TargetStopped(
                target.reverse(r#"{"cmd":"step_back"}"#),
            )),
            Some(Action::ReverseContinue) => Ok(Event::TargetStopped(
                target.reverse(r#"{"cmd":"reverse_continue"}"#),
            )),
            Some(Action::Continue) | None => loop {
                if let Some(_byte) = conn.peek().map_err(WaitForStopReasonError::Connection)? {
                    let byte = conn.read().map_err(WaitForStopReasonError::Connection)?;
                    return Ok(Event::IncomingData(byte));
                }
                let reply = target.ask(&format!(r#"{{"cmd":"run","instructions":{SLICE}}}"#));
                if !Cible::ok(&reply) {
                    return Ok(Event::TargetStopped(SingleThreadStopReason::Signal(
                        Signal::SIGABRT,
                    )));
                }
                let stop = reply.get("stop").cloned().unwrap_or_else(Value::object);
                match stop.get("kind").and_then(Value::as_str).unwrap_or("") {
                    "budget" | "frame" | "sleep" => {}
                    // Une application qui attend une touche : le temps passe, sans occuper tout le processeur.
                    "wait_event" => {
                        target.ask(r#"{"cmd":"advance","ms":16}"#);
                        std::thread::sleep(Duration::from_millis(4));
                    }
                    _ => return Ok(Event::TargetStopped(Cible::stop_reason(&stop))),
                }
            },
        }
    }

    fn on_interrupt(_target: &mut Cible<'a>) -> Result<Option<Self::StopReason>, String> {
        Ok(Some(SingleThreadStopReason::Signal(Signal::SIGINT)))
    }
}

/// Sert un débogueur sur `stream` jusqu'à sa déconnexion. La session garde son état (points d'arrêt compris) entre deux connexions.
pub fn serve_connection(session: &mut dyn Debuggee, stream: TcpStream) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    // Le débogueur peut remonter le temps (reverse-step, reverse-continue) : l'historique tourne pendant la connexion.
    let recording = session
        .exchange(r#"{"cmd":"history"}"#)
        .contains(r#""recording":true"#);
    if !recording {
        session.exchange(r#"{"cmd":"history","action":"start"}"#);
    }
    let mut target = Cible::new(session);
    let result = GdbStub::new(stream).run_blocking::<Boucle>(&mut target);
    if !recording {
        target.ask_raw(r#"{"cmd":"history","action":"stop"}"#);
    }
    target.forget_watches();
    match result {
        Ok(_) => Ok(()),
        // Le débogueur qui ferme la connexion sans dire au revoir n'est pas une erreur.
        Err(e) if e.is_connection_error() => match e.into_connection_error() {
            Some((io, _)) if io.kind() != std::io::ErrorKind::UnexpectedEof => Err(io),
            _ => Ok(()),
        },
        Err(e) => Err(std::io::Error::other(e.to_string())),
    }
}
