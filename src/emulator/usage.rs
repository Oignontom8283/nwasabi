//! Ce que l'application utilise de la mémoire : tas et pile.

/// Une plage d'adresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

/// Utilisation de la mémoire par l'application depuis son lancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Plage réservée au tas, limite actuelle du programme (`brk`) et plus haute adresse écrite.
    pub heap: Span,
    pub heap_break: u32,
    pub heap_high_water: Option<u32>,
    /// Plage réservée à la pile (la pile descend depuis `end`) et adresse la plus basse écrite.
    pub stack: Span,
    pub stack_low_water: Option<u32>,
    /// Pile actuelle.
    pub sp: u32,
}
