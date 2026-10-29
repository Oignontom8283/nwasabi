//! Édition de liens de tous les `.nwa` du corpus (si le corpus est présent : `NWASABI_CORPUS` ou `local/nwa`).
//!
//! Les résultats sont comparés à `tests/golden/link_images.tsv` : pour chaque application, taille et
//! somme de contrôle de l'image et adresses principales. Ce fichier a été confronté à l'éditeur de
//! liens GNU (`arm-none-eabi-ld`) : à la fusion des chaînes de caractères près, les images sont
//! identiques octet pour octet. Pour le régénérer après un changement voulu :
//! `NWASABI_UPDATE_GOLDEN=1 NWASABI_CORPUS=… cargo test --test link_corpus`.

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use nwasabi::link::{LinkOptions, LinkedApp, link};
use support::{crc32, n0110_layout};

const GOLDEN: &str = "tests/golden/link_images.tsv";

/// Données externes fournies à toutes les applications (assez grandes pour les lecteurs de ROM).
fn external_data() -> Vec<u8> {
    (0..1000u32).map(|i| (i * 7 + 3) as u8).collect()
}

fn summary(app: &LinkedApp) -> String {
    format!(
        "{}\t{:08x}\t{:x}\t{:x}\t{}\t{:x}\t{}\t{:x}",
        app.flash.len(),
        crc32(&app.flash),
        app.app_end,
        app.data.flash_address,
        app.data.size,
        app.bss.address,
        app.bss.size,
        app.heap.address,
    )
}

#[test]
fn every_nwa_of_the_corpus_links_into_a_consistent_image() {
    let Some(files) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let layout = n0110_layout();
    let blob = external_data();
    let mut results = BTreeMap::new();

    for (name, bytes) in &files {
        let app = link(bytes, Some(&blob), &layout, &LinkOptions::default())
            .unwrap_or_else(|e| panic!("{name}: {e}"));

        // En-tête d'application.
        let word = |i: usize| u32::from_le_bytes(app.flash[4 * i..4 * i + 4].try_into().unwrap());
        assert_eq!(
            (word(0), word(7)),
            (0xdec0_beba, 0xdec0_beba),
            "{name}: magic"
        );
        assert!(!app.name.is_empty(), "{name}: nom vide");
        assert_eq!(word(1), app.api_level, "{name}");

        // Tout tient dans les zones réservées.
        assert!(
            app.flash.len() as u32 <= layout.flash_length,
            "{name}: flash"
        );
        let ram_end = layout.ram_end();
        assert_eq!(
            u64::from(app.heap.address) + u64::from(app.heap.size),
            ram_end,
            "{name}"
        );
        assert!(app.data.address >= layout.ram_start, "{name}");
        assert!(
            app.bss.address >= app.data.address + app.data.size,
            "{name}"
        );
        assert!(app.heap.address >= app.bss.address + app.bss.size, "{name}");

        // Le point d'entrée est dans l'image, en mode Thumb.
        assert_eq!(app.entry & 1, 1, "{name}");
        let entry = (app.entry & !1) - app.flash_start;
        assert!(
            (entry as usize) < app.flash.len(),
            "{name}: entrée hors de l'image"
        );
        assert_eq!(
            &app.flash[entry as usize..entry as usize + 2],
            [0x10, 0xb5],
            "{name}: _start"
        );

        results.insert(name.trim_end_matches(".nwa").to_owned(), summary(&app));
    }

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    if std::env::var_os("NWASABI_UPDATE_GOLDEN").is_some() {
        let mut text = String::from(
            "# nom\ttaille\tcrc32\tapp_end\tdata_flash\tdata_taille\tbss_début\tbss_taille\ttas_début\n",
        );
        for (name, line) in &results {
            text += &format!("{name}\t{line}\n");
        }
        std::fs::write(&path, text).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{GOLDEN} illisible ({e}) ; le générer avec NWASABI_UPDATE_GOLDEN=1")
    });
    let expected: BTreeMap<&str, &str> = golden
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .filter_map(|l| l.split_once('\t'))
        .collect();
    for (name, line) in &results {
        match expected.get(name.as_str()) {
            Some(want) => assert_eq!(line, want, "{name}: l'image liée a changé"),
            None if support::corpus_required() => panic!(
                "{name} absent de {GOLDEN} (NWASABI_UPDATE_GOLDEN=1 pour l'ajouter)"
            ),
            None => eprintln!(
                "{name} absent de {GOLDEN} : comparaison ignorée (NWASABI_UPDATE_GOLDEN=1 pour l'ajouter)"
            ),
        }
    }
}
