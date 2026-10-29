//! Fichiers de mapping NMK : du clavier de l'ordinateur et des gestes tactiles vers les touches de la
//! calculatrice. Le format est décrit dans `docs/reference/formats/nmk.md`.
//!
//! Ce module lit et valide un fichier et répond à « quelle touche de la calculatrice pour cette touche
//! de l'ordinateur ? ». La reconnaissance des gestes elle-même se fait dans `@nwasabi/js`, au plus près
//! des événements du navigateur ; ici on ne garde que les règles.

use std::collections::BTreeMap;
use std::fmt;

use crate::json::{self, Value, ValueExt};
use crate::kernel::keyboard::Key;

/// Version du format que ce module sait lire.
pub const VERSION: u64 = 1;

/// Le mapping par défaut (celui qu'un fichier complète avec `"extends": "default"`).
pub const DEFAULT_SOURCE: &str = include_str!("default.nmk");

/// Une touche de l'ordinateur : sa position (`KeyboardEvent.code`) ou le caractère produit (`KeyboardEvent.key`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum HostKey {
    Code(String),
    Key(String),
}

impl HostKey {
    /// Lit `KeyW` (code) ou `key:e` (caractère).
    pub fn parse(text: &str) -> Result<HostKey, String> {
        match text.split_once(':') {
            Some(("key", value)) if !value.is_empty() && value.chars().count() <= 32 => {
                Ok(HostKey::Key(value.to_owned()))
            }
            Some(("key", _)) => Err("valeur vide ou trop longue après « key: »".into()),
            Some((prefix, _)) => Err(format!(
                "préfixe inconnu « {prefix}: » (seul « key: » existe)"
            )),
            None if !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric()) => {
                Ok(HostKey::Code(text.to_owned()))
            }
            None => Err(
                "un code de touche s'écrit avec des lettres et des chiffres (KeyW, Digit1…)".into(),
            ),
        }
    }
}

impl fmt::Display for HostKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostKey::Code(code) => f.write_str(code),
            HostKey::Key(key) => write!(f, "key:{key}"),
        }
    }
}

/// Touches de la calculatrice enfoncées ensemble.
pub type Chord = Vec<Key>;

/// Un geste tactile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gesture {
    Tap,
    DoubleTap,
    LongPress,
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
}

const GESTURE_NAMES: [(&str, Gesture); 7] = [
    ("tap", Gesture::Tap),
    ("double_tap", Gesture::DoubleTap),
    ("long_press", Gesture::LongPress),
    ("swipe_left", Gesture::SwipeLeft),
    ("swipe_right", Gesture::SwipeRight),
    ("swipe_up", Gesture::SwipeUp),
    ("swipe_down", Gesture::SwipeDown),
];

impl Gesture {
    pub fn name(self) -> &'static str {
        GESTURE_NAMES
            .iter()
            .find(|(_, g)| *g == self)
            .map_or("?", |(n, _)| n)
    }

    pub fn from_name(name: &str) -> Option<Gesture> {
        GESTURE_NAMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, g)| *g)
    }
}

/// Une règle tactile : ce geste, avec ce nombre de doigts, appuie sur ces touches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchRule {
    pub gesture: Gesture,
    pub fingers: u8,
    pub keys: Chord,
    /// Appui long seulement : la touche reste enfoncée tant que le doigt reste posé.
    pub while_held: bool,
}

/// Réglages de la reconnaissance des gestes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchSettings {
    pub swipe_distance: u32,
    pub long_press_ms: u32,
    pub double_tap_ms: u32,
    pub press_ms: u32,
}

impl Default for TouchSettings {
    fn default() -> Self {
        TouchSettings {
            swipe_distance: 40,
            long_press_ms: 500,
            double_tap_ms: 300,
            press_ms: 80,
        }
    }
}

/// Erreur de lecture : l'endroit du fichier (`keyboard.KeyW`, `touch.rules[1].gesture`) et le problème.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NmkError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for NmkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{} : {}", self.path, self.message)
        }
    }
}

