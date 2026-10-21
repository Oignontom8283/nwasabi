//! Installation des applications du corpus dans un appareil fabriqué (si `NWASABI_CORPUS` est défini).

mod support;

use nwasabi::device::DeviceImage;
use nwasabi::link::{LinkOptions, LinkWarning, link};
use nwasabi::profile::{FirmwareProfile, Model};

fn external_data() -> Vec<u8> {
    (0..1000u32).map(|i| (i * 7 + 3) as u8).collect()
}

fn fresh() -> DeviceImage {
    DeviceImage::new(FirmwareProfile::default_for(Model::N0110)).unwrap()
}

#[test]
fn every_app_installs_alone_and_is_listed_by_name() {
    let Some(files) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let blob = external_data();
    for (file, bytes) in &files {
        let mut device = fresh();
        let app = link(
            bytes,
            Some(&blob),
            &device.app_layout(),
            &LinkOptions::default(),
        )
        .unwrap_or_else(|e| panic!("{file}: {e}"));
        let address = device
            .install(&app)
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let apps = device.apps();
        assert_eq!(apps.len(), 1, "{file}");
        assert_eq!(apps[0].address, address, "{file}");
        assert_eq!(apps[0].name.as_deref(), Some(app.name.as_str()), "{file}");
        assert_eq!(apps[0].header.icon_size as usize, app.icon.len(), "{file}");
    }
}

#[test]
fn the_corpus_fills_the_apps_area_in_order() {
    let Some(files) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let blob = external_data();
    let mut device = fresh();
    let mut installed = Vec::new();
    let mut hidden_after: Option<usize> = None; // première application dont la suivante est perdue
    for (file, bytes) in &files {
        let Ok(app) = link(
            bytes,
            Some(&blob),
            &device.app_layout(),
            &LinkOptions::default(),
        ) else {
            continue; // elle ne tient plus : on essaie la suivante, plus petite peut-être
        };
        let mismatch = app
            .warnings
            .iter()
            .any(|w| matches!(w, LinkWarning::AppSizeMismatch { .. }));
        device.install(&app).unwrap();
        installed.push((file.clone(), app.name.clone()));
        if mismatch && hidden_after.is_none() {
            hidden_after = Some(installed.len());
        }
    }
    assert!(
        installed.len() >= 5,
        "seulement {} applications installées",
        installed.len()
    );
    // Le système voit toutes les applications installées, sauf celles qui suivent une taille d'en-tête
    // incohérente (particularité du format reproduite par nwasabi).
    let expected = hidden_after.unwrap_or(installed.len());
    let listed = device.apps();
    assert_eq!(listed.len(), expected, "{installed:?}");
    for (entry, (_, name)) in listed.iter().zip(&installed) {
        assert_eq!(entry.name.as_deref(), Some(name.as_str()));
    }
}
