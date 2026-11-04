//! La vitesse réelle du processeur émulé, en millions d'instructions par seconde (MIPS) : `cargo bench --bench cpu`.
//!
//! Ce n'est pas un test : `cargo test` ne le lance jamais. Il sert à comparer avant et après un changement du code du
//! processeur, du bus ou du noyau simulé. Chaque mesure exécute une boucle d'une application installée dans un vrai
//! appareil ; le temps par instruction se lit en divisant la durée par `INSTRUCTIONS`.

use divan::counter::ItemsCount;
use divan::{Bencher, black_box};
use nwasabi::elf::{
    ObjectWriter, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SectionId, SymbolBind, SymbolKind,
    SymbolSection,
};
use nwasabi::emulator::{Emulator, RunStop};
use nwasabi::profile::Model;

fn main() {
    divan::main();
}

/// Instructions exécutées par mesure.
const INSTRUCTIONS: u64 = 2_000_000;

/// Une application dont `main` est le code Thumb donné (comme les tests, sans appel système).
fn app(code: &[u16]) -> Vec<u8> {
    let bytes: Vec<u8> = code.iter().flat_map(|hw| hw.to_le_bytes()).collect();
    let mut w = ObjectWriter::new();
    let main_section: SectionId = w.add_section(".text.main", SHF_ALLOC | SHF_EXECINSTR, 4, &bytes);
    w.add_symbol(
        "main",
        1,
        bytes.len() as u32,
        SymbolBind::Global,
        SymbolKind::Func,
        SymbolSection::Section(main_section.0),
    );
    w.add_section(".rodata.eadk_api_level", SHF_ALLOC, 4, &0u32.to_le_bytes());
    w.add_section(".rodata.eadk_app_name", SHF_ALLOC, 1, b"Bench\0");
    w.add_section(
        ".rodata.eadk_app_icon",
        SHF_ALLOC | SHF_WRITE,
        1,
        &[1, 2, 3, 4, 5, 6, 7, 8],
    );
    w.finish()
}

fn measure(bencher: Bencher, code: &[u16]) {
    let nwa = app(code);
    // La boucle doit tourner jusqu'au bout du budget : sinon on mesurerait une application qui s'arrête ou plante.
    let mut check = Emulator::new(Model::N0110).unwrap();
    check.install_nwa(&nwa, None).unwrap();
    assert!(
        matches!(check.run(INSTRUCTIONS), RunStop::Budget),
        "la boucle s'arrête avant la fin du budget"
    );
    bencher
        .counter(ItemsCount::new(INSTRUCTIONS as usize))
        .with_inputs(|| {
            let mut emu = Emulator::new(Model::N0110).unwrap();
            emu.install_nwa(&nwa, None).unwrap();
            emu
        })
        .bench_local_values(|mut emu| black_box(emu.run(INSTRUCTIONS)));
}

/// Arithmétique et branchement : `adds`, `eors`, `lsls`, `subs`, `bne` (instructions de 16 bits).
#[divan::bench]
fn integer_loop(bencher: Bencher) {
    // movs r1, #0 ; loop: adds r0, #1 ; eors r1, r0 ; lsls r3, r1, #3 ; subs r2, r0, r1 ; b loop
    measure(bencher, &[0x2100, 0x3001, 0x4041, 0x00cb, 0x1a42, 0xe7fa]);
}

/// Lectures et écritures de la pile : `ldr`, `str`, `ldrb`, `strb`.
#[divan::bench]
fn memory_loop(bencher: Bencher) {
    // mov r4, sp ; loop: ldr r2, [sp, #0] ; adds r2, #1 ; str r2, [sp, #0] ; ldrb r3, [r4, #4] ; strb r2, [r4, #5] ; b loop
    measure(
        bencher,
        &[0x466c, 0x9a00, 0x3201, 0x9200, 0x7923, 0x7162, 0xe7f9],
    );
}

/// Instructions de 32 bits : `mul`, `add.w`, `ldr.w`.
#[divan::bench]
fn thumb2_loop(bencher: Bencher) {
    // loop: mul r1, r0, r0 ; add.w r0, r1, r0 ; ldr.w r2, [sp, #8] ; b loop
    measure(
        bencher,
        &[0xfb00, 0xf100, 0xeb01, 0x0000, 0xf8dd, 0x2008, 0xe7f8],
    );
}

/// Virgule flottante simple précision : `vadd.f32`, `vmul.f32`.
#[divan::bench]
fn float_loop(bencher: Bencher) {
    // loop: vadd.f32 s0, s0, s1 ; vmul.f32 s1, s1, s0 ; b loop
    measure(bencher, &[0xee30, 0x0a20, 0xee60, 0x0a80, 0xe7fa]);
}
