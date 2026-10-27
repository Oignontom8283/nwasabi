# Débogage

L'onglet **Débogage** du panneau de l'émulateur regroupe les outils pour comprendre ce que fait une application, instruction par
instruction, comme avec un débogueur d'ordinateur. Le bouton **i** en haut à droite de l'onglet rappelle l'essentiel et renvoie à
cette page.

::: warning À savoir avant de commencer
Nwasabi émule le **processeur** (un Cortex-M7), pas toute la calculatrice. Le système (Epsilon), les périphériques et les temps
d'accès de la vraie puce sont remplacés par des versions simplifiées. Vous déboguez donc votre code face à ce système simulé, pas
face à la puce exacte. Voir [Les limites](#les-limites).
:::

## Les outils

L'onglet a six vues, choisies par la barre du haut.

### Code

Le code de l'application, **désassemblé** (instructions Thumb-2) autour de l'instruction en cours, marquée d'une flèche.

| Commande | Raccourci | Ce qu'elle fait |
|---|---|---|
| Reprendre / Pause | F5 | lance l'application, ou l'arrête sur l'instruction en cours |
| Pas | F11 | exécute une instruction, puis s'arrête |
| Par-dessus | F10 | comme « Pas », mais un appel de fonction (`bl`, `blx`) est exécuté en entier |
| Sortir | Maj + F11 | exécute jusqu'au retour de la fonction en cours |

Dans la liste du code :

- cliquez le **point à gauche** d'une ligne pour poser ou retirer un point d'arrêt ;
- cliquez une **adresse** pour exécuter jusqu'à elle ;
- cliquez la flèche **→** d'un saut ou d'un appel pour aller à sa cible, et l'icône de puce pour la voir dans la mémoire ;
- le champ **Aller à** accepte une adresse (`0x90570398`) ou un nom de fonction (`main`).

Les instructions qui font **sauter** l'exécution ailleurs portent une icône à gauche de l'adresse : saut (flèche), saut
conditionnel (flèche avec losange), appel de fonction (triangle), retour (flèche coudée), saut indirect (trois points). En
survolant une de ces lignes, une **flèche** va jusqu'à l'instruction visée (au bord de la liste quand elle est hors de vue).
Survoler le nom d'une instruction affiche ce qu'elle fait. La colonne **Actions** (à droite) mène à la cible d'un saut ou d'un
appel, ou la montre dans la mémoire.

Un **clic droit** sur une ligne ouvre un menu : exécuter jusqu'ici, poser un point d'arrêt (conditionnel ou non), reprendre à cette
adresse (`pc` prend sa valeur), aller à la cible d'un saut ou y poser un point d'arrêt, copier l'adresse, l'instruction ou ses
octets, et remplacer l'instruction par un `nop` (ce qui modifie le code de l'application).

Sous le code se trouvent :

- les **dernières instructions exécutées** en pas à pas, de la plus récente à la plus ancienne ;

- la **pile d'appels** (voir [Les limites](#les-limites) : elle est estimée) ;
- les **registres** r0 à r12, sp, lr et pc, modifiables quand l'application est en pause : tapez une valeur (décimale ou
  `0x…`) et validez avec Entrée ; ceux qui viennent de changer sont en orange ; les indicateurs N, Z, C, V, Q sont en gras
  quand ils sont levés ;
