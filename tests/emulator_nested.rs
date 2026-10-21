//! Des émulateurs dans l'émulateur : nofrendo (NES) et Peanut-GB (Game Boy) sont des applications
//! du corpus qui exécutent une ROM donnée comme « données externes ». On leur fabrique ici deux
//! ROM minuscules, écrites à la main, qui peignent tout l'écran d'une couleur : si l'écran de la
//! calculatrice change de la bonne façon, c'est que des centaines de milliers d'instructions
//! (processeur 6502 ou Sharp LR35902 émulé, PPU, affichage) se sont exécutées correctement.

mod support;

use nwasabi::emulator::{Emulator, RunStop};
use nwasabi::profile::Model;

/// ROM NES (mapper 0, 16 Kio de programme) qui règle la couleur de fond du PPU puis boucle.
fn nes_rom() -> Vec<u8> {
    #[rustfmt::skip]
    let code = [
        0x78, 0xd8, 0xa2, 0xff, 0x9a,             // sei ; cld ; ldx #$ff ; txs
        0xa9, 0x3f, 0x8d, 0x06, 0x20,             // lda #$3f ; sta $2006
        0xa9, 0x00, 0x8d, 0x06, 0x20,             // lda #$00 ; sta $2006   (adresse $3f00 : palette)
        0xa9, 0x21, 0x8d, 0x07, 0x20,             // lda #$21 ; sta $2007   (couleur de fond : bleu clair)
        0xa9, 0x00, 0x8d, 0x05, 0x20, 0x8d, 0x05, 0x20, // défilement à zéro
        0xa9, 0x0a, 0x8d, 0x01, 0x20,             // lda #$0a ; sta $2001   (affiche le fond)
        0x4c, 0x21, 0xc0,                         // boucle : jmp boucle
    ];
    let mut prg = code.to_vec();
    prg.resize(0x4000 - 6, 0);
    prg.extend_from_slice(&[0x00, 0xc0, 0x00, 0xc0, 0x00, 0xc0]); // vecteurs NMI, reset, IRQ
    let mut rom = b"NES\x1a".to_vec();
    rom.extend_from_slice(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // 1 × 16 Kio de PRG, 1 × 8 Kio de CHR
    rom.extend(prg);
    rom.extend(vec![0; 0x2000]);
    rom
}

/// ROM Game Boy de 32 Kio qui remplit la tuile 0 (couleur 1) puis boucle.
fn game_boy_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x100..0x104].copy_from_slice(&[0x00, 0xc3, 0x50, 0x01]); // nop ; jp $0150
    #[rustfmt::skip]
    let code = [
        0x21, 0x00, 0x80,                         // ld hl,$8000
        0x06, 0x08,                               // ld b,8
        0x3e, 0xff, 0x22, 0xaf, 0x22, 0x05, 0x20, 0xf8, // boucle : une ligne de la tuile, 8 fois
        0x18, 0xfe,                               // jr $
    ];
    rom[0x150..0x150 + code.len()].copy_from_slice(&code);
    rom[0x134..0x13c].copy_from_slice(b"NWASABI\0");
    let checksum = rom[0x134..0x14d]
        .iter()
        .fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1));
    rom[0x14d] = checksum;
    rom
}

/// Lance l'application avec la ROM et rend l'écran après `instructions` instructions.
fn run(app_prefix: &str, rom: &[u8], instructions: u64) -> Option<Vec<u16>> {
    let apps = support::corpus().or_else(|| {
        support::skip_without_corpus();
        None
    })?;
    let (_, nwa) = apps.iter().find(|(n, _)| n.starts_with(app_prefix))?;
    let mut emu = Emulator::new(Model::N0110).unwrap();
    emu.install_nwa(nwa, Some(rom)).unwrap();
    while emu.cpu.cycles < instructions {
        match emu.run(1_000_000) {
            RunStop::Fault(f) => panic!("{app_prefix} : {f:?}"),
            RunStop::Exited(code) => panic!("{app_prefix} s'est terminée (code {code})"),
            _ => {}
        }
    }
    Some(emu.kernel.display.pixels().to_vec())
}

fn pixel(screen: &[u16], x: usize, y: usize) -> u16 {
    screen[y * 320 + x]
}

#[test]
fn nofrendo_paints_the_background_color_of_a_nes_rom() {
    let Some(screen) = run("nofrendo-1.2.4", &nes_rom(), 60_000_000) else {
        return;
    };
    // Image de 256 × 240 centrée sur les 320 pixels de l'écran : bandes noires de 32 pixels.
    let color = pixel(&screen, 160, 120);
    assert_ne!(color, 0, "le fond est peint");
    assert_eq!(pixel(&screen, 32, 0), color);
    assert_eq!(pixel(&screen, 287, 239), color);
    assert_eq!(pixel(&screen, 5, 120), 0, "bande noire à gauche");
    assert_eq!(pixel(&screen, 310, 120), 0, "bande noire à droite");
    // Bleu clair de la palette NES ($21) : plus de bleu que de rouge.
    assert!(color & 0x1f > (color >> 11) & 0x1f);
}

#[test]
fn peanut_gb_draws_the_tile_of_a_game_boy_rom() {
    for app in ["peanutgb-1.2.3", "peanutgbc-1.1.2"] {
        let Some(screen) = run(app, &game_boy_rom(), 100_000_000) else {
            return;
        };
        let color = pixel(&screen, 160, 120);
        assert_ne!(color, 0, "{app} : la tuile est dessinée");
        assert_eq!(pixel(&screen, 40, 5), color, "{app}");
        assert_eq!(pixel(&screen, 2, 120), 0, "{app} : bande noire à gauche");
    }
}

#[test]
fn nofrendo_refuses_an_invalid_rom_and_exits_cleanly() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("nofrendo-1.2.4"))
        .unwrap();
    let mut emu = Emulator::new(Model::N0110).unwrap();
    emu.install_nwa(nwa, Some(&[1, 2, 3])).unwrap();
    let stop = loop {
        match emu.run(1_000_000) {
            RunStop::Frame | RunStop::Sleep { .. } | RunStop::Budget => {}
            other => break other,
        }
        assert!(emu.cpu.cycles < 50_000_000, "ne se termine pas");
    };
    assert!(matches!(stop, RunStop::Exited(_)), "{stop:?}");
}
