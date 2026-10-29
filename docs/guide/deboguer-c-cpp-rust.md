# Déboguer une application en C, en C++ ou en Rust

Cette page montre, langage par langage, comment compiler une application pour qu'on puisse la déboguer ligne par ligne dans
Nwasabi, puis comment poser des points d'arrêt, lire des variables et appeler des fonctions. Les outils sont décrits dans
[Le débogage](../debug/index.md) ; ce qui est pris en charge, et ce qui ne l'est pas, dans
[Technologies prises en charge](../reference/technologies.md).

Dans tous les cas, le principe est le même : le compilateur écrit des **informations de débogage DWARF** dans le fichier objet, `nwlink`
les conserve dans le `.nwa`, et Nwasabi les lit au chargement. Elles ne sont pas dans l'image flashée sur la calculatrice : elles n'ajoutent
rien à la taille de l'application installée.

## Compiler

### C

```sh
CFLAGS="$(nwlink eadk-cflags-device) -O0 -g -ffreestanding -fdebug-prefix-map=$(pwd)=."
arm-none-eabi-gcc $CFLAGS -c main.c -o main.o
arm-none-eabi-gcc $CFLAGS $(nwlink eadk-ldflags-device) -nostdlib main.o icon.o -o app.nwa
```

`-O0` garde chaque variable en mémoire et chaque ligne à sa place ; `-g` écrit les informations de débogage ;
`-fdebug-prefix-map` remplace le dossier de compilation dans les chemins des sources (le fichier n'a plus le chemin de votre machine).

### C++

Les options sont celles du C, avec ce que le C++ demande de plus quand il n'y a ni système ni bibliothèque standard :

```sh
arm-none-eabi-g++ $CFLAGS -fno-exceptions -fno-rtti -fno-threadsafe-statics -c main.cpp -o main.o
arm-none-eabi-g++ $CFLAGS $(nwlink eadk-ldflags-device) -nostdlib main.o icon.o -o app.nwa
```

- pas d'**exceptions** ni de **RTTI** : il n'y a pas de dérouleur de pile pour les exceptions ;
- une classe qui a une fonction virtuelle pure demande `extern "C" void __cxa_pure_virtual() { for (;;) {} }` ;
- un destructeur virtuel demande les opérateurs `operator delete(void *)` et `operator delete(void *, unsigned int)` (vides, s'il n'y a pas
  de tas) ;
- les **constructeurs d'objets globaux ne sont pas exécutés** : construisez vos objets dans `main`.

### Rust

Une application Rust est un programme `no_std` et `no_main` dont `main` est une fonction C :

```rust
#![no_std]
#![no_main]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }

#[unsafe(link_section = ".rodata.eadk_app_name")] #[used] #[unsafe(no_mangle)]
pub static eadk_app_name: [u8; 4] = *b"Jeu\0";
#[unsafe(link_section = ".rodata.eadk_api_level")] #[used] #[unsafe(no_mangle)]
pub static eadk_api_level: u32 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> i32 { 0 }
```

On compile en fichier objet, puis on le lie comme une application en C :

```sh
rustc --edition 2024 --target thumbv7em-none-eabihf -C panic=abort -C opt-level=0 -C debuginfo=2 \
  -C target-cpu=cortex-m7 -C relocation-model=static --remap-path-prefix "$(pwd)=." --emit=obj -o main.o main.rs
arm-none-eabi-gcc $(nwlink eadk-cflags-device) $(nwlink eadk-ldflags-device) -nostdlib main.o icon.o -o app.nwa
```

La bibliothèque `core` appelle des fonctions de panique pour les contrôles de bornes et de dépassement : si votre projet ne les embarque pas,
l'édition de liens échoue sur `core::panicking::…`. Coupez ces contrôles (`-C overflow-checks=off`, `get_unchecked` à la place de l'indexation) ou
fournissez-les.

## Voir ce qu'on débogue

```sh
nwasabi --nwa app.nwa --src .
```

Ouvrez l'onglet **Débogage**, puis la vue **Source**. Le texte des fichiers n'est pas dans le `.nwa` : `--src` indique le dossier où le serveur les
lit (sans lui, glissez les fichiers dans la vue). Posez un point d'arrêt en cliquant à gauche d'une ligne, ou dans la console : `break main`,
`break fichier.c:42`.

Avec GDB, donnez-lui l'application liée au format ELF :

```sh
nwasabi --nwa app.nwa --gdb 3333        # puis cochez « Accepter un débogueur GDB » dans la vue GDB
nwasabi-cli link app.nwa --elf app.elf  # le même --model et le même --firmware que la page
gdb-multiarch app.elf
(gdb) target remote 127.0.0.1:3333
(gdb) break main
(gdb) continue
```

GDB reconnaît tout seul le langage (C, C++, Rust) d'après les informations de débogage.

## Les expressions

La console (`print`), la zone « Expressions suivies » de la vue Source et la commande `eval` du protocole évaluent des expressions avec la
syntaxe du C, quel que soit le langage du programme :

