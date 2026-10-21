//! Table des symboles.

/// Attachement du symbole (`st_info >> 4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolBind {
    Local,
    Global,
    Weak,
    Other(u8),
}

impl SymbolBind {
    pub(crate) fn from_info(info: u8) -> Self {
        match info >> 4 {
            0 => Self::Local,
            1 => Self::Global,
            2 => Self::Weak,
            n => Self::Other(n),
        }
    }
}

/// Nature du symbole (`st_info & 0xf`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    NoType,
    Object,
    Func,
    Section,
    File,
    Common,
    Tls,
    Other(u8),
}

impl SymbolKind {
    pub(crate) fn from_info(info: u8) -> Self {
        match info & 0xf {
            0 => Self::NoType,
            1 => Self::Object,
            2 => Self::Func,
            3 => Self::Section,
            4 => Self::File,
            5 => Self::Common,
            6 => Self::Tls,
            n => Self::Other(n),
        }
    }
}

/// Où est défini un symbole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolSection {
    /// Non défini ici (à résoudre ailleurs).
    Undefined,
    /// Valeur absolue (`SHN_ABS`) : `value` est l'adresse elle-même.
    Absolute,
    /// Symbole commun (`SHN_COMMON`) : `value` est l'alignement, `size` la taille.
    Common,
    /// Défini dans la section d'indice donné.
    Section(usize),
}

/// Un symbole ELF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    /// Valeur. Pour une fonction Thumb, le bit 0 est à 1 (convention ARM).
    pub value: u32,
    pub size: u32,
    pub bind: SymbolBind,
    pub kind: SymbolKind,
    pub section: SymbolSection,
}

impl Symbol {
    pub fn is_defined(&self) -> bool {
        self.section != SymbolSection::Undefined
    }

    pub fn is_weak(&self) -> bool {
        self.bind == SymbolBind::Weak
    }

    /// Fonction Thumb : symbole de fonction dont l'adresse a le bit 0 à 1.
    pub fn is_thumb_function(&self) -> bool {
        self.kind == SymbolKind::Func && self.value & 1 == 1
    }
}
