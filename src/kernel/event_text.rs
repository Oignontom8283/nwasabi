//! Le texte que certains événements insèrent (touches de fonctions, chiffres, lettres…), rendu par
//! `eadk_event_copy_text`.
//!
//! La table est indexée par numéro d'événement (`touche + 54 × plan`, voir `keyboard`), de 0 à 215.
//! `None` : événement non défini ; `Some("")` : événement défini mais sans texte. Le caractère
//! `U+0011` marque la position du curseur après l'insertion (par exemple `sin(` puis `)`).
//! Valeurs relevées dans le noyau d'Epsilon 26.4.0 (identiques sur les trois modèles).

/// Nombre d'événements de la table.
pub const EVENT_COUNT: usize = 216;

#[rustfmt::skip]
const EVENT_TEXTS: [Option<&str>; EVENT_COUNT] = [
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    None,
    Some(""),
    None,
    None,
    None,
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    Some("e^(\u{11})"),
    Some("ln(\u{11})"),
    Some("log(\u{11})"),
    Some("i"),
    Some(","),
    Some("^"),
    Some("sin(\u{11})"),
    Some("cos(\u{11})"),
    Some("tan(\u{11})"),
    Some("π"),
    Some("√(\u{11})"),
    Some("^2"),
    Some("7"),
    Some("8"),
    Some("9"),
    Some("("),
    Some(")"),
    None,
    Some("4"),
    Some("5"),
    Some("6"),
    Some("×"),
    Some("/"),
    None,
    Some("1"),
    Some("2"),
    Some("3"),
    Some("+"),
    Some("-"),
    None,
    Some("0"),
    Some("."),
    Some("×10^"),
    Some(""),
    Some(""),
    None,
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(""),
    Some(""),
    Some(""),
    Some(""),
    Some("["),
    Some("]"),
    Some("{"),
    Some("}"),
    Some("_"),
    Some(""),
    Some("arcsin(\u{11})"),
    Some("arccos(\u{11})"),
    Some("arctan(\u{11})"),
    Some("="),
    Some("<"),
    Some(">"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(""),
    Some(""),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(":"),
    Some(";"),
    Some("\""),
    Some("%"),
    Some("a"),
    Some("b"),
    Some("c"),
    Some("d"),
    Some("e"),
    Some("f"),
    Some("g"),
    Some("h"),
    Some("i"),
    Some("j"),
    Some("k"),
    Some("l"),
    Some("m"),
    Some("n"),
    Some("o"),
    Some("p"),
    Some("q"),
    None,
    Some("r"),
    Some("s"),
    Some("t"),
    Some("u"),
    Some("v"),
    None,
    Some("w"),
    Some("x"),
    Some("y"),
    Some("z"),
    Some(" "),
    None,
    Some("?"),
    Some("!"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("A"),
    Some("B"),
    Some("C"),
    Some("D"),
    Some("E"),
    Some("F"),
    Some("G"),
    Some("H"),
    Some("I"),
    Some("J"),
    Some("K"),
    Some("L"),
    Some("M"),
    Some("N"),
    Some("O"),
    Some("P"),
    Some("Q"),
    None,
    Some("R"),
    Some("S"),
    Some("T"),
    Some("U"),
    Some("V"),
    None,
    Some("W"),
    Some("X"),
    Some("Y"),
    Some("Z"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];

/// Le texte d'un événement, s'il en a un (non vide).
pub fn event_text(id: u32) -> Option<&'static str> {
    EVENT_TEXTS
        .get(id as usize)
        .copied()
        .flatten()
        .filter(|text| !text.is_empty())
}

/// Vrai si l'événement existe : toutes les entrées de la table, plus les événements spéciaux
/// (le premier et le deuxième, puis les sept suivant le troisième).
pub fn is_defined(id: u32) -> bool {
    if (id as usize) < EVENT_COUNT {
        return EVENT_TEXTS[id as usize].is_some();
    }
    id.wrapping_add(0x28) & 0xff < 2 || id.wrapping_add(0x25) & 0xff < 7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_the_expected_shape() {
        assert_eq!(EVENT_TEXTS.iter().flatten().count(), 127);
        assert_eq!(event_text(24), Some("sin(\u{11})"));
        assert_eq!(event_text(30), Some("7"));
        assert_eq!(event_text(27), Some("π"));
        assert_eq!(event_text(126), Some("a"));
        assert_eq!(event_text(180), Some("A"));
    }

    #[test]
    fn events_without_text_are_still_defined() {
        // L'événement 6 (Home) existe mais n'insère rien ; 9 n'existe pas.
        assert_eq!(event_text(6), None);
        assert!(is_defined(6));
        assert!(!is_defined(9));
        assert!(!is_defined(10));
    }

    #[test]
    fn special_events_are_defined_after_the_table() {
        for id in [216, 217, 219, 220, 221, 222, 223, 224, 225] {
            assert!(is_defined(id), "{id}");
        }
        assert!(!is_defined(218));
        assert!(!is_defined(226));
    }
}
