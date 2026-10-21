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