impl std::error::Error for NmkError {}

fn error<T>(path: impl Into<String>, message: impl Into<String>) -> Result<T, NmkError> {
    Err(NmkError {
        path: path.into(),
        message: message.into(),
    })
}

/// Un mapping lu, complété par les liaisons par défaut si le fichier le demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMap {
    name: Option<String>,
    keyboard: BTreeMap<HostKey, Chord>,
    settings: TouchSettings,
    rules: Vec<TouchRule>,
}

/// Le fichier tel qu'il est écrit, avant d'être complété par le défaut.
struct Raw {
    name: Option<String>,
    extends_default: bool,
    keyboard: Vec<(HostKey, Option<Chord>)>,
    settings: [Option<u32>; 4],
    rules: Vec<(Gesture, u8, Option<Chord>, bool)>,
}

impl KeyMap {
    /// Lit un fichier NMK.
    pub fn parse(text: &str) -> Result<KeyMap, NmkError> {
        let raw = parse_raw(text)?;
        let base = if raw.extends_default {
            Some(Self::default_map())
        } else {
            None
        };
        Ok(build(raw, base))
    }

    /// Le mapping par défaut.
    pub fn default_map() -> KeyMap {
        // Le défaut ne complète rien : il est lu sans passer par `extends`.
        let raw = parse_raw(DEFAULT_SOURCE).expect("default.nmk est valide");
        build(raw, None)
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Les liaisons du clavier, triées.
    pub fn keyboard(&self) -> impl Iterator<Item = (&HostKey, &[Key])> {
        self.keyboard.iter().map(|(k, v)| (k, v.as_slice()))
    }

    pub fn rules(&self) -> &[TouchRule] {
        &self.rules
    }

    pub fn settings(&self) -> TouchSettings {
        self.settings
    }

    /// Touches de la calculatrice pour une touche de l'ordinateur ; la position (`code`) prime sur le
    /// caractère (`key`).
    pub fn resolve(&self, code: &str, key: &str) -> Option<&[Key]> {
        self.keyboard
            .get(&HostKey::Code(code.to_owned()))
            .or_else(|| self.keyboard.get(&HostKey::Key(key.to_owned())))
            .map(Vec::as_slice)
    }

    /// Le mapping complet en JSON (après fusion avec le défaut), tel que les outils l'affichent et que les
    /// tests le comparent d'une implémentation à l'autre.
    pub fn to_json(&self) -> Value {
        let names = |chord: &[Key]| -> Value {
            chord
                .iter()
                .map(|k| Value::from(k.name()))
                .collect::<Vec<_>>()
                .into()
        };
        let mut keyboard = Value::object();
        for (host, chord) in self.keyboard() {
            keyboard.set(&host.to_string(), names(chord));
        }
        let rules: Vec<Value> = self
            .rules
            .iter()
            .map(|r| {
                Value::object()
                    .with("gesture", r.gesture.name())
                    .with("fingers", r.fingers)
                    .with("key", names(&r.keys))
                    .with("while_held", r.while_held)
            })
            .collect();
        let touch = Value::object()
            .with("swipe_distance", self.settings.swipe_distance)
            .with("long_press_ms", self.settings.long_press_ms)
            .with("double_tap_ms", self.settings.double_tap_ms)
            .with("press_ms", self.settings.press_ms)
            .with("rules", rules);
        Value::object()
            .with("name", self.name.clone().map_or(Value::Null, Value::from))
            .with("keyboard", keyboard)
            .with("touch", touch)
    }

    /// Comme [`KeyMap::resolve`], avec un nom donné en ligne de commande : `KeyW` ou `key:e`.
    pub fn resolve_named(&self, host: &str) -> Option<&[Key]> {
        self.keyboard
            .get(&HostKey::parse(host).ok()?)
            .map(Vec::as_slice)
    }
}

fn build(raw: Raw, base: Option<KeyMap>) -> KeyMap {
    let mut map = base.unwrap_or(KeyMap {
        name: None,
        keyboard: BTreeMap::new(),
        settings: TouchSettings::default(),
        rules: Vec::new(),
    });
    map.name = raw.name;
    for (host, chord) in raw.keyboard {
        match chord {
            Some(chord) => map.keyboard.insert(host, chord),
            None => map.keyboard.remove(&host),
        };
    }
    let [swipe, long, double, press] = raw.settings;
    map.settings.swipe_distance = swipe.unwrap_or(map.settings.swipe_distance);
    map.settings.long_press_ms = long.unwrap_or(map.settings.long_press_ms);
    map.settings.double_tap_ms = double.unwrap_or(map.settings.double_tap_ms);
    map.settings.press_ms = press.unwrap_or(map.settings.press_ms);
    for (gesture, fingers, chord, while_held) in raw.rules {
        map.rules
            .retain(|r| !(r.gesture == gesture && r.fingers == fingers));
        if let Some(keys) = chord {
            map.rules.push(TouchRule {
                gesture,
                fingers,
                keys,
                while_held,
            });
        }
    }
    map
}

fn parse_raw(text: &str) -> Result<Raw, NmkError> {
    let root = match json::parse(text) {
        Ok(v) => v,
        Err(e) => return error("", format!("JSON invalide : {e}")),
    };
    let Some(root) = root.as_object() else {
        return error("", "le fichier doit être un objet JSON");
    };
    allow_only(root, "", &["nmk", "name", "extends", "keyboard", "touch"])?;

    match root.get("nmk") {
        None => return error("nmk", "version du format obligatoire (\"nmk\": 1)"),
        Some(v) if v.as_u64() == Some(VERSION) => {}
        Some(v) => {
            return error(
                "nmk",
                format!("version {v} non prise en charge (seule {VERSION} l'est)"),
            );
        }
    }
    let name = match root.get("name") {
        None => None,
        Some(Value::String(s)) if s.chars().count() <= 100 => Some(s.clone()),
        Some(_) => return error("name", "une chaîne de 100 caractères au plus est attendue"),
    };
    let extends_default = match root.get("extends").map(|v| v.as_str()) {
        None | Some(Some("default")) => true,
        Some(Some("none")) => false,
        Some(_) => return error("extends", "« default » ou « none » est attendu"),
    };

    let mut keyboard = Vec::new();
    if let Some(section) = root.get("keyboard") {
        let Some(section) = section.as_object() else {
            return error("keyboard", "un objet est attendu");
        };
        for (id, value) in section {
            let path = format!("keyboard.{id}");
            let host = HostKey::parse(id).or_else(|m| error(&path, m))?;
            keyboard.push((host, parse_chord(value, &path, true)?));
        }
    }

    let mut settings = [None; 4];
    let mut rules = Vec::new();
    if let Some(section) = root.get("touch") {
        let Some(section) = section.as_object() else {
            return error("touch", "un objet est attendu");
        };
        allow_only(
            section,
            "touch",
            &[
                "swipe_distance",
                "long_press_ms",
                "double_tap_ms",
                "press_ms",
                "rules",
            ],
        )?;
        let limits = [
            ("swipe_distance", 5, 1000),
            ("long_press_ms", 50, 10_000),
            ("double_tap_ms", 50, 5_000),
            ("press_ms", 10, 5_000),
        ];
        for (slot, (field, min, max)) in settings.iter_mut().zip(limits) {
            if let Some(v) = section.get(field) {
                let path = format!("touch.{field}");
                match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
                    Some(n) if (min..=max).contains(&n) => *slot = Some(n),
                    _ => return error(path, format!("un entier de {min} à {max} est attendu")),
                }
            }
        }
        if let Some(list) = section.get("rules") {
            let Some(list) = list.as_array() else {
                return error("touch.rules", "une liste est attendue");
            };
            for (i, rule) in list.iter().enumerate() {
                let rule = parse_rule(rule, &format!("touch.rules[{i}]"))?;
                if rules
                    .iter()
                    .any(|r: &(Gesture, u8, Option<Chord>, bool)| r.0 == rule.0 && r.1 == rule.1)
                {
                    return error(
                        format!("touch.rules[{i}]"),
                        format!(
                            "règle en double pour {} avec {} doigt(s)",
                            rule.0.name(),
                            rule.1
                        ),
                    );
                }
                rules.push(rule);
            }
        }
    }

