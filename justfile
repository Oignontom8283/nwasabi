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

# Compile la bibliothèque en WebAssembly (sans les binaires). SIMD et copies en bloc : mêmes options que `build.rs`.
build-wasm:
    RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+simd128,+bulk-memory" cargo build --lib --profile wasm --no-default-features --features wasm --target {{wasm_target}}

# Compile tout ce qui peut l'être : serveur, CLI et WebAssembly.
build-all: build-server build-cli build-wasm

# Prépare la page d'exemple : copie le module WebAssembly et @nwasabi/js (compilé) à côté de la page.
example: build-wasm build-js
    cp target/{{wasm_target}}/wasm/nwasabi.wasm example/nwasabi.wasm
    rm -rf example/js && cp -r packages/js/dist example/js

# Prépare la page d'exemple puis la sert sur http://localhost:8000/ (à lancer de n'importe quel dossier).
example-serve: example
    python3 -m http.server -d {{justfile_directory()}}/example 8000

# --- Émulateur web et site de documentation ---------------------------------
# Les recettes qui lancent un site (`emulator`, `docs`) utilisent Vite/VitePress en développement ; celles
# qui construisent (`build-emulator`, `build-docs`) compilent en release. Chaque recette s'appuie sur les
# précédentes et saute ce qui est déjà à jour : relancée sans changement, elle ne reconstruit rien ; si une
# étape manque ou est périmée, elle la refait.

wasm_file := "target/" + wasm_target + "/wasm/nwasabi.wasm"

# Installe les paquets npm d'un dossier s'ils manquent ou si le fichier de verrouillage a changé (les
# workspaces npm sont tous à la racine : `npm-install "."`).
[private]
npm-install dir:
    @cd {{dir}} && if [ ! -d node_modules ] || [ package-lock.json -nt node_modules/.package-lock.json ]; then npm ci --no-audit --no-fund; fi

# Compile @nwasabi/js (TypeScript) dans packages/js/dist.
[private]
build-js: (npm-install ".")
    @if [ ! -f packages/js/dist/index.js ] || [ -n "$(find packages/js/src packages/js/package.json packages/js/tsconfig.json -newer packages/js/dist/index.js | head -n 1)" ]; then npm run build -w @nwasabi/js; else echo "@nwasabi/js à jour"; fi

# Compile le module WebAssembly, @nwasabi/js puis l'émulateur (web/) en release dans web/dist, sans le lancer.
build-emulator: build-wasm build-js
    @cd web && if [ ! -f dist/index.html ] || [ -n "$(find src index.html vite.config.ts package.json ../packages/js/dist ../{{wasm_file}} -newer dist/index.html | head -n 1)" ]; then npm run build; else echo "émulateur à jour (web/dist)"; fi

alias build-emu := build-emulator

# Lance l'émulateur seul dans le navigateur (Vite en développement, rechargement à chaud), comme `nwasabi` le sert.
emulator: build-wasm build-js
    cd web && npm run dev -- --open

alias emu := emulator
alias web := emulator

# Place l'émulateur (compilé en release) dans le site de documentation, à l'adresse /emulator/.
[private]
docs-emulator: build-emulator
    @mkdir -p docs/public && for f in logo logo-bordered; do cmp -s assets/logo/$f.svg docs/public/$f.svg || cp assets/logo/$f.svg docs/public/$f.svg; done
    @if [ ! -f docs/public/emulator/index.html ] || [ web/dist/index.html -nt docs/public/emulator/index.html ]; then rm -rf docs/public/emulator && mkdir -p docs/public && cp -r web/dist docs/public/emulator; else echo "émulateur déjà dans la documentation"; fi

# `SITE_BASE=/nwasabi/ just build-docs` pour un site publié dans un sous-dossier. Construit le site statique complet (VitePress + émulateur à /emulator/) dans docs/.vitepress/dist.
build-docs: docs-emulator (npm-install ".")
    @cd docs && if [ ! -f .vitepress/dist/index.html ] || [ -n "$(find index.html index.md .vitepress/config.mjs public package.json -newer .vitepress/dist/index.html 2>/dev/null | head -n 1)" ]; then npm run build; else echo "documentation à jour (docs/.vitepress/dist)"; fi

alias build-doc := build-docs

# Lance le site de documentation (VitePress en développement) ; son émulateur à /emulator/ est la version release.
docs: docs-emulator (npm-install ".")
    cd docs && npm run dev -- --open

alias doc := docs

# --- Paquets npm ---------------------------------------------------------------

