---
title: "Déboguer dans l'émulateur"
description: "Points d'arrêt, observation, pas à pas, pile d'appels, code source, remontée dans le temps, console à la GDB, profil et registres de faute dans l'émulateur Nwasabi."
---

# Déboguer dans l'émulateur

Nwasabi émule le **processeur** (un Cortex-M7), pas toute la calculatrice : le système (Epsilon), les périphériques et les temps
d'accès de la vraie puce sont remplacés par des versions simplifiées (voir [Compatibilité](../reference/compatibilite.md)). On
débogue donc son code face à ce système simulé. Cette page décrit ce que le cœur d'émulation sait faire ; les mêmes fonctions sont
offertes par la page web, par le [protocole de session](../reference/protocole-session.md) et par [GDB](./deboguer-c-cpp-rust.md#gdb). Pour déboguer un projet en C, C++ ou Rust, voir [Déboguer un projet en C, C++ ou Rust](./deboguer-c-cpp-rust.md).

## Points d'arrêt

Un point d'arrêt est une adresse d'instruction : l'exécution s'arrête **avant** de l'exécuter. Il peut porter une **condition**
comparant un registre à une valeur non signée : `==`, `!=`, `<`, `<=`, `>`, `>=` ou `&` (au moins un bit en commun), par exemple
`r0 == 5`. Un point désactivé est conservé sans effet.

Dès qu'un point existe, l'émulation avance instruction par instruction au lieu d'exécuter par grandes tranches : elle est plus lente.

## Points d'observation

Un point d'observation surveille une plage d'octets en lecture, en écriture ou les deux, et enregistre pour chaque instruction
qui y touche : la dernière adresse atteinte, la valeur et le nombre d'accès. C'est la réponse à « qui modifie cette
variable ? ». Avec l'option d'interruption, l'exécution s'arrête juste **après** le premier accès.

## Pas à pas

- **pas** : une instruction ;
- **par-dessus** : comme le pas, mais un appel (`bl`, `blx`) est exécuté en entier ;
- **sortir** : jusqu'au retour de la fonction en cours.

## Pile d'appels

Avec les informations de débogage (application compilée avec `-g`), la pile est **exacte** : Nwasabi lit les tables d'appel
(`.debug_frame`, CFI) qui disent, pour chaque adresse, où la fonction a rangé son adresse de retour et comment retrouver le
cadre de l'appelant. Chaque cadre donne sa fonction, sa ligne, ses paramètres et ses variables.

Sans elles (ou dans du code qu'elles ne décrivent pas, comme une bibliothèque compilée sans `-g`), le jeu d'instructions Thumb
n'enchaîne pas les cadres de pile : la pile est alors **estimée**. Nwasabi cherche dans la pile les adresses de retour plausibles
(celles qui suivent un `bl` ou un `blx` du code de l'application) ; une vieille adresse restée dans la pile peut s'y glisser. Les
cadres estimés sont signalés comme tels.

## Noms de fonctions

Ils viennent des informations de débogage s'il y en a, sinon des symboles du fichier `.nwa` (voir
[Écrire une application](../guide/ecrire-une-application.md)) ; sans eux, seules les adresses sont connues.

## Déboguer le code source

Quand l'application est compilée avec `-g`, son `.nwa` contient des informations de débogage DWARF (versions 2 à 5) : Nwasabi en
tire les **lignes de source**, les **fonctions**, les **variables** (paramètres, locales, globales, blocs imbriqués), les
**types** (entiers, flottants, caractères, booléens, pointeurs, structures, unions, tableaux, énumérations, `typedef`, champs de
bits) et les **tables d'appel**. Elles sont relues à chaque chargement, avec les adresses définitives de l'application liée.

**Le texte des sources** n'est pas dans le `.nwa` : il ne contient que les noms des fichiers et les numéros de lignes. La page web
demande les fichiers (choix ou glisser-déposer, rattachés par leur nom) et les garde pour l'application. Avec le serveur local
(`nwasabi --nwa jeu.nwa --src dossier/`), elle les lit elle-même dans ce dossier, à chaque chargement : le texte suit vos
modifications. Le serveur ne sert que des fichiers de ce dossier, choisis par la fin de leur chemin ; il ne lit jamais un chemin
qu'on lui donnerait.

Ce que l'on peut faire :

- **points d'arrêt par ligne** (`fichier:ligne`, ou `ligne`) : une ligne sans code se pose sur la suivante qui en a ; **par fonction**,
  l'arrêt a lieu après le prologue ;
- **pas à la ligne** : *entrer* (entre dans la fonction appelée si elle a des lignes, sinon l'exécute), *par-dessus* (exécute les
  appels en entier) et *sortir* (jusqu'au retour dans la fonction appelante, en tenant compte de la récursion) ;
- **expressions** avec la syntaxe du C, évaluées dans le cadre choisi de la pile : variables, `a.b`, `p->b`, `t[2][3]`, `*p`, `&x`,
  constantes d'énumération, registres (`$r0`, `$sp`), arithmétique et bits, comparaisons, `?:`, conversions (`(char)n`, `(Point *)p`),
  `sizeof`, affectations (`x = 5`, `n += 2`, `i++`) et **appels de fonctions** de l'application (`fib(6)`, `rect.area()`) : voir
  [Déboguer en C, C++ et Rust](../guide/deboguer-c-cpp-rust.md#les-expressions) ;
- **valeurs** lues d'après le type : structures et tableaux dépliables, chaînes de `char *`, énumérations par leur nom, pointeurs que
  l'on déplie pour voir ce qu'ils désignent ;
- **modification** d'une variable ou d'un champ en mémoire (`set var x = 5`, ou un double-clic sur sa valeur) ;
- **observation d'une variable** : le point d'observation est posé à l'adresse de l'objet, avec sa taille ;
- **types** : `ptype` décrit une structure (champs, positions, taille) ou une énumération, `whatis` donne le type d'une expression.

Les limites sont celles du code compilé : en `-O2`, une variable peut vivre dans un registre un instant puis disparaître
(`<optimisé>`), une fonction « en ligne » n'a plus son propre cadre, et les lignes sautent. Pour suivre un programme pas à pas, compilez
la partie qui vous intéresse avec `-O0 -g`.

Le débogage fonctionne pour le **C**, le **C++** (espaces de noms, classes, fonctions virtuelles, références, modèles) et le **Rust**
(énumérations avec données, méthodes, tranches) ; les noms de fonctions de ces deux langages s'affichent sous leur forme lisible. Voir
[Déboguer en C, C++ et Rust](../guide/deboguer-c-cpp-rust.md).

## Profil

Le profil relève régulièrement l'instruction en cours ; les fonctions et adresses qui reviennent le plus souvent sont celles où
l'application passe son temps. Une application qui attend (écran, clavier) exécute trop peu d'instructions pour être profilée.

## Journal des appels système

Chaque appel au système simulé est noté : numéro, registres r0 à r3 à l'entrée, valeur de retour, adresse de l'appel. Le journal
garde les 2000 derniers appels. Voir [Appels système](../reference/appels-systeme.md).

## Remonter le temps

L'émulation est déterministe : le même départ donne toujours la même suite. Quand l'historique est démarré, l'émulateur garde des
**points de reprise** réguliers (processeur, mémoire vive, écran) et note les touches et le temps qui passe. Reculer, c'est rétablir
le point de reprise précédent puis rejouer jusqu'à l'endroit voulu : le résultat est exactement l'état d'alors. Si l'on avance
ensuite, l'historique se poursuit depuis cet endroit.

Opérations : pas arrière, reprise en arrière (jusqu'au point d'arrêt ou d'observation précédent, ou au début), saut à un instant.

L'historique couvre quelques dizaines de millions d'instructions. **Modifier la mémoire ou les registres à la main le remet à
zéro**, car ce qui n'a pas été enregistré ne peut pas être rejoué ; il en va de même si l'application écrit dans les fichiers de la
calculatrice.

## Commandes de la console

Les actions ci-dessus ont des commandes inspirées de GDB dans la console de la page.

| Commande | Effet |
|---|---|
| `break main`, `b 0x90570398`, `break main if r0 == 5` | point d'arrêt, éventuellement conditionnel |
| `delete [adresse]`, `info breakpoints` | retire (tous sans argument), liste |
| `watch 0x20020000 8`, `rwatch`, `awatch` | observe en écriture, en lecture, ou les deux |
| `info watchpoints`, `unwatch 3` | liste, retire |
| `continue`, `pause` | reprend, interrompt |
| `stepi [n]`, `nexti [n]`, `finish` | une instruction, sans entrer dans les appels, jusqu'au retour |
| `info registers [reg]`, `print $r0 + 4`, `set $r1 = 0x10` | registres, expressions |
| `x/NFU adresse` | lit la mémoire : N éléments, format (`x d u o t c a s i`), taille (`b h w g`) |
| `x/5i main`, `disassemble main 8` | désassemble |
| `backtrace`, `info symbol 0x…` | pile d'appels, nom de la fonction |

Une adresse peut être un nombre, un registre (`$sp`, `$pc`…) ou un nom de fonction, avec `+N` ou `-N` (`main+8`).

Avec les informations de débogage du code source, les commandes de GDB sur les lignes et les variables s'ajoutent :

| Commande | Effet |
|---|---|
| `break fichier:ligne`, `break 42`, `break fonction` | point d'arrêt sur une ligne, ou après le prologue d'une fonction |
| `list [ligne\|fonction\|fichier:ligne]` | montre le code source autour de la ligne en cours (les fichiers sont donnés dans la vue Source) |
| `step`, `next`, `finish` | avance d'une ligne en entrant dans les appels, sans y entrer, jusqu'au retour de la fonction |
| `print expr`, `p expr` | valeur d'une expression C (`shape->flags[1]`, `*p`, `n + 1`) |
| `ptype expr\|type`, `whatis expr` | décrit un type, donne le type d'une expression |
| `set var x = 5` | modifie une variable |
| `info locals`, `info args`, `info frame`, `info source`, `info line [n]`, `info variables [nom]` | variables locales, arguments, cadre, fichiers, adresses d'une ligne, variables globales |
| `frame [n]`, `up`, `down` | choisit un cadre de la pile : `print` et `info locals` le suivent |
| `backtrace`, `watch variable` | pile avec fonctions, lignes et arguments ; observe l'adresse d'une variable |

Sans ces informations, `step` et `next` restent des pas d'instruction, comme `stepi` et `nexti`.

## Registres de faute

Quand une application fait quelque chose d'interdit (lire une adresse qui n'existe pas, exécuter une instruction indéfinie…), elle
**s'arrête** et la cause (en clair), l'instruction fautive et les registres sont rapportés.

Nwasabi n'exécute pas de gestionnaire d'exception, mais il remplit, comme le ferait le vrai processeur, les registres de faute du
bloc de contrôle du système (SCB), lisibles à leur adresse habituelle :

| Registre | Adresse | Contenu |
|---|---|---|
| `CPUID` | `0xE000ED00` | identifiant du processeur (Cortex-M7) |
| `ICSR` | `0xE000ED04` | exception active (3 : HardFault) |
| `CFSR` | `0xE000ED28` | cause de la faute (accès mémoire, instruction indéfinie, état invalide, accès non aligné) |
| `HFSR` | `0xE000ED2C` | faute « escaladée » en HardFault |
| `DFSR` | `0xE000ED30` | événement de débogage (`bkpt`) |
| `MMFAR` | `0xE000ED34` | adresse d'un accès interdit |
| `BFAR` | `0xE000ED38` | adresse d'un accès sur de la mémoire inexistante |

Les registres sont en lecture
seule pour l'application ; la relance de l'application les remet à zéro. Un débogueur branché par GDB les lit comme ceux d'une
vraie carte, après un plantage.

## Les limites

- **Le système est simulé** : ni périphériques STM32, ni amorçage, ni cache, ni temps d'accès à la flash.
- **Pas de gestionnaires d'exception** : une faute arrête l'application ; seuls les registres de faute sont remplis.
- **Le code optimisé se débogue mal** : variables optimisées, lignes qui sautent, fonctions en ligne (compilez en `-O0 -g` pour
  suivre un programme).
- **Sans `-g`**, la pile d'appels est estimée et il n'y a ni lignes ni variables, seulement les symboles du fichier.
- **Les sources ne sont pas dans le `.nwa`** : il faut les donner à la page.
- **L'historique est borné** et repart de zéro quand on modifie à la main la mémoire ou les registres.
