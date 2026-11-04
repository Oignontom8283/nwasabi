//! Le désassembleur sur tout l'espace des encodages Thumb : les 65 536 demi-mots (instructions de 16 bits) et, pour les
//! 6 144 premiers demi-mots d'instructions de 32 bits, 64 seconds demi-mots variés. Le texte de chaque instruction est
//! résumé par un CRC-32 par tranche de 256 premiers demi-mots, pour situer une différence ; quelques lignes lisibles et des
//! suites avec bloc `it` complètent le fichier. Garde-fou de tout remaniement ou remplacement du désassembleur : tant que le
//! fichier de référence ne change pas, le texte affiché dans le débogueur est le même.
//!
//! Pour régénérer le fichier après un changement voulu : `NWASABI_UPDATE_GOLDEN=1 cargo test --test disasm_golden`.

mod support;

use nwasabi::cpu::{disassemble, is_32bit};

const GOLDEN: &str = "tests/golden/disasm.txt";
const START: u32 = 0x0800_0000;

/// Les seconds demi-mots essayés avec chaque premier demi-mot de 32 bits : des motifs de bits et des valeurs pseudo-aléatoires
/// (suite fixe, sans dépendance).
fn second_halfwords() -> Vec<u16> {
    let mut list: Vec<u16> = vec![
        0x0000, 0xffff, 0x8000, 0x0001, 0x1234, 0xa5a5, 0x5a5a, 0xf0f0, 0x0f0f, 0x00ff, 0xff00,
        0x7fff, 0xc000, 0x0100, 0x0200, 0x0400, 0x0800, 0x1000, 0x2000, 0x4000,
    ];
    let mut state = 0x2545_f491_4f6c_dd1du64;
    while list.len() < 64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        list.push((state >> 24) as u16);
    }
    list
}

fn text_of(hw1: u16, hw2: u16) -> String {
    let mut read = |address: u32| match address.wrapping_sub(START) {
        0 => Some(hw1),
        2 => Some(hw2),
        _ => None,
    };
    let lines = disassemble(&mut read, START, 1);
    match lines.first() {
        Some(l) => format!("{} {} {} {:?}", l.size, l.mnemonic, l.operands, l.target),
        None => "(rien)".to_owned(),
    }
}

fn sequence(words: &[u16]) -> String {
    let mut read = |address: u32| {
        let i = address.wrapping_sub(START) as usize / 2;
        words.get(i).copied()
    };
    disassemble(&mut read, START, words.len())
        .iter()
        .map(|l| format!("  {:08x}  {} {}\n", l.address, l.mnemonic, l.operands))
        .collect()
}

fn transcript() -> String {
    let seconds = second_halfwords();
    let mut out = String::new();
    out.push_str("# crc32 par tranche de 256 premiers demi-mots : 16 bits seuls | 32 bits avec 64 seconds demi-mots\n");
    for high in 0..=255u16 {
        let mut short = String::new();
        let mut long = String::new();
        for low in 0..=255u16 {
            let hw1 = high << 8 | low;
            if is_32bit(hw1) {
                for &hw2 in &seconds {
                    long.push_str(&text_of(hw1, hw2));
                    long.push('\n');
                }
            } else {
                short.push_str(&text_of(hw1, 0));
                short.push('\n');
            }
        }
        out.push_str(&format!(
            "{high:02x}xx  {:08x}  {:08x}\n",
            support::crc32(short.as_bytes()),
            support::crc32(long.as_bytes())
        ));
    }

    out.push_str("\n# des instructions lisibles\n");
    for (hw1, hw2) in [
        (0x2001, 0),
        (0x1840, 0),
        (0x4770, 0),
        (0xb510, 0),
        (0xe7fe, 0),
        (0xd0fe, 0),
        (0x4668, 0),
        (0xf04f, 0x0001),
        (0xf8d0, 0x1004),
        (0xe92d, 0x4ff0),
        (0xfb00, 0xf101),
        (0xeeb0, 0x0a40),
        (0xed9f, 0x0a01),
        (0xf3bf, 0x8f4f),
    ] {
        out.push_str(&format!("{hw1:04x} {hw2:04x}  {}\n", text_of(hw1, hw2)));
    }

    out.push_str("\n# des suites avec bloc it\n");
    out.push_str(&sequence(&[0xbf08, 0x1840, 0xbf1c, 0x1840, 0x2001, 0x4770]));
    out.push_str(&sequence(&[0xbfb8, 0x4608, 0x4608, 0x4608, 0x4608, 0x4770]));
    out
}

#[test]
fn the_disassembler_prints_exactly_as_recorded() {
    let actual = transcript();
    let path = format!("{}/{GOLDEN}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("NWASABI_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path} : {e} (NWASABI_UPDATE_GOLDEN=1 pour le créer)"));
    if actual != expected {
        let first = actual
            .lines()
            .zip(expected.lines())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
        panic!(
            "le désassemblage a changé à la ligne {} :\n  attendu : {}\n  obtenu  : {}",
            first + 1,
            expected.lines().nth(first).unwrap_or("(fin)"),
            actual.lines().nth(first).unwrap_or("(fin)"),
        );
    }
}