    Ok(Raw {
        name,
        extends_default,
        keyboard,
        settings,
        rules,
    })
}

fn parse_rule(value: &Value, path: &str) -> Result<(Gesture, u8, Option<Chord>, bool), NmkError> {
    let Some(rule) = value.as_object() else {
        return error(path, "un objet est attendu");
    };
    allow_only(rule, path, &["gesture", "fingers", "key", "while_held"])?;
    let gesture = match rule.get("gesture").and_then(Value::as_str) {
        Some(name) => Gesture::from_name(name).map_or_else(
            || {
                error(
                    format!("{path}.gesture"),
                    format!("geste inconnu « {name} »"),
                )
            },
            Ok,
        )?,
        None => return error(format!("{path}.gesture"), "geste obligatoire"),
    };
    let fingers = match rule.get("fingers") {
        None => 1,
        Some(v) => match v.as_u64() {
            Some(n @ 1..=3) => n as u8,
            _ => return error(format!("{path}.fingers"), "1, 2 ou 3 est attendu"),
        },
    };
    let Some(key) = rule.get("key") else {
        return error(
            format!("{path}.key"),
            "touche obligatoire (null pour retirer la règle par défaut)",
        );
    };
    let chord = parse_chord(key, &format!("{path}.key"), true)?;
    let while_held = match rule.get("while_held") {
        None => false,
        Some(Value::Bool(b)) => {
            if *b && gesture != Gesture::LongPress {
                return error(format!("{path}.while_held"), "réservé à long_press");
            }
            *b
        }
        Some(_) => return error(format!("{path}.while_held"), "vrai ou faux est attendu"),
    };
    Ok((gesture, fingers, chord, while_held))
}

