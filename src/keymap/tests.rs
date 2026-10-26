use super::*;

fn fixtures() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nmk")
}

fn names(chord: &[Key]) -> Vec<&'static str> {
    chord.iter().map(|k| k.name()).collect()
}

#[test]
fn the_default_map_is_valid_and_has_the_arrow_swipes() {
    let map = KeyMap::default_map();
    assert_eq!(names(map.resolve("", "ArrowUp").unwrap()), ["up"]);
    assert_eq!(names(map.resolve("", " ").unwrap()), ["ok"]);
    assert_eq!(map.rules().len(), 4);
    assert!(map.rules().iter().all(|r| r.fingers == 1));
}

#[test]
fn a_minimal_file_is_the_default_map() {
    let map = KeyMap::parse(r#"{"nmk":1}"#).unwrap();
    let mut default = KeyMap::default_map();
    assert_eq!(map.name(), None);
    default.name = None;
    assert_eq!(map, default);
}

#[test]
fn the_code_wins_over_the_character() {
    let map = KeyMap::parse(r#"{"nmk":1,"keyboard":{"KeyE":"ok","key:e":"back"}}"#).unwrap();
    assert_eq!(names(map.resolve("KeyE", "e").unwrap()), ["ok"]);
    assert_eq!(names(map.resolve("KeyX", "e").unwrap()), ["back"]);
    assert_eq!(map.resolve("KeyX", "x"), None);
}

#[test]
fn null_removes_a_default_binding_and_a_default_rule() {
    let map = KeyMap::parse(
        r#"{"nmk":1,"keyboard":{"key:e":null},
            "touch":{"rules":[{"gesture":"swipe_up","key":null}]}}"#,
    )
    .unwrap();
    assert_eq!(map.resolve("", "e"), None);
    assert!(map.rules().iter().all(|r| r.gesture != Gesture::SwipeUp));
    assert_eq!(map.rules().len(), 3);
}

#[test]
fn a_rule_replaces_the_default_rule_of_the_same_gesture_and_fingers() {
    let map = KeyMap::parse(
        r#"{"nmk":1,"touch":{"rules":[{"gesture":"swipe_left","key":"back"},
                                      {"gesture":"swipe_left","fingers":2,"key":"home"}]}}"#,
    )
    .unwrap();
    let left: Vec<_> = map
        .rules()
        .iter()
        .filter(|r| r.gesture == Gesture::SwipeLeft)
        .collect();
    assert_eq!(left.len(), 2);
    assert!(
        left.iter()
            .any(|r| r.fingers == 1 && names(&r.keys) == ["back"])
    );
}

#[test]
fn extends_none_starts_from_nothing() {
    let map = KeyMap::parse(r#"{"nmk":1,"extends":"none"}"#).unwrap();
    assert_eq!(map.keyboard().count(), 0);
    assert!(map.rules().is_empty());
}

#[test]
fn host_keys_are_parsed_and_printed_the_same_way() {
    for text in ["KeyW", "Digit1", "key:e", "key: ", "key:é"] {
        assert_eq!(HostKey::parse(text).unwrap().to_string(), text);
    }
    for text in ["", "key:", "char:a", "Key W", "a:b"] {
        assert!(HostKey::parse(text).is_err(), "{text:?}");
    }
}

#[test]
fn resolve_named_takes_a_command_line_name() {
    let map = KeyMap::parse(r#"{"nmk":1,"keyboard":{"KeyW":"up"}}"#).unwrap();
    assert_eq!(names(map.resolve_named("KeyW").unwrap()), ["up"]);
    assert_eq!(names(map.resolve_named("key:e").unwrap()), ["exe"]);
    assert_eq!(map.resolve_named("KeyZ"), None);
}

/// Les fichiers de `tests/nmk/` : `X.nmk` valide avec `X.expected.json` (le mapping complet), ou
/// `error_*.nmk` avec `error_*.expected.json` (l'endroit de l'erreur). `@nwasabi/js` rejoue les mêmes.
#[test]
fn shared_fixtures() {
    let mut count = 0;
    let mut files: Vec<_> = std::fs::read_dir(fixtures())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "nmk"))
        .collect();
    files.sort();
    for path in files {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        let expected_path = path.with_extension("expected.json");
        let actual = match KeyMap::parse(&text) {
            Ok(map) => map.to_json(),
            Err(e) => Value::object().with("error_path", e.path),
        };
        if std::env::var_os("NWASABI_UPDATE_GOLDEN").is_some() {
            let pretty = serde_json::to_string_pretty(&actual).unwrap() + "\n";
            std::fs::write(&expected_path, pretty).unwrap();
        }
        let expected: Value = json::parse(
            &std::fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display())),
        )
        .unwrap();
        assert_eq!(actual, expected, "{stem}");
        assert_eq!(
            stem.starts_with("error_"),
            actual.get("error_path").is_some(),
            "{stem}"
        );
        count += 1;
    }
    assert!(count >= 10, "{count} fichiers");
}
