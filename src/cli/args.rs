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

