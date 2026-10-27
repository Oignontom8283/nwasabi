# Le serveur local `nwasabi`

`nwasabi` sert l'émulateur web depuis votre machine. L'émulation elle-même tourne dans le navigateur : le serveur donne la page, les
fichiers que vous lui indiquez, et sait quand ils changent sur le disque.

```sh
nwasabi --nwa jeu.nwa --nmk jeu.nmk --open
```

Il affiche l'adresse à ouvrir (`http://127.0.0.1:8080/` par défaut). Avec les sources du dépôt : `just serve --nwa jeu.nwa`.

## Options

| Option | Rôle |
|---|---|
| `--nwa FICHIER.nwa` | application à charger dans la page |
| `--data FICHIER` | données externes de l'application (ROM, script…) ; demande `--nwa` |
| `--nmk FICHIER.nmk` | mapping des touches ([format](../format/nmk.md)), vérifié au démarrage |
| `--nfs FICHIER.nfs` | fichiers de la calculatrice à charger ([format](../format/nfs.md)) |
| `--nds FICHIER.nds` | état à reprendre ([format](../format/nds.md)) ; il contient son application |
| `--model n0110\|n0115\|n0120` | modèle émulé (défaut `n0110`) |
| `--firmware 24\|26.4.0` | disposition du firmware (défaut `24`) ; voir [Modèles et mémoire](../reference/modeles-memoire.md) |
| `--port N` | port d'écoute (défaut 8080 ; 0 : un port libre) |
| `--host ADRESSE` | adresse d'écoute (défaut `127.0.0.1` : cette machine seulement) |
| `--open` | ouvre la page dans le navigateur |
| `--gdb PORT` | offre un serveur GDB sur l'émulation de la page ([plus bas](#brancher-gdb)) |
| `--gdb-headless PORT` | serveur GDB sur une émulation sans page |

Un fichier invalide arrête le serveur au démarrage, avec l'endroit de l'erreur.

## Recompiler sans recharger à la main

Le serveur relit ses fichiers à chaque requête et donne leur empreinte. La page la compare une fois par seconde : quand le `.nwa`
(ou un autre fichier donné) change sur le disque, la page propose de recharger (**Ctrl + R**). Un
réglage, désactivé par défaut, les recharge sans rien demander.

Cela donne la boucle de travail habituelle : on compile, on regarde, sans toucher à la page.

## Brancher GDB

Avec `--gdb PORT`, le serveur accepte un débogueur (GDB, LLDB, VS Code…) qui pilote **l'émulation de la page**, celle qu'on voit :

```sh
nwasabi --nwa jeu.nwa --gdb 3333
```

C'est une option, et rien n'est actif par défaut : sans `--gdb`, aucun port n'est ouvert pour un débogueur ; avec `--gdb`, le
serveur écoute mais **refuse les débogueurs** tant que vous n'avez pas coché « Accepter un débogueur GDB » dans la vue GDB
de la page (ce réglage n'est pas retenu d'une visite à l'autre). Puis :

```
(gdb) target remote 127.0.0.1:3333
```

Détails et commandes prises en charge : [Le débogage](../debug/index.md#utiliser-gdb).

Le serveur n'écoute que sur cette machine. N'exposez pas ce port sur un réseau : un débogueur connecté peut lire et écrire toute la
mémoire de l'émulation.

## Texte propre à votre installation

Si vous compilez le site vous-même, une fenêtre « i » (à côté de « Relancer ») peut porter votre texte, avec des variables
d'environnement lues à la compilation (`NWASABI_INFO`, `NWASABI_INFO_FILE`, `NWASABI_INFO_TITLE`) : voir [L'émulateur web](./emulateur-web.md).
