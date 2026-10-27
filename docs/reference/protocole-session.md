# Le protocole de session

La ligne de commande (`nwasabi-cli session`), le serveur, la bibliothèque JavaScript et le serveur GDB parlent tous le même langage :
des objets JSON qui pilotent un appareil émulé. Pour automatiser des tests, c'est la porte d'entrée la plus complète.

Une requête est un objet `{"cmd": "…", …}` (avec un `"id"` facultatif, renvoyé tel quel). La réponse est `{"ok": true, …}` ou
`{"ok": false, "error": "…"}`. La session ne touche ni au disque ni à l'horloge : les fichiers sont échangés en base64, la mémoire
en hexadécimal, et le temps n'avance que par `advance` (ou `run`).

```json
{"cmd": "new", "model": "n0110"}
{"cmd": "load", "nwa": "<base64>"}
{"cmd": "key", "key": "OK", "action": "tap"}
{"cmd": "advance", "ms": 500}
{"cmd": "screen", "rgba": true}
```

La commande `help` renvoie la liste à jour avec les paramètres de chacune ; ce qui suit en donne le panorama.

## Créer et charger

| Commande | Effet |
|---|---|
| `new`, `load` | crée un appareil, ou en charge un avec une application (`nwa` et `data` en base64). Options : `model`, `firmware`, `clock` (`virtual` ou `host`), `speed`, `mhz` |
| `launch` | lance l'application |
| `dump`, `restore` | exporte ou restaure l'état (`parts` : `app`, `memory`, `fs`, `state`) |
| `settings_get`, `settings_set` | lit ou modifie les réglages (`settings_get` donne les noms) |

## Faire tourner

| Commande | Effet |
|---|---|
| `run` | exécute jusqu'à une limite d'instructions ou un arrêt (point, faute, fin) |
| `key` | `press`, `release` ou `tap` d'une touche |
| `advance` | fait passer du temps (`us` ou `ms`) |
| `speed` | `model`, `custom` (avec `mhz`) ou `max` |
| `time` | temps écoulé |
| `screen` | image de l'écran (`rgba` ou `rgb565`) ; le CRC-32 est toujours donné |
| `stdout` | texte écrit par l'application (`clear` pour le vider) |

## Observer

| Commande | Effet |
|---|---|
| `cpu`, `cpu_set` | registres du processeur, lire ou modifier |
| `inspect`, `regions`, `usage` | structures du système, zones mémoire, occupation |
| `memory_read`, `memory_write` | lit ou écrit la mémoire (hexadécimal, 1 Mio au plus par lecture) |
| `changes` | octets modifiés depuis la dernière demande |
| `symbol`, `symbols`, `backtrace`, `disasm` | noms, pile d'appels, désassemblage |
| `profile` | `start`, `stop`, `get` |
| `svc_stats`, `svc_log` | statistiques et journal des appels système (`start`, `stop`, `clear`, `get`, `since`) |

## Déboguer

| Commande | Effet |
|---|---|
| `break` (avec une condition `{register, op, value}` facultative), `unbreak`, `breaks` | points d'arrêt |
| `watch`, `unwatch`, `watch_list`, `watch_hits` | points d'observation |
| `step` | une instruction |
| `history` (`start`, `stop`, `status`), `step_back`, `reverse_continue`, `history_goto` | [remonter le temps](../debug/index.md#remonter-le-temps) |

## Fichiers de la calculatrice

`fs_list`, `fs_stats`, `fs_read`, `fs_write` (un script Python avec `python: true` et `text`, ou un contenu `content` en base64),
`fs_remove`, `fs_rename`, `fs_export`, `fs_import`.
