---
title: "C++ et Rust"
description: "Comment le débogueur de Nwasabi comprend les informations DWARF de C++ et de Rust : noms qualifiés, classes et bases, méthodes virtuelles, références, énumérations à données, démanglage."
---

# C++ et Rust

::: tip En bref
C++ et Rust ajoutent des notions que C n'a pas : noms qualifiés, classes, méthodes, énumérations avec données. Nwasabi les retrouve dans le DWARF et dans les **noms manglés** des symboles.
:::


Le DWARF de C++ et de Rust décrit des notions que le C n'a pas. `debuginfo/parse.rs`, `types.rs`, `expr.rs` et `symbols.rs` les prennent en charge. Cette page dit comment ; ce qui marche ou non en pratique est dans le
[guide de débogage](../../guide/deboguer-c-cpp-rust.md).

## Noms qualifiés

Le préfixe d'espace de noms (`prefix_pass`, [DWARF](./dwarf.md)) donne aux **types** leur nom complet (`shapes::Rect`, `Stack<int, 4>`, `main::Counter`). Les **fonctions** et les **variables globales** reçoivent leur nom qualifié depuis leur **symbole** :
`DebugInfo::attach_symbols`, appelé par `Emulator::debug_info` avec les symboles de `LinkedApp::symbols`, associe chaque adresse de fonction (ou de variable) à son symbole manglé, qu'`symbols::qualified` ramène au nom écrit dans le code **sans ses
paramètres ni son condensé** (`numcraft::world::Chunk::generate`, `shapes::Shape::counter`). Les membres statiques, qui n'ont pas d'adresse dans leur classe, sont ainsi retrouvés.

La résolution d'un nom (`find_variable`, `function_named`, `functions_named`) essaie, dans l'ordre : le nom exact, le nom qualifié, puis le **suffixe** à une frontière `::` (`Rect::grow` trouve `shapes::Rect::grow`).

## Noms lisibles : démanglage

`symbols.rs` fait appel à deux caisses : `cpp_demangle` (ABI Itanium : `_ZNK4EADK8Keyboard5State7keyDownENS0_3KeyE` → `EADK::Keyboard::State::keyDown(EADK::Keyboard::Key) const`) et `rustc-demangle` (ancien format `_ZN…17h<hash>E` et format v0 `_RNvC…`).
`demangle(nom)` rend le nom lisible ou `nom` lui-même s'il n'est pas manglé ; le condensé final des noms Rust (`::h3f2a…`) est retiré (`{:#}`). Les symboles de la pile, du désassemblage, de `info symbol`, de `break` et du profil passent par là. La console web cherche
aussi `break` par nom démanglé (`gdbConsole.ts`).

## Classes C++

`TypeKind::Struct` porte `fields`, `union`, `methods` et `variants`. Un `Field` a un drapeau `base` : une **classe de base** (`DW_TAG_inheritance`) est un champ `<Base>` dont les champs se lisent directement (`find_field` descend dans les bases). Une `Method` est une
fonction membre déclarée dans la classe, avec son nom lié (`DW_AT_linkage_name`), son numéro de case virtuelle (`DW_AT_vtable_elem_location`), son `this` et ses paramètres. Les **références** (`TypeKind::Reference`) sont traversées partout : leur valeur est celle de l'objet
désigné.

**Les fonctions membres en ligne jamais utilisées ne sont pas compilées** : le DWARF les déclare, mais il n'y a pas d'adresse : l'appel donne « la méthode n'existe pas dans le programme ». Les **modèles** (`Stack<int, 4>`) sont des types ordinaires dont le nom contient les
arguments ; ceux que le programme n'instancie pas n'existent pas.

## Appels de méthodes

