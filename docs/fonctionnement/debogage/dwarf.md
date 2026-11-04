---
title: "Informations de débogage DWARF"
description: "Lecture des sections DWARF avec gimli, structures simples DebugInfo, tables de lignes, fonctions, variables, listes d'emplacements, types, et évaluation des expressions d'emplacement."
---

# Informations de débogage DWARF

::: tip En bref
DWARF est la **carte** que le compilateur joint au programme : quelle ligne de source correspond à quelle adresse, où vit chaque variable, quel est son type. Nwasabi la lit une fois et en tire des structures simples.
:::


`debuginfo/` lit les sections `.debug_*` **une fois**, au premier usage (`Emulator::debug_info`, mis en cache : `Some(None)` si l'application n'en a pas), vers des **structures simples** (`DebugInfo`) : les requêtes
(adresse → ligne, ligne → adresses, variables visibles à une adresse, pile d'appels) n'ont plus besoin de `gimli`.

Les sections viennent de `LinkedApp::debug`, relocations déjà appliquées ([Application et export ELF](../edition-de-liens/application.md#informations-de-debogage)). La lecture utilise `gimli` 0.34 avec les fonctionnalités
`read`, `std` et `endian-reader`. Versions DWARF 2 à 5 ; testé avec le DWARF 5 de GCC 14.2 (C et C++) et le DWARF 4 de `rustc` 1.96.

## Structures de `DebugInfo`

| Champ | Contenu |
|---|---|
| `files` | chemins des fichiers de code (indice = numéro de fichier des lignes) |
| `rows: Vec<LineRow>` | la **table des lignes** : `address`, `file`, `line`, `column`, `is_stmt` (début d'instruction de source : où se pose un point d'arrêt de ligne), `end_sequence` |
| `functions: Vec<Function>` | `name`, `low`, `high`, `file`, `line`, `params`, `body: Scope`, `frame_base`, `ret`, `external`, `qualified` |
| `globals: Vec<Variable>` | variables globales (avec `qualified` en C++ et Rust) |
| `types: Vec<Type>` | `Type { name, size, kind }`, indexé par `TypeId` |
| `symbol_addresses` | adresses par **nom de symbole manglé** : sert à appeler une fonction membre (voir [C++ et Rust](./langages.md)) |
| `qualified_names` | vrai pour un programme C++ ou Rust : les noms de types s'écrivent sans `struct` |
| `frame` | la section `.debug_frame`, pour dérouler la pile |

`Scope { ranges, vars, children }` représente un **bloc `{ … }`** : ses variables ne sont visibles que dans ses adresses. `visible_locals(pc)` parcourt les blocs qui contiennent `pc`.

### Variables et emplacements

`Variable { name, ty, location, file, line, is_param, qualified, const_value }`. `Location` : `None`, `Expr(octets)` (une expression DWARF) ou `List(Vec<(début, fin, expression)>)`, la **liste d'emplacements** d'une variable
optimisée qui change de registre ou de pile selon l'adresse (`Location::at(pc)`). Une variable sans emplacement mais avec `DW_AT_const_value` (`const_value`) est affichée avec cette valeur.

### Types

`TypeKind` : `Unknown`, `Void`, `Base(Encoding)`, `Pointer`, `Reference` (C++, Rust : toujours traversée), `Struct { fields, union, methods, variants }`, `Array { elem, counts }` (une liste de bornes pour les tableaux
à plusieurs dimensions), `Enum { variants, under }`, `Typedef`, `Const`, `Volatile`, `Function { ret, params }`. `strip` retire `typedef`, `const` et `volatile` ; `type_name` produit le texte du type (`struct Item *`, `Stack<int, 4>`) ; `size_of` rend la taille.

Après la lecture, `types.rs::complete` ajoute les types que le programme n'a pas déclarés mais dont les expressions ont besoin : les types de base courants (`char`, `short`, `int`, `unsigned…`, `float`, `double`…), les pointeurs sur un ou
deux niveaux, et les tableaux multi-dimensionnels **dépliés en tableaux imbriqués** (pour que `a[i][j]` s'indexe un niveau à la fois).

## Du fichier DWARF aux structures : `parse.rs`

La lecture se fait en **plusieurs passes** sur les entrées (`DW_TAG_*`) :

1. **`prefix_pass`** : calcule, pour chaque entrée, son **préfixe d'espace de noms** (`shapes::`, `main::Counter::`) en parcourant `DW_TAG_namespace`, `DW_TAG_structure_type`, `DW_TAG_class_type`. Elle doit précéder la création des types : sans elle, un type
   (`Shape` en Rust) serait créé avant qu'on connaisse son préfixe et resterait non qualifié.
2. **Passe principale** : crée les types (`Type`), fonctions, paramètres, variables, blocs. Les profondeurs relatives de `gimli` servent à rattacher chaque entrée à son parent (`OpenKind` : `Function`, `Block`, `Namespace`, `Skip`) ;
   un espace de noms n'est pas un `Skip` (les globales d'un espace de noms rust seraient perdues).
3. **Rattachement** : les fonctions membres reçoivent leur nom qualifié depuis leur symbole (`attach_symbols`), les globales aussi (voir [C++ et Rust](./langages.md)).

Les lignes sont lues avec le programme de lignes (`.debug_line`), en gardant `is_stmt` et les fins de séquence.

## Évaluation des emplacements : `eval.rs`

`locate(expr, ctx) -> Place` évalue une **expression d'emplacement** DWARF avec une pile de valeurs. `Context` contient les registres `r0`–`r15` du cadre, les registres flottants, la CFA, la base du cadre (`DW_AT_frame_base`) et une
référence à la mémoire (`dyn Target`). Opérations gérées : `DW_OP_addr`, les constantes (`DW_OP_const*`, `DW_OP_lit0`–`31`, `DW_OP_constu/s`), la manipulation de la pile (`dup`, `drop`, `over`, `pick`, `swap`, `rot`), `DW_OP_deref` et `DW_OP_deref_size`, l'arithmétique et la logique sur 32 bits
(`and`, `minus`, `mod`, `mul`, `or`, `plus`, `shl`, `shr`, `shra`, `xor`, `div` signée), les comparaisons (`eq`, `ge`, `gt`, `le`, `lt`, `ne`), `DW_OP_plus_uconst`, `DW_OP_reg0`–`31` et `regx`, `DW_OP_breg0`–`31` et `bregx`,
`DW_OP_fbreg`, `DW_OP_call_frame_cfa`, `DW_OP_piece`, `DW_OP_stack_value`. `DW_OP_entry_value` (la valeur d'un paramètre à l'entrée de la fonction) est inconnue ici : `OptimizedOut`. Le résultat, un `Place`, est : une adresse en mémoire, un registre, une valeur calculée (octets de poids faible en premier), une **valeur répartie** (`Pieces`, plusieurs
morceaux avec leur taille) ou `OptimizedOut`.

`read_value(info, type, place, ctx)` lit la valeur d'après le **type** et rend un arbre de `Value` : entier signé ou non (avec le champ de bits), flottant, caractère, booléen, pointeur (avec la chaîne si c'est un `char *`), énumération (par son nom), structure et union
(champs, base C++ comme un champ `<Base>`), tableau (`counts` imbriqués), référence (traversée), variante Rust. `summary()` donne le texte d'une ligne, `to_json()` l'arbre déplié que la page affiche.

## Ce que le lecteur ne gère pas

DWARF **compressé** (`SHF_COMPRESSED`, `.zdebug`) ; DWARF **séparé** (`.dwo`, `.dwp`, fichiers de débogage externes) ; `.eh_frame` et les tables d'exceptions ARM comme source de pile (voir [Pile d'appels](./pile.md)) ; les types de la bibliothèque standard
C++ (aucune bibliothèque standard n'est liée). Du code compilé en `-O2` produit des variables `<optimisé>`, des fonctions « en ligne » sans cadre propre et des lignes qui sautent : ce sont des limites du code compilé, pas du lecteur.

## Dans le code

| Fichier | Rôle | À regarder |
|---|---|---|
| [`debuginfo/mod.rs`](code:src/debuginfo/mod.rs) | les structures et les requêtes | [`DebugInfo`](code:src/debuginfo/mod.rs#DebugInfo), [`LineRow`](code:src/debuginfo/mod.rs#LineRow), [`Function`](code:src/debuginfo/mod.rs#Function), [`Variable`](code:src/debuginfo/mod.rs#Variable), [`Scope`](code:src/debuginfo/mod.rs#Scope), [`Location`](code:src/debuginfo/mod.rs#Location), [`TypeKind`](code:src/debuginfo/mod.rs#TypeKind), [`line_at`](code:src/debuginfo/mod.rs#line_at), [`addresses_for_line`](code:src/debuginfo/mod.rs#addresses_for_line), [`after_prologue`](code:src/debuginfo/mod.rs#after_prologue), [`visible_locals`](code:src/debuginfo/mod.rs#visible_locals) |
| [`debuginfo/parse.rs`](code:src/debuginfo/parse.rs) | le lecteur `gimli` | [`parse`](code:src/debuginfo/parse.rs#parse), [`prefix_pass`](code:src/debuginfo/parse.rs#prefix_pass), [`OpenKind`](code:src/debuginfo/parse.rs#OpenKind) |
| [`debuginfo/types.rs`](code:src/debuginfo/types.rs) | types ajoutés après lecture | [`complete`](code:src/debuginfo/types.rs#complete), [`type_named`](code:src/debuginfo/types.rs#type_named) |
| [`debuginfo/eval.rs`](code:src/debuginfo/eval.rs) | emplacements et valeurs | [`locate`](code:src/debuginfo/eval.rs#locate), [`Place`](code:src/debuginfo/eval.rs#Place), [`read_value`](code:src/debuginfo/eval.rs#read_value), [`Value`](code:src/debuginfo/eval.rs#Value), [`Context`](code:src/debuginfo/eval.rs#Context) |
| [`debuginfo/unwind.rs`](code:src/debuginfo/unwind.rs) | pile par CFI | [`unwind`](code:src/debuginfo/unwind.rs#unwind) |
| [`emulator/`](code:src/emulator/mod.rs#debug_info) | la lecture à la demande | [`debug_info`](code:src/emulator/mod.rs#debug_info) |

## Pourquoi des « structures simples » plutôt que de garder `gimli`

`gimli` donne un accès paresseux, puissant et difficile à manier : des itérateurs, des décalages, des durées de vie. Chaque requête de la page (« quelles variables à cette adresse ? ») devrait le reparcourir. On **lit une fois** vers des `Vec` et des `HashMap`, et toutes les requêtes deviennent des recherches dans des tableaux. Le coût (mémoire, un seul parcours) est négligeable à l'échelle d'une application NumWorks.

## Pour modifier

- **Lire une nouvelle sorte d'entrée DWARF** : une branche dans la passe principale de [`parse.rs`](code:src/debuginfo/parse.rs) ; si elle contient des noms, tenir compte du préfixe d'espace de noms ([`prefix_pass`](code:src/debuginfo/parse.rs#prefix_pass)).
- **Une nouvelle opération d'emplacement DWARF** : [`locate`](code:src/debuginfo/eval.rs#locate) (un numéro d'opcode, un test dans `eval.rs`).
- **Une nouvelle forme de valeur** : [`TypeKind`](code:src/debuginfo/mod.rs#TypeKind) → [`read_value`](code:src/debuginfo/eval.rs#read_value) → [`summary`](code:src/debuginfo/eval.rs#summary) (texte) et [`to_json`](code:src/debuginfo/eval.rs#to_json) (page). Les trois doivent évoluer ensemble.
- **Tests** : [`debugger.rs`](code:tests/debugger.rs) sur [`debugme.c`](code:tests/apps/src/debugme.c), [`debugger_corpus.rs`](code:tests/debugger_corpus.rs) sur le corpus.
