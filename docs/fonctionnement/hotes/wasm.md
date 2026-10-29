---
title: "Le module WebAssembly"
description: "L'interface extern C du module wasm de Nwasabi (allocation, requête JSON, pixels), les options de compilation (SIMD, bulk-memory, LTO, wasm-opt) et la parité avec la version native."
---

# Le module WebAssembly

`src/wasm.rs` est tout ce que JavaScript voit. **Pas de `wasm-bindgen`** : l'échange se fait par la mémoire du module, avec quelques fonctions `extern "C"`.

| Fonction | Rôle |
|---|---|
| `nwasabi_alloc(len) -> *mut u8` | réserve `len` octets dans la mémoire du module |
| `nwasabi_free(ptr, len)` | les libère (même taille) |
| `nwasabi_request(ptr, len) -> usize` | traite la requête JSON de `len` octets écrite à `ptr` et rend la **longueur de la réponse** |
| `nwasabi_response_ptr() -> *const u8` | adresse de la dernière réponse (valable jusqu'à la requête suivante) |
| `nwasabi_framebuffer_ptr() -> *const u16` | adresse des pixels de l'écran (RGB565, 320×240) ou 0 sans appareil ; **change quand un nouvel appareil est créé** |
| `nwasabi_framebuffer_len() -> usize` | nombre de pixels |

Un appel typique : JavaScript écrit la requête UTF-8 dans un tampon d'`alloc`, appelle `request`, lit la réponse à `response_ptr`. Les requêtes et les réponses sont celles de `Session`. L'**écran se lit sans JSON**, directement en mémoire.
Un module WebAssembly ne tourne que dans un seul fil : l'état (`Session`) est un `thread_local`.

## Construction

```text
RUSTFLAGS="-C target-feature=+simd128,+bulk-memory"
cargo build --lib --profile wasm --no-default-features --features wasm --target wasm32-unknown-unknown
wasm-opt -O3 --enable-simd --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int --enable-mutable-globals
```

- **SIMD et copies mémoire en bloc** : les boucles de pixels et de copies sont vectorisées (environ 20 % de plus sur une application qui dessine beaucoup). Tous les navigateurs actuels les ont (Chrome 91, Firefox 89, Safari 16.4).
- **Profil `wasm`** : hérite de `release`, `opt-level = 3`, **LTO**, `codegen-units = 1`, `panic = "abort"`.
- **Binaryen** (`wasm-opt`, paquet npm `binaryen`) réécrit le module après la compilation : code mort, doublons, opérations plus courtes ; environ 10 % de moins, sans changer ce que fait le module.
- La caisse a deux types : `rlib` (binaires et tests) et `cdylib` (le module, avec la fonctionnalité `wasm`).

## Parité native / WebAssembly

Le même cœur tourne dans les deux : les **tests de parité** (`tests/wasm/parity.mjs`) exécutent les mêmes requêtes en natif et en WebAssembly et comparent les sommes de contrôle de l'écran, du nombre d'instructions et des arrêts ([Vérification](../verification/index.md)).