- les **points d'arrêt** posés ;
- les **registres de faute**, expliqués [plus bas](#registres-de-faute).

Les noms de fonctions viennent des symboles du fichier `.nwa` ; sans eux, seules les adresses s'affichent.

### Points

Tous les points d'arrêt et d'observation au même endroit. On peut en poser (avec une **condition** si besoin), les
**désactiver** sans les perdre, les réactiver ou les retirer.

Une condition compare un registre à une valeur : `r0 == 5`, `sp < 0x20010000`… Les comparaisons sont `==`, `!=`, `<`, `<=`,
`>`, `>=` et `&` (au moins un bit en commun). Les valeurs sont non signées. Le point d'arrêt n'interrompt l'application que si la
condition est vraie.

### Observation

Un **point d'observation** surveille des octets de la mémoire et indique **quelle instruction** de l'application les lit ou les
écrit. C'est la réponse à « qui modifie cette variable ? » :

1. repérez l'adresse de la variable avec la recherche de valeurs de l'onglet Mémoire (un score, des points de vie…) ;
2. posez un point d'observation en écriture sur cette adresse, avec ou sans « Interrompre » ;
3. la liste des accès montre, pour chaque instruction, la dernière adresse touchée, la valeur et le nombre de fois.

Avec « Interrompre », l'application s'arrête juste après le premier accès. Le bouton **Observer** de l'éditeur hexadécimal pose
un point d'observation en écriture sur la sélection.

### Console

Les mêmes actions au clavier, avec des commandes inspirées de GDB. Tapez `help` pour la liste.

| Commande | Effet |
|---|---|
| `break main`, `b 0x90570398` | pose un point d'arrêt |
| `break main if r0 == 5` | point d'arrêt conditionnel |
| `delete [adresse]`, `info breakpoints` | retire (tous sans argument), liste |
| `watch 0x20020000 8`, `rwatch`, `awatch` | observe en écriture, en lecture, ou les deux (interrompt l'application) |
| `info watchpoints`, `unwatch 3` | liste, retire |
| `continue`, `pause` | reprend, interrompt |
| `stepi [n]`, `nexti [n]`, `finish` | une instruction, sans entrer dans les appels, jusqu'au retour |
| `info registers [reg]`, `print $r0 + 4` | registres, valeur d'une expression |
| `set $r1 = 0x10` | modifie un registre |
| `x/8xw $sp` | lit la mémoire : `x/NFU adresse`, avec N éléments, un format (`x d u o t c a s i`) et une taille (`b h w g`) |
| `x/5i main`, `disassemble main 8` | désassemble |
| `backtrace`, `info symbol 0x…` | pile d'appels, nom de la fonction |

Une adresse peut être un nombre, un registre (`$sp`, `$pc`…), un nom de fonction, avec `+N` ou `-N` (`main+8`). Les flèches haut et
bas rappellent les commandes précédentes. Les commandes qui dépendent des informations de débogage du code source (`list`,
`info line`, `info locals`…) ne sont pas disponibles.

### Profil

Le profil relève régulièrement l'instruction en cours : les **fonctions** et les **adresses** qui reviennent le plus souvent sont
celles où l'application passe son temps. Lancez-le, laissez l'application tourner, et lisez les parts de temps. Cliquer une
fonction ou une adresse l'ouvre dans la vue Code. Une application qui passe son temps à attendre (écran, clavier) exécute trop
peu d'instructions pour être profilée.

### Appels système

Chaque appel de l'application au système de la calculatrice (écran, clavier, temps, fichiers…), du plus récent au plus ancien, avec
les registres r0 à r3 à l'entrée (les arguments, selon l'appel), la valeur de retour et l'adresse de l'appel. Le journal garde les
400 derniers appels ; on peut le filtrer par nom ou numéro, le figer ou le vider.

## Remonter le temps

L'émulation est déterministe : le même départ donne toujours la même suite. Nwasabi en profite pour **reculer**.

Dans la vue Code, cochez « Enregistrer pour remonter le temps ». L'émulateur garde alors des points de reprise réguliers (le
processeur, la mémoire vive, l'écran) et note les touches et le temps qui passe. Ensuite, quand l'application est en pause :

- **Pas arrière** revient une instruction en arrière ;
- **Reprendre en arrière** remonte jusqu'au point d'arrêt ou d'observation précédent, ou au début de l'historique ;
- le **curseur** revient à un instant précis de l'historique.

Revenir en arrière, c'est rétablir le point de reprise précédent puis rejouer jusqu'à l'endroit voulu : le résultat est exactement
l'état d'alors. Si vous avancez ensuite, l'historique se poursuit depuis cet endroit.

L'historique couvre quelques dizaines de millions d'instructions (la vue indique ce qui est gardé, en instructions et en
mégaoctets). **Modifier la mémoire ou les registres à la main le remet à zéro**, car ce qui n'a pas été enregistré ne peut pas
être rejoué ; il en va de même si l'application écrit dans les fichiers de la calculatrice.

## Registres de faute

Quand une application fait quelque chose d'interdit (lire une adresse qui n'existe pas, exécuter une instruction indéfinie…), elle
**s'arrête** et une fenêtre explique ce qui s'est passé : la cause en clair, l'instruction fautive, les registres, et un bouton qui
l'ouvre dans le débogueur.

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