/// Un nom de touche, ou une liste de noms ; `null` si `allow_null`.
fn parse_chord(value: &Value, path: &str, allow_null: bool) -> Result<Option<Chord>, NmkError> {
    let key = |value: &Value| -> Result<Key, NmkError> {
        let Some(name) = value.as_str() else {
            return error(path, "un nom de touche est attendu");
        };
        Key::from_name(name).map_or_else(
            || {
                error(
                    path,
                    format!("touche de la calculatrice inconnue « {name} »"),
                )
            },
            Ok,
        )
    };
    match value {
        Value::Null if allow_null => Ok(None),
        Value::Array(items) => {
            if items.is_empty() || items.len() > 4 {
                return error(path, "une combinaison compte de 1 à 4 touches");
            }
            let mut chord = Vec::new();
            for item in items {
                let k = key(item)?;
                if chord.contains(&k) {
                    return error(path, format!("touche « {} » répétée", k.name()));
                }
                chord.push(k);
            }
            Ok(Some(chord))
        }
        other => Ok(Some(vec![key(other)?])),
    }
}

/// Refuse les champs inconnus (une faute de frappe ne doit pas passer en silence).
fn allow_only(
    object: &serde_json::Map<String, Value>,
    path: &str,
    allowed: &[&str],
) -> Result<(), NmkError> {
    for field in object.keys() {
        if !allowed.contains(&field.as_str()) {
            let at = if path.is_empty() {
                field.clone()
            } else {
                format!("{path}.{field}")
            };
            return error(
                at,
                format!("champ inconnu (attendus : {})", allowed.join(", ")),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
