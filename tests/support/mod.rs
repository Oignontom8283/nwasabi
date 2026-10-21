//! Outils partagés par les tests d'intégration.
//!
//! Chaque fichier de test n'en utilise qu'une partie.

#![allow(dead_code)]

use std::path::PathBuf;

/// Fichiers `.nwa` du corpus de test, s'il est fourni.
///
/// Le corpus (applications de tiers) n'est pas distribué avec le dépôt : on l'indique avec la
/// variable d'environnement `NWASABI_CORPUS` (dossier contenant des `.nwa`). Sans elle, les
/// tests qui en dépendent sont ignorés avec un message, et ne font pas échouer la suite.
pub fn corpus() -> Option<Vec<(String, Vec<u8>)>> {
    let dir = PathBuf::from(std::env::var_os("NWASABI_CORPUS")?);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("NWASABI_CORPUS={}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "nwa"))
        .collect();
    files.sort();
    Some(
        files
            .into_iter()
            .map(|p| {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                (name, std::fs::read(&p).unwrap())
            })
            .collect(),
    )
}

/// Message affiché quand un test est ignoré faute de corpus.
pub fn skip_without_corpus() {
    eprintln!("corpus absent (variable NWASABI_CORPUS non définie) : test ignoré");
}

use nwasabi::elf::{
    ObjectWriter, R_ARM_ABS32, R_ARM_THM_CALL, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SectionId,
    SymbolBind, SymbolId, SymbolKind, SymbolSection,
};
use nwasabi::link::LayoutParams;

/// Zones réservées aux applications sur un N0110 sous Epsilon 26.4.0 (valeurs lues dans le
/// `UserlandHeader` de ce firmware).
pub fn n0110_layout() -> LayoutParams {
    LayoutParams {
        flash_start: 0x9020_0000,
        flash_length: 0x1f_0000,
        ram_start: 0x2001_18a4,
        ram_length: 0x2003_7000 - 0x2001_18a4,
        trampoline_address: 0x9002_0038,
    }
}

/// CRC-32 (polynôme IEEE 802.3, comme zlib) : sert à résumer une image dans les fichiers de référence.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Nombre de références de chaque sorte que `main` peut contenir.
const SLOTS: usize = 8;
/// Décalage, dans `main`, du premier emplacement de mot (après `bx lr` et un remplissage).
pub const WORD_SLOTS_AT: u32 = 4;
/// Décalage, dans `main`, du premier emplacement d'appel (après les emplacements de mot).
pub const CALL_SLOTS_AT: u32 = WORD_SLOTS_AT + 4 * SLOTS as u32;

/// Une application minimale fabriquée à la main : `main`, le niveau d'API, un nom et une icône.
///
/// `main` est une fonction de `bx lr` suivie d'emplacements que les tests remplissent avec
/// [`SyntheticApp::word_ref`] (un mot qui contiendra une adresse) et [`SyntheticApp::call`] (un
/// `bl`). Tout ce que `main` référence reste dans l'image, le reste est supprimé.
pub struct SyntheticApp {
    pub writer: ObjectWriter,
    pub main_section: SectionId,
    pub main_symbol: SymbolId,
    words: usize,
    calls: usize,
}

/// Sections obligatoires que l'on peut omettre pour tester les erreurs.
pub const API_LEVEL: &str = ".rodata.eadk_api_level";
pub const NAME: &str = ".rodata.eadk_app_name";
pub const ICON: &str = ".rodata.eadk_app_icon";

pub fn synthetic_app() -> SyntheticApp {
    synthetic_app_with(0u32.to_le_bytes().to_vec(), &[])
}

/// Comme [`synthetic_app`], avec un contenu de niveau d'API choisi et des sections omises.
pub fn synthetic_app_with(api_level: Vec<u8>, skip: &[&str]) -> SyntheticApp {
    let mut w = ObjectWriter::new();
    let mut code = vec![0x70, 0x47, 0x00, 0xbf]; // bx lr ; nop
    code.extend(std::iter::repeat_n(0, 4 * SLOTS)); // emplacements de mots
    for _ in 0..SLOTS {
        code.extend([0xff, 0xf7, 0xfe, 0xff]); // bl -4 : valeur laissée par l'assembleur
    }
    let main_section = w.add_section(".text.main", SHF_ALLOC | SHF_EXECINSTR, 4, &code);
    let main_symbol = w.add_symbol(
        "main",
        1,
        code.len() as u32,
        SymbolBind::Global,
        SymbolKind::Func,
        SymbolSection::Section(main_section.0),
    );
    let header: [(&str, u32, u32, &[u8]); 3] = [
        (API_LEVEL, SHF_ALLOC, 4, &api_level),
        (NAME, SHF_ALLOC, 1, b"Test\0"),
        (ICON, SHF_ALLOC | SHF_WRITE, 1, &[1, 2, 3, 4, 5, 6, 7, 8]),
    ];
    for (name, flags, align, data) in header {
        if !skip.contains(&name) {
            w.add_section(name, flags, align, data);
        }
    }
    SyntheticApp {
        writer: w,
        main_section,
        main_symbol,
        words: 0,
        calls: 0,
    }
}

impl SyntheticApp {
    /// `main` contient désormais un mot égal à l'adresse de `target` ; renvoie son décalage dans `main`.
    pub fn word_ref(&mut self, target: SymbolId) -> u32 {
        assert!(self.words < SLOTS);
        let offset = WORD_SLOTS_AT + 4 * self.words as u32;
        self.words += 1;
        self.writer
            .add_relocation(self.main_section, offset, target, R_ARM_ABS32);
        offset
    }

    /// `main` contient désormais un `bl target` ; renvoie son décalage dans `main`.
    pub fn call(&mut self, target: SymbolId) -> u32 {
        assert!(self.calls < SLOTS);
        let offset = CALL_SLOTS_AT + 4 * self.calls as u32;
        self.calls += 1;
        self.writer
            .add_relocation(self.main_section, offset, target, R_ARM_THM_CALL);
        offset
    }

    /// Ajoute une fonction `bx lr` dans sa propre section et renvoie son symbole.
    pub fn function(&mut self, name: &str) -> SymbolId {
        let section = self.writer.add_section(
            &format!(".text.{name}"),
            SHF_ALLOC | SHF_EXECINSTR,
            4,
            &[0x70, 0x47, 0, 0],
        );
        self.writer.add_symbol(
            name,
            1,
            4,
            SymbolBind::Global,
            SymbolKind::Func,
            SymbolSection::Section(section.0),
        )
    }

    pub fn finish(self) -> Vec<u8> {
        self.writer.finish()
    }
}

/// Symbole non défini (à résoudre par l'éditeur de liens).
pub fn undefined(w: &mut ObjectWriter, name: &str, bind: SymbolBind) -> SymbolId {
    w.add_symbol(
        name,
        0,
        0,
        bind,
        SymbolKind::NoType,
        SymbolSection::Undefined,
    )
}

/// Encode une image RGBA en PNG (sans compression : sert à regarder une capture d'écran).
pub fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for row in rgba.chunks((width * 4) as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    while let Some(block) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&(b << 16 | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}
