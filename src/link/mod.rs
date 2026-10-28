//! Éditeur de liens : transforme un `.nwa` (objet ELF relogeable) en image prête à être placée
//! en mémoire, comme le fait l'outil de flashage officiel avant d'écrire l'application dans la
//! flash de la calculatrice.
//!
//! Le travail se fait en cinq temps, un fichier chacun :
//! 1. résoudre les symboles entre le `.nwa`, le runtime EADK et les données externes ;
//! 2. écarter les sections que rien n'utilise (comme `--gc-sections`) ;
//! 3. placer les sections en flash et en RAM selon la disposition d'une application externe ;
//! 4. appliquer les relocations ;
//! 5. écrire l'en-tête d'application et assembler l'image.

mod error;
mod linker;
pub mod relocate;
pub mod runtime;
pub mod thumb;

pub use error::LinkError;

use crate::elf::{ElfObject, Limits, SymbolKind};

/// Taille d'un secteur de la flash externe : les applications y sont alignées.
pub const APP_SECTOR_SIZE: u32 = 0x1_0000;

/// Où placer l'application : zones de flash et de RAM réservées aux applications externes.
///
/// Ces valeurs viennent du `UserlandHeader` du firmware cible ; elles changent avec le modèle et
/// la version d'Epsilon (voir le profil du firmware).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutParams {
    pub flash_start: u32,
    pub flash_length: u32,
    pub ram_start: u32,
    pub ram_length: u32,
    /// Adresse de la table de fonctions du userland (`eadk_display_draw_string` y est lue).
    pub trampoline_address: u32,
}

impl LayoutParams {
    pub fn flash_end(&self) -> u64 {
        u64::from(self.flash_start) + u64::from(self.flash_length)
    }

    pub fn ram_end(&self) -> u64 {
        u64::from(self.ram_start) + u64::from(self.ram_length)
    }
}

/// Réglages de l'édition de liens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinkOptions {
    /// Bornes de lecture du fichier ELF.
    pub limits: Limits,
    /// Fournir aussi les fonctions que `eadk.h` déclare mais que le firmware officiel n'offre pas
    /// (batterie, USB). Désactivé par défaut : un vrai appareil refuserait ces applications.
    pub extended_runtime: bool,
}

/// D'où vient une section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Le fichier `.nwa` de l'application.
    App,
    /// Le runtime EADK fabriqué par nwasabi.
    Runtime,
    /// Les données externes fournies avec l'application.
    ExternalData,
}

/// Rôle d'une section dans l'image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionClass {
    /// En-tête d'application, nom, icône, niveau d'API.
    Header,
    Text,
    Rodata,
    Data,
    Bss,
    /// Variables communes (`COMMON`), placées avec `.bss`.
    Common,
}

/// Une section de l'application, telle qu'elle est placée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedSection {
    pub name: String,
    pub origin: Origin,
    pub class: SectionClass,
    /// Adresse d'exécution.
    pub address: u32,
    /// Adresse dans la flash de la copie initiale (absente pour `.bss`).
    pub flash_address: Option<u32>,
    pub size: u32,
    pub alignment: u32,
}

/// Un symbole de l'image finale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedSymbol {
    pub name: String,
    pub address: u32,
    pub size: u32,
    pub kind: SymbolKind,
    pub origin: Origin,
}

/// Une zone de mémoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub address: u32,
    pub size: u32,
}

/// La section `.data` : copiée de la flash vers la RAM par `_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataSegment {
    /// Adresse en RAM.
    pub address: u32,
    /// Adresse de la copie initiale en flash.
    pub flash_address: u32,
    pub size: u32,
}

/// Remarque qui n'empêche pas de lier mais mérite attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkWarning {
    /// La taille écrite dans l'en-tête d'application ne couvre pas l'image `.data` : si celle-ci
    /// franchit une limite de secteur, le système cherche l'application suivante au mauvais
    /// endroit (défaut du format officiel, reproduit tel quel).
    AppSizeMismatch {
        system_next_offset: u32,
        installed_next_offset: u32,
    },
}

/// Résultat de l'édition de liens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedApp {
    /// Nom de l'application (`.rodata.eadk_app_name`).
    pub name: String,
    pub api_level: u32,
    /// Icône brute, au format NWI.
    pub icon: Vec<u8>,
    /// Adresse du début de l'image en flash.
    pub flash_start: u32,
    /// Contenu de la flash à partir de `flash_start` (en-tête, code, constantes, copie de `.data`).
    pub flash: Vec<u8>,
    /// Adresse de `_start`, bit 0 à 1 (Thumb).
    pub entry: u32,
    /// Fin du code et des constantes (`_eadk_app_end`), valeur écrite dans l'en-tête d'application.
    pub app_end: u32,
    pub data: DataSegment,
    pub bss: Region,
    /// Tas : de la fin de `.bss` jusqu'à la fin de la RAM réservée.
    pub heap: Region,
    pub sections: Vec<PlacedSection>,
    pub symbols: Vec<LinkedSymbol>,
    /// Informations de débogage de l'application (sections `.debug_*`), relocations appliquées : les adresses
    /// qu'elles contiennent sont celles de l'image finale. Vide si l'application n'en a pas.
    pub debug: Vec<DebugSection>,
    pub warnings: Vec<LinkWarning>,
}

/// Une section `.debug_*` de l'application, prête à être lue (adresses définitives).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSection {
    pub name: String,
    pub data: Vec<u8>,
}

impl LinkedApp {
    /// Taille occupée en flash une fois l'application installée : l'outil de flashage complète
    /// jusqu'au secteur suivant, et ajoute un secteur entier quand la taille est déjà alignée.
    pub fn installed_size(&self) -> u32 {
        let len = self.flash.len() as u32;
        len + (APP_SECTOR_SIZE - len % APP_SECTOR_SIZE)
    }
}

/// Lie une application.
///
/// - `nwa` : le contenu du fichier `.nwa` ;
/// - `external_data` : les données externes, si l'application en réclame (`eadk_external_data`) ;
/// - `layout` : zones de flash et de RAM réservées aux applications.
pub fn link(
    nwa: &[u8],
    external_data: Option<&[u8]>,
    layout: &LayoutParams,
    options: &LinkOptions,
) -> Result<LinkedApp, LinkError> {
    let runtime_bytes = runtime::build_runtime(options.extended_runtime);
    let data_bytes = external_data.map(runtime::build_external_data);

    // L'ordre compte : en cas de définitions multiples, la première gagne (le runtime d'abord,
    // comme `eadk.o` avant l'application dans l'outil de flashage officiel).
    let mut objects = vec![
        linker::Obj {
            origin: Origin::Runtime,
            elf: ElfObject::parse(&runtime_bytes, &options.limits)?,
        },
        linker::Obj {
            origin: Origin::App,
            elf: ElfObject::parse(nwa, &options.limits)?,
        },
    ];
    if let Some(bytes) = &data_bytes {
        objects.push(linker::Obj {
            origin: Origin::ExternalData,
            elf: ElfObject::parse(bytes, &options.limits)?,
        });
    }
    linker::run(&objects, layout)
}
