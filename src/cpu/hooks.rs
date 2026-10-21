use super::{Bus, Cpu};

/// Ce que fait le processeur après une instruction `svc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvcAction {
    /// Le hook a traité l'appel (les résultats sont déjà dans les registres) : on continue.
    Handled,
    /// Le hook a traité l'appel et demande au processeur de rendre la main à l'hôte.
    HandledAndStop,
    /// Le hook n'a pas traité l'appel : le processeur lève lui-même l'exception SVCall.
    Raise,
}

/// Points d'extension qui permettent à l'hôte de réagir à ce qui se passe dans le processeur.
/// Chaque méthode a une valeur par défaut : l'hôte n'implémente que ce dont il a besoin.
pub trait Hooks {
    /// Appelé quand une instruction `svc #imm8` s'exécute. Le bus permet de lire et d'écrire la
    /// mémoire de l'application (arguments passés par pointeur).
    fn on_svc<B: Bus>(&mut self, _imm8: u8, _cpu: &mut Cpu, _bus: &mut B) -> SvcAction {
        SvcAction::Raise
    }
}