# Prépare @nwasabi/js et @nwasabi/wasm (compilation TypeScript, module .wasm copié) ; ne publie rien.
build-npm: build-wasm (npm-install ".")
    npm run build -w @nwasabi/js
    npm run build -w @nwasabi/wasm
    npm run copy-wasm -w @nwasabi/wasm

# Montre ce que contiendraient les paquets npm, sans rien écrire ni publier.
pack-npm: build-npm
    npm pack --dry-run -w @nwasabi/js -w @nwasabi/wasm

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

# Lance les tests natifs (architecture de la machine hôte). Si le dossier local/nwa existe, les .nwa de tiers qu'il contient
# sont aussi testés ; sinon (ou si NWASABI_CORPUS ne désigne rien), ces tests sont ignorés sans avertissement ni erreur.
test:
    cargo test --all-features

# Lance les tests natifs avec un corpus d'applications .nwa de son choix (dossier de fichiers .nwa), à la place de local/nwa.
test-corpus corpus:
    NWASABI_CORPUS={{corpus}} cargo test --all-features

# Teste le module WebAssembly sous Node.js. Les tests qui demandent des .nwa de tiers utilisent le dossier local/nwa s'il existe
# (ou celui donné : `just test-wasm chemin/du/corpus`) et sont ignorés sans avertissement ni erreur sinon.
test-wasm corpus="": build-wasm build-js
    NWASABI_CORPUS={{corpus}} node tests/wasm/parity.mjs
    NWASABI_CORPUS={{corpus}} node tests/wasm/player.mjs
    NWASABI_CORPUS={{corpus}} node tests/wasm/node.mjs
    NWASABI_CORPUS={{corpus}} node tests/wasm/speed.mjs
    node tests/wasm/keymap.mjs
    node tests/wasm/state.mjs
    node tests/wasm/step.mjs
    NWASABI_CORPUS={{corpus}} node tests/wasm/data.mjs
    node tests/web/locales.mjs
    node tests/web/scan.mjs
    node tests/web/gif.mjs
    node tests/web/snapshot.mjs
    node tests/web/faultregs.mjs

# Vérifie les types TypeScript de l'interface web (web/src, Vue et TypeScript strict).
typecheck-web: build-js
    cd web && npm run typecheck

# Recompile les applications de test en C (tests/apps/src → tests/apps/bin). Demande arm-none-eabi-gcc et
# nwlink (`npx` le télécharge) ; on ne le fait qu'en modifiant les sources, les .nwa sont versionnés.
build-test-apps:
    tests/apps/build.sh

# Régénère les résultats de référence de l'édition de liens après un changement voulu.
update-golden corpus:
    NWASABI_UPDATE_GOLDEN=1 NWASABI_CORPUS={{corpus}} cargo test --test link_corpus --test emulator_corpus

# Séquence RAPIDE avant de proposer un changement : formatage, clippy, tests natifs, WebAssembly. Pour tout vérifier, voir `check-all`.
ci: fmt-check clippy test build-wasm build-js typecheck-web

# Contenu : formatage, clippy, compilation de tout (serveur, CLI, WebAssembly), tous les tests Rust (dont les applications
# de test en C, les vecteurs QEMU et les fichiers de référence), tests WebAssembly sous Node.js, émulateur web, site de
# documentation et contenu des paquets npm. Les tests qui demandent des .nwa de tiers (parité, références) utilisent local/nwa s'il
# existe, ou le dossier donné (`just check-all chemin/du/corpus`), et sont sinon ignorés sans avertissement ni erreur. Ne recompile pas les applications de test en C
# (voir `build-test-apps`) et ne publie rien.
# VÉRIFICATION INTÉGRALE, LONGUE : tout contrôler (qualité, tests, compilations, web, docs, npm). Voir `ci` pour la version rapide.
check-all corpus="": fmt-check clippy typecheck-web build-all (test-all corpus) (test-wasm corpus) build-emulator build-docs pack-npm
    @echo "check-all : tout est vérifié"

# Tests natifs, avec le corpus s'il est donné (sinon local/nwa s'il existe, sinon ces tests sont ignorés en silence).
[private]
test-all corpus="":
    @if [ -n "{{corpus}}" ]; then just test-corpus "{{corpus}}"; else just test; fi

# --- Ménage -----------------------------------------------------------------

# Supprime les fichiers de compilation.
clean:
    cargo clean

    rm -rf node_modules web/node_modules docs/node_modules
    rm -rf web/dist packages/js/dist packages/wasm/dist docs/.vitepress/dist docs/public/emulator
    rm -rf tests/apps/build

    rm -rf ./example/nwasabi.wasm ./example/js