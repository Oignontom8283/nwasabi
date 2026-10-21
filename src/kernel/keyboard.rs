//! Le clavier : état des touches (`keyboard_scan`) et événements (`event_get`).
//!
//! Les numéros de touches sont ceux de l'API EADK publique (`eadk_key_*`). Un événement est un
//! numéro : celui de la touche, augmenté de 56 quand `shift` est actif, de 112 avec `alpha`, de
//! 168 avec les deux. Ces règles sont une approximation du comportement d'Epsilon : pas de
//! répétition sur appui long, pas d'événements « texte ».

use std::collections::VecDeque;

/// Les touches de la calculatrice (valeur = numéro de bit dans l'état du clavier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Key {
    Left = 0,
    Up = 1,
    Down = 2,
    Right = 3,
    Ok = 4,
    Back = 5,
    Home = 6,
    OnOff = 8,
    Shift = 12,
    Alpha = 13,
    Xnt = 14,
    Var = 15,
    Toolbox = 16,
    Backspace = 17,
    Exp = 18,
    Ln = 19,
    Log = 20,
    Imaginary = 21,
    Comma = 22,
    Power = 23,
    Sine = 24,
    Cosine = 25,
    Tangent = 26,
    Pi = 27,
    Sqrt = 28,
    Square = 29,
    Seven = 30,
    Eight = 31,
    Nine = 32,
    LeftParenthesis = 33,
    RightParenthesis = 34,
    Four = 36,
    Five = 37,
    Six = 38,
    Multiplication = 39,
    Division = 40,
    One = 42,
    Two = 43,
    Three = 44,
    Plus = 45,
    Minus = 46,
    Zero = 48,
    Dot = 49,
    Ee = 50,
    Ans = 51,
    Exe = 52,
}

impl Key {
    pub const ALL: [Key; 46] = [
        Key::Left,
        Key::Up,
        Key::Down,
        Key::Right,
        Key::Ok,
        Key::Back,
        Key::Home,
        Key::OnOff,
        Key::Shift,
        Key::Alpha,
        Key::Xnt,
        Key::Var,
        Key::Toolbox,
        Key::Backspace,
        Key::Exp,
        Key::Ln,
        Key::Log,
        Key::Imaginary,
        Key::Comma,
        Key::Power,
        Key::Sine,
        Key::Cosine,
        Key::Tangent,
        Key::Pi,
        Key::Sqrt,
        Key::Square,
        Key::Seven,
        Key::Eight,
        Key::Nine,
        Key::LeftParenthesis,
        Key::RightParenthesis,
        Key::Four,
        Key::Five,
        Key::Six,
        Key::Multiplication,
        Key::Division,
        Key::One,
        Key::Two,
        Key::Three,
        Key::Plus,
        Key::Minus,
        Key::Zero,
        Key::Dot,
        Key::Ee,
        Key::Ans,
        Key::Exe,
    ];

    /// Numéro de bit de la touche.
    pub fn bit(self) -> u32 {
        u32::from(self as u8)
    }

    /// La touche portant ce numéro, s'il y en a une.
    pub fn from_bit(bit: u32) -> Option<Key> {
        Self::ALL.into_iter().find(|k| k.bit() == bit)
    }
}

/// Nombre d'événements par « plan » (normal, shift, alpha, shift+alpha).
pub const EVENTS_PER_PLANE: u16 = 56;
/// Numéro de l'événement « rien » (délai écoulé) : le premier événement spécial.
pub const EVENT_NONE: u16 = 4 * EVENTS_PER_PLANE;

/// Ce que fait la touche `Home`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeBehavior {
    /// Comme sur l'appareil : le noyau reprend la main et quitte l'application.
    ExitApp,
    /// L'application reçoit la touche comme les autres (comme avec le contournement DFU).
    Deliver,
}

/// État du clavier.
#[derive(Debug, Clone)]
pub struct Keyboard {
    state: u64,
    events: VecDeque<u16>,
    shift: bool,
    alpha: bool,
    alpha_lock: bool,
    pub home: HomeBehavior,
    exit_requested: bool,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            state: 0,
            events: VecDeque::new(),
            shift: false,
            alpha: false,
            alpha_lock: false,
            home: HomeBehavior::ExitApp,
            exit_requested: false,
        }
    }
}

/// Au plus autant d'événements en attente (le clavier réel n'en garde qu'un nombre limité).
const MAX_PENDING_EVENTS: usize = 64;