Seuls les bits levés sont listés dans la vue Code et dans la fenêtre de faute, avec leur explication. Les registres sont en lecture
seule pour l'application ; la relance de l'application les remet à zéro. Un débogueur branché par GDB les lit comme ceux d'une
vraie carte, après un plantage.

## Utiliser GDB

Le serveur `nwasabi` peut offrir un **serveur GDB** : un débogueur (GDB, LLDB, VS Code…) s'y branche comme à une sonde de débogage.

```sh
nwasabi --nwa jeu.nwa --gdb 3333
```

Puis, dans GDB :

```
(gdb) target remote 127.0.0.1:3333
(gdb) break main
(gdb) continue
(gdb) x/8xw $sp
(gdb) stepi
(gdb) reverse-stepi
```

Ce qui est pris en charge : lecture et écriture des registres et de la mémoire, points d'arrêt, points d'observation (`watch`,
`rwatch`, `awatch`), pas à pas, reprise, interruption (Ctrl+C), et le **débogage en remontant le temps** (`reverse-stepi`,
`reverse-continue`, grâce à l'historique de la connexion).

À savoir :

- c'est une **émulation à part** de celle de la page : elle charge l'application `--nwa` telle qu'elle est sur le disque, et repart
  de zéro à chaque connexion ; un seul débogueur à la fois ;
- le serveur n'écoute que sur cette machine (`--host` change l'adresse) ;
- GDB ne lit pas les informations de débogage du code source (fonctions, variables, lignes) : il travaille sur les adresses et le
  désassemblage. Les noms de fonctions du fichier `.nwa` s'affichent dans l'onglet Débogage de l'émulateur.

## Les limites

- **Le système est simulé.** Les appels système (écran, clavier, temps, fichiers) sont des fonctions écrites pour nwasabi, pas le
  système Epsilon d'origine. Il n'y a ni périphériques STM32, ni amorçage, ni cache, ni temps d'accès à la flash.
- **Pas de gestionnaires d'exception.** Une faute arrête l'application ; seuls les registres de faute sont remplis.
- **La pile d'appels est estimée.** Le jeu d'instructions Thumb n'enchaîne pas les cadres de pile : nwasabi cherche dans la pile
  les adresses de retour plausibles (celles qui suivent un `bl` ou un `blx` du code de l'application). Une vieille adresse restée
  dans la pile peut s'y glisser.
- **Les noms de fonctions dépendent du fichier.** Ils viennent des symboles du `.nwa` ; un fichier sans symboles n'affiche que des
  adresses.
- **Pas d'informations de débogage du code source.** Il n'y a ni lignes, ni variables locales, ni types : le débogage se fait sur
  les instructions et la mémoire.
- **Les points ralentissent l'émulation.** Avec un point d'arrêt ou d'observation, l'émulateur avance instruction par instruction
  au lieu d'exécuter par grandes tranches.
- **L'historique est borné.** Il couvre quelques dizaines de millions d'instructions et repart de zéro quand on modifie à la main
  la mémoire ou les registres.