Voir [Expressions](./expressions.md#c-methodes-this-surcharges) : résolution du nom, adresse par symbole (`symbol_addresses`), **appel virtuel par la table des fonctions virtuelles**, `this` en `r0`, surcharges par genre d'arguments.

## Rust

- **Énumérations avec données** : `DW_TAG_variant_part` donne un `VariantPart { discriminant, variants }` ; chaque `Variant` a une valeur et des champs. La valeur lue est la variante active (`ValueKind::Variant`, `Circle {center = {x = 1, y = 2}, radius = 2}`).
- **Méthodes d'`impl` et de traits** : ce sont des fonctions dont le nom qualifié contient le type (`main::Counter::advance`, `<main::Shape as main::Area>::area`) ; `Type::méthode` s'appelle sur un objet (`counter.advance()`), le premier paramètre `self` étant l'objet ou
  une référence à lui.
- **Statiques** dans un espace de noms (`main::GLOBAL_TOTAL`) : retrouvées par leur symbole.
- **Tranches** (`&[T]`) : une structure à deux champs `data_ptr` et `length`.
- **`self` est une référence** : traversée, `self.count` fonctionne directement.

## Ce que le noyau d'exécution ne fournit pas

- **Constructeurs d'objets C++ globaux** : `_start` ne parcourt pas `.init_array` ; l'objet global est donc **non construit**. Construisez vos objets dans `main`.
- **Exceptions et RTTI** : pas de déroulement (`.ARM.exidx` n'est ni placée ni lue) ; compilez en `-fno-exceptions -fno-rtti`.
- **Rust** : `core` appelle des fonctions de panique que le projet doit fournir (ou on désactive les contrôles de dépassement et de bornes).

## Dans le code

| Où | Quoi |
|---|---|
| [`symbols.rs`](code:src/symbols.rs) | [`demangle`](code:src/symbols.rs#demangle), [`qualified`](code:src/symbols.rs#qualified) |
| [`prefix_pass`](code:src/debuginfo/parse.rs#prefix_pass) | les préfixes d'espaces de noms |
| [`struct_members`](code:src/debuginfo/parse.rs#struct_members), [`variant_part`](code:src/debuginfo/parse.rs#variant_part) | membres, bases, méthodes, variantes Rust |
| [`attach_symbols`](code:src/debuginfo/mod.rs#attach_symbols) | noms qualifiés depuis les symboles |
| [`find_variable`](code:src/debuginfo/mod.rs#find_variable), [`functions_named`](code:src/debuginfo/mod.rs#functions_named), [`best_overload`](code:src/debuginfo/mod.rs#best_overload) | résolution des noms |
| [`find_method`](code:src/debuginfo/mod.rs#find_method), [`method_address`](code:src/debuginfo/mod.rs#method_address) | méthodes |
| [`Method`](code:src/debuginfo/mod.rs#Method), [`VariantPart`](code:src/debuginfo/mod.rs#VariantPart), [`Variant`](code:src/debuginfo/mod.rs#Variant) | les types qui les portent |
| [`call_method`](code:src/debuginfo/expr.rs#call_method), [`find_field`](code:src/debuginfo/expr.rs#find_field) | côté expressions |
| [`gdbConsole.ts`](code:web/src/gdbConsole.ts) | `break` par nom démanglé dans la console de la page |

## Pourquoi passer par les symboles pour les noms de fonctions

Le DWARF de GCC donne à une fonction membre son **nom court** (`grow`) et un lien vers sa déclaration, mais pas toujours un nom complet utilisable. Le **symbole** du fichier (`_ZN6shapes4Rect4growEi`) contient tout : espaces de noms, classe, paramètres. Le démangler donne `shapes::Rect::grow(int)`, qui, privé de ses paramètres, est le nom qu'écrit le programmeur. Cela marche **aussi pour Rust**, dont les noms qualifiés viennent du même mécanisme.

## Pour modifier

- **Un nouveau langage qui produit du DWARF** (Zig, Go…) : ouvrir un exemple compilé, lire ce que `gimli` rend dans [`parse.rs`](code:src/debuginfo/parse.rs) (les `DW_TAG_*` inconnus sont ignorés sans erreur), puis ajouter ce qui manque. Les noms manglés supplémentaires vont dans [`symbols.rs`](code:src/symbols.rs).
- **Un format de démanglage de plus** : [`demangle`](code:src/symbols.rs#demangle) essaie `cpp_demangle` puis `rustc_demangle` ; ajouter un cas et un test avec un vrai symbole.
- **Tester** : les tests unitaires de [`symbols.rs`](code:src/symbols.rs) (démanglage de vrais symboles C++ et Rust) et [`debugger.rs`](code:tests/debugger.rs) ; pour du C++ ou du Rust réels, compiler un petit projet comme dans le [guide](../../guide/deboguer-c-cpp-rust.md) et le déboguer avec un vrai GDB (`gdb_real.rs` automatise ce contrôle pour le C).
