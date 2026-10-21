# Recettes du projet. `just` sans argument liste les recettes disponibles.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Cible WebAssembly utilisée pour le navigateur et pour les tests sous Node.js.
wasm_target := "wasm32-unknown-unknown"

# Liste les recettes.
default:
    @just --list

# --- Compilation ------------------------------------------------------------

# Compile le serveur web `nwasabi` (feature `serve`, par défaut).
build-server:
    cargo build --bin nwasabi

# Compile l'outil de test natif `nwasabi-cli` (feature `cli`).
build-cli:
    cargo build --no-default-features --features cli --bin nwasabi-cli

# Compile la bibliothèque en WebAssembly (sans les binaires).
build-wasm:
    cargo build --lib --no-default-features --features wasm --target {{wasm_target}}

# Compile tout ce qui peut l'être : serveur, CLI et WebAssembly.
build-all: build-server build-cli build-wasm

# --- Lancement --------------------------------------------------------------

# Lance l'outil de test natif : `just cli run app.nwa --json`.
cli *args:
    cargo run --quiet --no-default-features --features cli --bin nwasabi-cli -- {{args}}

# Lance le serveur web : `just serve --nwa app.nwa --port 8080`.
serve *args:
    cargo run --quiet --bin nwasabi -- {{args}}

# --- Qualité et tests -------------------------------------------------------

# Vérifie le formatage sans rien modifier.
fmt-check:
    cargo fmt --check

# Formate le code.
fmt:
    cargo fmt

# Lance clippy sur toutes les cibles et features, en refusant les avertissements.
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Lance les tests natifs (architecture de la machine hôte).
test:
    cargo test --all-features

# Lance les tests natifs avec un corpus d'applications .nwa (dossier de fichiers .nwa).
test-corpus corpus:
    NWASABI_CORPUS={{corpus}} cargo test --all-features

# Régénère les résultats de référence de l'édition de liens après un changement voulu.
update-golden corpus:
    NWASABI_UPDATE_GOLDEN=1 NWASABI_CORPUS={{corpus}} cargo test --test link_corpus --test emulator_corpus

# Séquence complète avant de proposer un changement : formatage, clippy, tests, WebAssembly.
ci: fmt-check clippy test build-wasm

# --- Ménage -----------------------------------------------------------------

# Supprime les fichiers de compilation.
clean:
    cargo clean
