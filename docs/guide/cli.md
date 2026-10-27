# La ligne de commande `nwasabi-cli`

`nwasabi-cli` exécute l'émulateur **sans navigateur** : le cœur est compilé dans le binaire, les résultats sont déterministes (une
même exécution donne toujours les mêmes octets avec `--json`). Il sert aux scripts, aux tests automatiques et à l'intégration continue.

```sh
just build-cli
just cli run jeu.nwa --frames 120 --screenshot ecran.png
```

**Codes de sortie** : 0 succès, 1 erreur, 2 mauvais usage, 3 l'application s'est arrêtée sur une faute.

Les options de l'appareil (`--model`, `--firmware`, `--data`, `--mhz`) valent pour `run`, `inspect` et `link`.

## `run` : exécuter une application

```sh
nwasabi-cli run jeu.nwa --frames 300 --press right --screenshot final.png --json
```

Exécute l'application et résume l'arrêt, l'écran et la sortie standard.

| Option | Rôle |
|---|---|
| `--instructions N` | nombre maximal d'instructions (défaut 10 millions) |
| `--frames N` | s'arrêter après N images |
| `--press TOUCHE` | touche enfoncée avant de lancer (répétable) ; `nwasabi-cli keys` les liste |
| `--nmk FICHIER` | mapping pour traduire les touches de `--press` (accepte alors `KeyW`, `key:e`…) |
| `--nds`, `--nfs` | reprend un état, charge des fichiers de la calculatrice |
| `--save-nds`, `--save-nfs` | enregistre l'état ou les fichiers après l'exécution |
| `--screenshot FICHIER.png` | enregistre l'écran final |
| `--json` | sortie en JSON |
| `--mhz N` | fréquence du processeur, qui règle le temps virtuel |

## Les autres commandes

| Commande | Rôle |
|---|---|
| `inspect [APP]` | affiche en JSON les structures de l'OS lues en mémoire (en-têtes, applications, stockage) |
| `link APP` | résultat de l'édition de liens ; `--map` ajoute les sections et les symboles |
| `fs --image FICHIER …` | manipule une image du système de fichiers : `list`, `stats`, `read`, `write`, `remove`, `rename`, `export`, `import` |
| `survey DOSSIER…` | lance chaque application d'un dossier sans touche et résume ce qui s'est passé |
| `session` | une requête JSON par ligne en entrée, une réponse par ligne en sortie (voir ci-dessous) |
| `keys` | liste les noms des touches |

## `session` : piloter l'émulateur depuis un autre programme

```sh
nwasabi-cli session
{"cmd":"load","nwa":"<base64 du .nwa>"}
{"cmd":"run","frames":60}
{"cmd":"screen"}
```

Chaque ligne d'entrée est une commande, chaque ligne de sortie sa réponse (`{"ok":true,…}` ou `{"ok":false,"error":"…"}`). C'est le même
langage que celui du module WebAssembly et du serveur GDB. Toutes les commandes : [Le protocole de session](../reference/protocole-session.md).
