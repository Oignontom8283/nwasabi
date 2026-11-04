//! L'appareil de la page web : un débogueur branché sur le serveur pilote l'appareil que la page fait tourner, par un pont.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::Debuggee;
use crate::json::{Value, ValueExt};

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
