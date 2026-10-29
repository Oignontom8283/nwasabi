//! Les noms de symboles lisibles : C++ (Itanium) et Rust (ancien et v0) sont « manglés » par le compilateur
//! (`_ZN8numcraft5world5Chunk8generate17h3f2a…E`) ; on les remet sous leur forme écrite dans le code.

/// Le nom de `name` tel qu'on l'écrit dans le programme (`numcraft::world::Chunk::generate`, `EADK::Keyboard::State::keyDown(...)`) ; `name`
/// lui-même s'il n'est pas manglé.
pub fn demangle(name: &str) -> String {
    if let Ok(d) = rustc_demangle::try_demangle(name) {
        // `{:#}` : sans le condensé qui termine les noms de Rust (`::h3f2a…`).
        return format!("{d:#}");
    }
    if name.starts_with("_Z")
        && let Ok(symbol) = cpp_demangle::Symbol::new(name)
        && let Ok(text) = symbol.demangle()
    {
        return text;
    }
    name.to_owned()
}

/// Le nom qualifié d'une fonction, sans ses paramètres (`EADK::Keyboard::State::keyDown`) : pour retrouver une fonction par son nom.
pub fn qualified(name: &str) -> String {
    let full = demangle(name);
    if full == name {
        return full;
    }
    // On coupe à la parenthèse ouvrante du dernier niveau (les paramètres), hors des `<…>` des modèles.
    let mut depth = 0i32;
    for (i, c) in full.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            '(' if depth <= 0 => return full[..i].trim_end().to_owned(),
            _ => {}
        }
    }
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_are_unchanged() {
        assert_eq!(demangle("main"), "main");
        assert_eq!(demangle("eadk_display_push_rect"), "eadk_display_push_rect");
        assert_eq!(qualified("main"), "main");
    }

    #[test]
    fn cpp_names_are_demangled() {
        assert_eq!(
            demangle("_ZNK4EADK8Keyboard5State7keyDownENS0_3KeyE"),
            "EADK::Keyboard::State::keyDown(EADK::Keyboard::Key) const"
        );
        assert_eq!(
            qualified("_ZNK4EADK8Keyboard5State7keyDownENS0_3KeyE"),
            "EADK::Keyboard::State::keyDown"
        );
        assert_eq!(demangle("_Z10waitForExev"), "waitForExe()");
        assert_eq!(qualified("_Z3addii"), "add");
    }

    #[test]
    fn rust_names_lose_their_hash() {
        let mangled = "_ZN8numcraft5world5Chunk8generate17h0123456789abcdefE";
        assert_eq!(demangle(mangled), "numcraft::world::Chunk::generate");
        assert_eq!(qualified(mangled), "numcraft::world::Chunk::generate");
        // Un nom en v0.
        assert_eq!(demangle("_RNvCs1234_7mycrate4main"), "mycrate::main");
    }

    #[test]
    fn an_invalid_mangled_name_is_kept() {
        assert_eq!(demangle("_Zzzz"), "_Zzzz");
    }
}
