# Débogage

Nwasabi émule le **processeur** (un Cortex-M7), pas toute la calculatrice : le système (Epsilon), les périphériques et les temps
d'accès de la vraie puce sont remplacés par des versions simplifiées (voir [Compatibilité](../reference/compatibilite.md)). On
débogue donc son code face à ce système simulé. Cette page décrit ce que le cœur d'émulation sait faire ; les mêmes fonctions sont
offertes par la page web, par le [protocole de session](../reference/protocole-session.md) et par [GDB](#utiliser-gdb).

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

Le jeu d'instructions Thumb n'enchaîne pas les cadres de pile : la pile d'appels est **estimée**. Nwasabi cherche dans la pile les
adresses de retour plausibles (celles qui suivent un `bl` ou un `blx` du code de l'application). Une vieille adresse restée dans la
pile peut s'y glisser.

## Noms de fonctions

Ils viennent des symboles du fichier `.nwa` (voir [Écrire une application](../guide/ecrire-une-application.md)) ; sans eux, seules
les adresses sont connues. Il n'y a ni lignes, ni variables locales, ni types : le débogage se fait sur les instructions et la
mémoire.

## Profil

Le profil relève régulièrement l'instruction en cours ; les fonctions et adresses qui reviennent le plus souvent sont celles où
l'application passe son temps. Une application qui attend (écran, clavier) exécute trop peu d'instructions pour être profilée.

## Journal des appels système

Chaque appel au système simulé est noté : numéro, registres r0 à r3 à l'entrée, valeur de retour, adresse de l'appel. Le journal
garde les 400 derniers appels. Voir [Appels système](../reference/appels-systeme.md).

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

Une adresse peut être un nombre, un registre (`$sp`, `$pc`…) ou un nom de fonction, avec `+N` ou `-N` (`main+8`). Les commandes qui
dépendent des informations de débogage du code source (`list`, `info line`, `info locals`…) n'existent pas.

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

## Utiliser GDB

Le serveur `nwasabi` peut offrir un **serveur GDB** : un débogueur (GDB, LLDB, VS Code…) s'y branche comme à une sonde de
débogage, et pilote **l'émulation de la page** : ce que fait GDB (pas, points d'arrêt, mémoire) se voit dans la page, et inversement.

```sh
nwasabi --nwa jeu.nwa --gdb 3333
```

Ce n'est **pas activé par défaut**, ni côté serveur ni côté page : la page doit accepter explicitement un débogueur (case
« Accepter un débogueur GDB » de la vue GDB, décochée à chaque ouverture). Tant qu'elle ne l'est pas, une connexion est refusée.
Le serveur sert d'intermédiaire : la page vient chercher les commandes du débogueur par une requête longue et y répond.

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
`reverse-continue`, grâce à l'historique, démarré à la connexion).

À savoir :

- un seul débogueur à la fois ;
- le serveur n'écoute que sur cette machine (`--host` change l'adresse) : n'ouvrez pas ce port à un réseau auquel vous ne faites pas
  confiance, car un débogueur peut modifier la mémoire et les registres ;
- pour un débogueur sans page ouverte, `--gdb-headless 3333` (avec `--nwa`) lance une émulation à part, qui charge l'application
  telle qu'elle est sur le disque et repart de zéro à chaque connexion ;
- le protocole est le Remote Serial Protocol de GDB, avec description XML des registres du Cortex-M, mode sans acquittement, `vCont`, points d'arrêt logiciels et matériels, `ReverseStep` et `ReverseContinue` ;
- GDB ne lit pas les informations de débogage du code source (fonctions, variables, lignes) : il travaille sur les adresses et le
  désassemblage.

## Les limites

- **Le système est simulé** : ni périphériques STM32, ni amorçage, ni cache, ni temps d'accès à la flash.
- **Pas de gestionnaires d'exception** : une faute arrête l'application ; seuls les registres de faute sont remplis.
- **La pile d'appels est estimée**, et les noms de fonctions dépendent des symboles du fichier.
- **Pas d'informations de débogage du code source.**
- **L'historique est borné** et repart de zéro quand on modifie à la main la mémoire ou les registres.
