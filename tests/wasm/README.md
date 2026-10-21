# tests/wasm

Tests de la version WebAssembly de nwasabi. Ils s'exécutent avec Node.js : aucun navigateur
n'est nécessaire.

Principe : la même application `.nwa` est exécutée une fois par la version native et une fois
par la version WebAssembly. Les deux doivent produire exactement le même résultat (mêmes
pixels à l'écran, même somme de contrôle de la mémoire). Les résultats de référence produits
par la version native sont rangés dans `tests/golden/`.

Cargo ne regarde pas dans ce dossier (il ne prend que `tests/*.rs`) : ces tests se lancent
séparément de `cargo test`.

**État :** pas encore implémenté. Ce fichier expliquera comment les lancer quand la version
WebAssembly existera.
