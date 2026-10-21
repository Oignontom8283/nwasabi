//! Analyse minimale des arguments : des mots, des options `--nom valeur` et des drapeaux `--nom`.

use std::collections::HashMap;

/// Arguments d'une sous-commande.
pub struct Args {
    pub positional: Vec<String>,
    options: HashMap<String, Vec<String>>,
}

impl Args {
    /// `flags` : options sans valeur ; toute autre `--option` prend la valeur suivante.
    pub fn parse(args: &[String], flags: &[&str]) -> Result<Args, String> {
        let mut positional = Vec::new();
        let mut options: HashMap<String, Vec<String>> = HashMap::new();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let Some(name) = arg.strip_prefix("--") else {
                positional.push(arg.clone());
                continue;
            };
            if let Some((name, value)) = name.split_once('=') {
                options
                    .entry(name.to_owned())
                    .or_default()
                    .push(value.to_owned());
            } else if flags.contains(&name) {
                options.entry(name.to_owned()).or_default();
            } else {
                let value = it
                    .next()
                    .ok_or_else(|| format!("l'option --{name} attend une valeur"))?;
                options
                    .entry(name.to_owned())
                    .or_default()
                    .push(value.clone());
            }
        }
        Ok(Args {
            positional,
            options,
        })
    }

    pub fn flag(&self, name: &str) -> bool {
        self.options.contains_key(name)
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.options.get(name)?.last().map(String::as_str)
    }

    pub fn values(&self, name: &str) -> &[String] {
        self.options.get(name).map_or(&[], Vec::as_slice)
    }

    /// Un entier (décimal ou `0x…`) donné en option.
    pub fn number(&self, name: &str) -> Result<Option<u64>, String> {
        let Some(text) = self.value(name) else {
            return Ok(None);
        };
        let parsed = match text.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => text.replace('_', "").parse(),
        };
        parsed
            .map(Some)
            .map_err(|_| format!("--{name} : « {text} » n'est pas un entier"))
    }

    /// Le seul argument positionnel attendu.
    pub fn one_positional(&self, what: &str) -> Result<&str, String> {
        match self.positional.as_slice() {
            [one] => Ok(one),
            [] => Err(format!("{what} manquant")),
            _ => Err("trop d'arguments".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn options_flags_and_words_are_separated() {
        let a = Args::parse(
            &strings(&[
                "app.nwa", "--model", "n0120", "--json", "--key=ok", "--key", "up",
            ]),
            &["json"],
        )
        .unwrap();
        assert_eq!(a.positional, ["app.nwa"]);
        assert_eq!(a.value("model"), Some("n0120"));
        assert!(a.flag("json"));
        assert_eq!(a.values("key"), ["ok", "up"]);
        assert!(!a.flag("absent"));
    }

    #[test]
    fn a_missing_value_is_an_error() {
        assert!(Args::parse(&strings(&["--model"]), &[]).is_err());
    }

    #[test]
    fn numbers_accept_hex_and_separators() {
        let a = Args::parse(
            &strings(&["--n", "0x10", "--m", "40_000_000", "--bad", "x"]),
            &[],
        )
        .unwrap();
        assert_eq!(a.number("n"), Ok(Some(16)));
        assert_eq!(a.number("m"), Ok(Some(40_000_000)));
        assert_eq!(a.number("absent"), Ok(None));
        assert!(a.number("bad").is_err());
    }
}
