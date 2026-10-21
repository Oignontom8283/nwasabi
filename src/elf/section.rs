//! En-têtes de sections.

/// `sh_type` : sections de code, de données, de symboles…
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_REL: u32 = 9;
pub const SHT_INIT_ARRAY: u32 = 14;
pub const SHT_FINI_ARRAY: u32 = 15;
pub const SHT_PREINIT_ARRAY: u32 = 16;
pub const SHT_GROUP: u32 = 17;
/// Table d'exceptions ARM (`.ARM.exidx`).
pub const SHT_ARM_EXIDX: u32 = 0x7000_0001;

/// `sh_flags`.
pub const SHF_WRITE: u32 = 0x1;
pub const SHF_ALLOC: u32 = 0x2;
pub const SHF_EXECINSTR: u32 = 0x4;
pub const SHF_TLS: u32 = 0x400;

/// Un en-tête de section ELF, avec son nom déjà résolu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub kind: u32,
    pub flags: u32,
    pub addr: u32,
    pub(crate) offset: usize,
    pub size: u32,
    pub link: u32,
    pub info: u32,
    /// Alignement demandé (`sh_addralign`) ; 0 et 1 signifient « aucun ».
    pub align: u32,
    pub entsize: u32,
}

impl Section {
    /// La section occupe-t-elle de la mémoire à l'exécution ?
    pub fn is_alloc(&self) -> bool {
        self.flags & SHF_ALLOC != 0
    }

    pub fn is_writable(&self) -> bool {
        self.flags & SHF_WRITE != 0
    }

    pub fn is_executable(&self) -> bool {
        self.flags & SHF_EXECINSTR != 0
    }

    /// Section sans contenu dans le fichier (ex. `.bss`).
    pub fn is_nobits(&self) -> bool {
        self.kind == SHT_NOBITS
    }

    /// Alignement effectif en octets (au moins 1).
    pub fn alignment(&self) -> u32 {
        self.align.max(1)
    }
}
