---
title: "Les expressions"
description: "Le langage d'expressions C de Nwasabi : lexique, grammaire, opérandes typés, conversions, affectations, appels de fonctions sur l'émulateur avec l'ABI AAPCS et effets sur l'historique."
---

# Les expressions

`debuginfo/expr.rs` (environ 1 400 lignes) évalue les expressions que l'on saisit pour regarder ou modifier une variable : `x`, `shape.corner.x`, `*p`, `&n`, `a[2][3]`, `n + 1`, `(char)n`, `sizeof(Shape)`, `x = 5`, `i++`, `n > 2 ? a : b`, `factorial(4)`, `$r0 = 1`.
**La syntaxe est celle du C**, quel que soit le langage de l'application (voir [C++ et Rust](./langages.md) pour ce qui s'y ajoute).

## Lexique

Le lexeur reconnaît : identifiants (`::` compris : `shapes::Rect`, `Stack<int, 4>::push`), registres (`$r0`…`$r15`, `$sp`, `$lr`, `$pc`), entiers (décimal, `0x`, suffixes `u`, `l`), flottants (`1.5`, `1e-3`, suffixe `f`), caractères (`'A'`, `'\n'`, `'\x41'`, `'\0'`),
et la ponctuation (`<<=`, `>>=`, `&&`, `||`, `==`, `!=`, `<=`, `>=`, `++`, `--`, `->`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<`, `>>`, `::`…).

## Grammaire

Un analyseur à descente récursive, par niveaux de priorité C :

```text
expression  := affectation (',' affectation)*
affectation := ternaire (('=' | '+=' | … ) affectation)?
ternaire    := binaire ('?' expression ':' ternaire)?
binaire     := niveaux : || && | ^ & ==,!= <,<=,>,>= <<,>> +,- *,/,%
unaire      := ('-' | '+' | '!' | '~' | '*' | '&' | '++' | '--' | sizeof | '(' type ')') unaire | postfixe
postfixe    := primaire ( '[' expression ']' | '(' arguments ')' | '.' nom | '->' nom | '++' | '--' )*
primaire    := nombre | caractère | registre | '(' expression ')' | identifiant (variable, fonction, constante d'énumération, type)
```

`try_type` décide si `(…)` est une conversion : il essaie de lire un **nom de type** (`unsigned char`, `struct Item`, `Point *`, `uint8_t`, `shapes::Rect *`). Les noms se comparent sans tenir compte des espaces et avec une **recherche par la fin** (`Rect` trouve
`shapes::Rect`), en excluant les déclarations sans contenu.

## Opérandes : `Operand`

Chaque sous-expression produit un `Operand` :

| `Operand` | Sens |
|---|---|
| `Int` | entier de 64 bits (signe porté séparément) |
| `Float` | `f64` |
| `Typed` | une valeur lue d'après un type, avec son lieu (modifiable) |
| `Object` | une structure, une union, un tableau : un lieu et un type |
| `Address` | une adresse avec un type pointé |
| `Func` | une fonction (adresse, prototype) |
| `Overloads` | plusieurs fonctions de même nom (C++) |
| `Register` | un registre du cadre |
| `Void` | rien (appel sans retour) |

L'arithmétique suit le C : promotion des petits entiers, entier signé 64 bits, non signé si un opérande l'est ; division par zéro → erreur ; `%` ; décalages bornés ; comparaisons rendent 0 ou 1 ; l'arithmétique de **pointeurs** multiplie par la taille de l'objet pointé
et `p - q` divise ; flottant dès qu'un opérande l'est. `cast` convertit en entier de la taille voulue (`truncate`), en flottant, en pointeur ; `(unsigned char)-1` donne 255.

## Écrire : `store`

Une affectation (`=` et les composées) et `++`/`--` écrivent par `Target::write` à l'adresse du lieu, tronquée à la taille du type, ou par `set_register` pour un registre. L'écriture en **flash est refusée** (`0x9000_0000..0xA000_0000`) : seul le noyau
simulé programme la flash. Un registre `$pc` écrit perd son bit 0 ; `$sp` ses deux bits de poids faible.

## Appeler une fonction : `Target::call`

`fib(6)`, `rect.area()`, `op(3, 4)` (pointeur de fonction). Le débogueur exécute **la fonction sur l'émulateur** :

1. sauvegarde du processeur, du noyau et du point de reprise (`resume_point`) ;
2. **arguments selon l'AAPCS à virgule flottante matérielle** : entiers dans `r0`–`r3` puis sur la pile ; `float` dans `s0`–`s15`, `double` dans les paires `d0`–`d7` (aligné) ; un 17ᵉ flottant est refusé (`trop d'arguments à virgule flottante`) ;
3. pile : `sp - 256`, alignée sur 8, avec les arguments de pile en dessous ;
4. **retour piégé** : `lr = entry | 1`, avec un point d'arrêt posé sur le point d'entrée de l'application ; `pc = fonction` ;
5. exécution par tranches de 200 000 instructions, **au plus 50 000 000** (`CALL_BUDGET`) ; une image, un `Budget` ou un autre point d'arrêt ne l'interrompent pas ; un `Sleep` fait passer le temps virtuel ; une **faute** ou un arrêt inattendu fait échouer l'appel ;
6. à l'arrivée sur le point d'arrêt, le résultat est lu : `r0`, `r1`, `s0`, `s1` (`CallResult`) ;
7. le **processeur et le noyau sont restaurés** ; la **mémoire garde les effets** de l'appel, comme avec GDB ; `dirty` est vrai.

Le type du résultat vient du prototype : entier selon sa taille (64 bits : `r0:r1`), flottant (`s0`) ou double (`s0:s1`), pointeur. Les fonctions qui **rendent une structure ou un tableau** sont exécutées mais leur résultat est refusé ; les **arguments
structures** ne sont pas gérés.

Un appel qui se fait **sur l'application en pause à n'importe quel point** exige que la pile soit saine : l'appel écrit sous `sp`.

## C++ : méthodes, `this`, surcharges

`call_method` cherche la méthode (`find_method`, `method_function`) dans le type (et ses bases), puis son adresse (`method_address`) : par le nom de fonction qualifié, ou, pour une méthode **virtuelle**, par la **table des fonctions virtuelles** — le pointeur `_vptr`
du lieu et le numéro de case donné par `DW_AT_vtable_elem_location`. `this` est passé en `r0`. Les **surcharges** (`Overloads`) se résolvent par `best_overload` d'après le nombre et le genre des arguments (entiers, flottants, pointeurs) ; les arguments à virgule
flottante suivent l'ABI ci-dessus. Dans une fonction membre, un nom inconnu se cherche en tant que membre de `*this` (**`this` implicite**).

## Effets sur l'historique

Chaque commande de source qui évalue (`eval`, `assign`) signale si elle a modifié l'appareil (`with_context` rend `(valeur, dirty)`) ; **`history_reset`** s'applique alors. Les lectures pures ne touchent pas à l'historique ([Historique](./historique.md)).