| Ce que l'on écrit | Exemples |
|---|---|
| variables, champs, tableaux, pointeurs | `total`, `shape->corner.x`, `grid[1][2]`, `*p`, `&n`, `p[3]` |
| arithmétique, comparaisons, bits, logique | `a + b * 2`, `n % 4 == 0`, `x & 0xff`, `x << 2 \| 1`, `a && !b` |
| flottants | `ratio * 2`, `-wide`, `1.5e2`, `(int)(ratio * 3)` |
| conversions et tailles | `(char)n`, `(unsigned char)-1`, `(Point *)p`, `*(int *)0x20000000`, `sizeof(Item)`, `sizeof grid` |
| conditions et virgule | `n > 2 ? a : b`, `(x = 1, x + 1)` |
| affectations et incréments | `x = 5`, `total += 2`, `i++`, `--count`, `words[1] = 99`, `$r0 = 7` |
| appels de fonctions | `fib(6)`, `add(2, 3)`, `op(3, 4)` par un pointeur de fonction |
| registres | `$r0` … `$r12`, `$sp`, `$lr`, `$pc` |
| caractères | `'A'`, `'\n'`, `'\x41'` |

Une affectation ou un appel **modifient l'appareil** : l'application reprend ensuite avec ces valeurs, et l'historique pour
[remonter le temps](../debug/index.md#remonter-le-temps) repart de là. Un appel exécute la fonction sur l'appareil (au plus 50 millions
d'instructions) puis rétablit le processeur : les registres et la pile sont ceux d'avant, la mémoire garde les effets de la fonction. Les
points d'arrêt posés dans la fonction appelée n'interrompent pas l'appel.

Les types se nomment comme dans le programme : `int`, `unsigned char`, `Point`, `struct Item`, `Kind`, `uint8_t`, avec `*` pour un pointeur.

## En C++

Les noms s'écrivent comme dans le code : `shapes::Rect`, `Stack<int, 4>`, `shapes::Shape::counter`. Les noms de fonctions sont lisibles
(`Stack<int, 4>::push`, et non `_ZN5StackIiLi4EE4pushERKi`) et `break` accepte le nom complet ou sa fin : `break Rect::grow`.

```
rect                        {<Shape> = {_vptr.Shape = 0x9057098c, name_ = 0x90570960 "rect", color_ = Green, id_ = 1}, origin_ = {x = 1, y = 2}, width_ = 3, height_ = 4}
rect.area()                 12                              (méthode virtuelle, par la table des fonctions virtuelles)
list[1]->area()             25                              (un pointeur de classe de base : la méthode de la classe réelle)
rect.grow(2)                <void>
(shapes::Rect *)list[0]     0x2003efa0
shapes::Shape::counter      2                               (membre statique)
combine(1, 2)               3     combine(1, 2, 3)  6     combine(1.5f, 2.0f)  3.0    (surcharges)
size_                       0                               (dans une fonction membre : this->size_)
```

- une **classe de base** s'affiche comme un champ `<Shape>` ; ses champs se lisent directement (`rect.id_`) ;
- une **référence** (`int &`) se lit comme l'objet qu'elle désigne ;
- une **énumération fortement typée** (`enum class`) s'affiche par le nom de ses valeurs ;
- une fonction membre **définie dans la classe et jamais appelée** n'est pas compilée : l'appeler donne « la méthode n'existe pas dans le programme ».

## En Rust

```
*self                       Circle {center = {x = 1, y = 2}, radius = 2}      (une énumération : la variante active et ses champs)
shapes                      {Circle {…}, Rect {corner = {x = 0, y = 0}, width = 3, height = 4}, Empty}
counter                     {count = 5, step = 5}
counter.advance()           5                                (une méthode : Type::méthode sur l'objet)
self.count                  0                                (self est une référence : elle est traversée)
main::GLOBAL_TOTAL          0                                (une variable globale, par son nom complet ou son nom seul)
factorial(5)                120
items                       {data_ptr = 0x2003efb0, length = 4}   (une tranche : son pointeur et sa longueur)
```

Les fonctions s'appellent par leur nom complet (`main::Counter::advance`) ; `break` accepte aussi le nom seul (`break advance`). Une
méthode d'un trait (`<main::Shape as main::Area>::area`) s'appelle sur l'objet : `shape.area()`.

## Ce qui ne marche pas (encore)

- un appel de fonction dont **le résultat est une structure ou un tableau** (une valeur Rust comme `Option<u32>`, un objet C++ rendu par valeur) :
  la fonction est exécutée, mais le résultat n'est pas affiché ;
- les **arguments structures** d'un appel, et au-delà de seize flottants ;
- les **chaînes de caractères littérales** dans une expression (`"texte"`) : elles n'ont pas d'adresse dans l'application ;
- en C++, les **fonctions membres en ligne jamais utilisées**, et les modèles que le programme n'a pas instanciés ;
- le code compilé avec `-O2` : variables optimisées (`<optimisé>`), fonctions en ligne sans cadre propre, lignes qui sautent.