impl Keyboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Touches actuellement enfoncées (bit `Key::bit`), valeur de `keyboard_scan`.
    pub fn state(&self) -> u64 {
        self.state
    }

    /// Remplace l'état des touches sans produire d'événements.
    pub fn set_state(&mut self, state: u64) {
        self.state = state;
    }

    /// Vrai si l'utilisateur a demandé de quitter l'application (touche `Home`).
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    pub fn clear_exit_request(&mut self) {
        self.exit_requested = false;
    }

    /// Indique si `shift`, `alpha`, `alpha` verrouillé sont actifs.
    pub fn modifiers(&self) -> (bool, bool, bool) {
        (self.shift, self.alpha, self.alpha_lock)
    }

    /// Enfonce une touche : met à jour l'état et produit son événement.
    pub fn press(&mut self, key: Key) {
        let already = self.state >> key.bit() & 1 != 0;
        self.state |= 1 << key.bit();
        if already {
            return;
        }
        match key {
            Key::Home if self.home == HomeBehavior::ExitApp => {
                self.exit_requested = true;
                return;
            }
            Key::Shift => self.shift = !self.shift,
            Key::Alpha => {
                if self.shift {
                    self.alpha_lock = true;
                    self.alpha = true;
                    self.shift = false;
                } else {
                    self.alpha = !self.alpha;
                    self.alpha_lock = false;
                }
            }
            _ => {}
        }
        let modifier = u16::from(self.shift) + 2 * u16::from(self.alpha);
        let event = match key {
            Key::Shift | Key::Alpha => key.bit() as u16,
            _ => key.bit() as u16 + modifier * EVENTS_PER_PLANE,
        };
        if !matches!(key, Key::Shift | Key::Alpha) {
            self.shift = false;
            if !self.alpha_lock {
                self.alpha = false;
            }
        }
        self.push_event(event);
    }

    /// Relâche une touche.
    pub fn release(&mut self, key: Key) {
        self.state &= !(1 << key.bit());
    }

    /// Ajoute un événement à la file (pour l'hôte qui veut en fabriquer directement).
    pub fn push_event(&mut self, event: u16) {
        if self.events.len() < MAX_PENDING_EVENTS {
            self.events.push_back(event);
        }
    }

    /// Prend le plus ancien événement en attente.
    pub fn pop_event(&mut self) -> Option<u16> {
        self.events.pop_front()
    }

    pub fn pending_events(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_numbers_match_the_public_api() {
        assert_eq!(Key::Left.bit(), 0);
        assert_eq!(Key::Ok.bit(), 4);
        assert_eq!(Key::Shift.bit(), 12);
        assert_eq!(Key::Exe.bit(), 52);
        assert_eq!(Key::ALL.len(), 46);
        for key in Key::ALL {
            assert_eq!(Key::from_bit(key.bit()), Some(key));
        }
        assert_eq!(Key::from_bit(7), None);
        assert_eq!(Key::from_bit(35), None);
    }

    #[test]
    fn pressing_sets_the_state_and_queues_an_event() {
        let mut k = Keyboard::new();
        k.press(Key::Ok);
        assert_eq!(k.state(), 1 << 4);
        assert_eq!(k.pop_event(), Some(4));
        assert_eq!(k.pop_event(), None);
        k.release(Key::Ok);
        assert_eq!(k.state(), 0);
    }

    #[test]
    fn holding_a_key_does_not_repeat_its_event() {
        let mut k = Keyboard::new();
        k.press(Key::Left);
        k.press(Key::Left);
        assert_eq!(k.pending_events(), 1);
    }

    #[test]
    fn shift_applies_to_the_next_key_only() {
        let mut k = Keyboard::new();
        k.press(Key::Shift);
        k.press(Key::One);
        k.press(Key::Two);
        assert_eq!(k.pop_event(), Some(12));
        assert_eq!(k.pop_event(), Some(42 + 56));
        assert_eq!(k.pop_event(), Some(43));
    }

    #[test]
    fn shift_then_alpha_locks_alpha() {
        let mut k = Keyboard::new();
        k.press(Key::Shift);
        k.press(Key::Alpha);
        assert_eq!(k.modifiers(), (false, true, true));
        k.press(Key::Sine);
        k.press(Key::Cosine);
        let events: Vec<_> = std::iter::from_fn(|| k.pop_event()).collect();
        assert_eq!(events, [12, 13, 24 + 112, 25 + 112]);
        k.release(Key::Alpha);
        k.press(Key::Alpha);
        assert!(!k.modifiers().1);
    }

    #[test]
    fn home_requests_exit_or_is_delivered() {
        let mut k = Keyboard::new();
        k.press(Key::Home);
        assert!(k.exit_requested());
        assert_eq!(k.pending_events(), 0);

        let mut k = Keyboard::new();
        k.home = HomeBehavior::Deliver;
        k.press(Key::Home);
        assert!(!k.exit_requested());
        assert_eq!(k.pop_event(), Some(6));
    }

    #[test]
    fn the_event_queue_is_bounded() {
        let mut k = Keyboard::new();
        for _ in 0..1000 {
            k.push_event(1);
        }
        assert_eq!(k.pending_events(), MAX_PENDING_EVENTS);
    }
}
