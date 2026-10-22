# web

Interface web de nwasabi (Vue 3 + Vite). Elle est construite automatiquement quand on compile le
serveur (`cargo build`, `cargo install --path .`) : le script `build.rs` s'en occupe, le résultat est
embarqué dans le binaire `nwasabi`.

## Ce qu'il faut sur la machine

- Rust avec `rustup` (le script installe lui-même la cible `wasm32-unknown-unknown`) ;
- Node.js et `npm` (version 18 ou plus récente) ; les dépendances de `web/package.json` sont
  installées par `npm ci` au premier passage.

## Compiler sans Node

`NWASABI_NO_WEB=1 cargo build` compile un serveur sans interface : il affiche une page qui explique
comment l'obtenir. La ligne de commande (`--no-default-features --features cli`) et le module
WebAssembly n'ont jamais besoin de Node.

## Travailler sur l'interface

`just emulator` (alias `emu`, `web`) construit le module WebAssembly puis le site dans `web/dist/`
(ignoré par git), en sautant ce qui est déjà à jour. `npm run dev` lance Vite avec rechargement à
chaud ; il lit le module construit par `just build-wasm`. `just docs` assemble le site de
documentation (VitePress) qui contient cet émulateur à l'adresse `/emulator/`.
