# tests/wasm

Tests de la version WebAssembly de nwasabi. Ils s'exécutent avec Node.js : aucun navigateur
n'est nécessaire.

Principe : la même application `.nwa` est exécutée une fois par la version native et une fois
par la version WebAssembly. Les deux doivent produire exactement le même écran. Les empreintes de
référence produites par la version native sont dans `tests/golden/screens.tsv`.

Lancer : `just test-wasm` (protocole, fichiers, réglages) ou `just test-wasm <dossier de .nwa>`
(avec la parité d'écran sur quelques applications). Le module est construit par
`just build-wasm` ; `NWASABI_WASM` permet d'en indiquer un autre.

Cargo ne regarde pas dans ce dossier (il ne prend que `tests/*.rs`) : ces tests se lancent
séparément de `cargo test`.
