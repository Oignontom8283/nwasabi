# Fichiers `.nrp` — Nwasabi RePlay

Un fichier **NRP** est l'**enregistrement d'une exécution** : les touches, le temps qui passe, les exécutions, avec ce qu'on a observé à
chaque étape (le motif d'arrêt, le nombre d'instructions, le CRC de l'écran). Rejoué sur la même application, il doit redonner
exactement la même chose : c'est un test de non-régression qui ne demande aucune image de référence. C'est du JSON, extension
`.nrp`.

L'émulation est déterministe (voir [Compatibilité](../reference/compatibilite.md)) : les mêmes requêtes sur un appareil neuf donnent les
mêmes états, d'où la valeur d'un enregistrement.

## Exemple

```json
{
  "nrp": 1,
  "created_by": "nwasabi 0.1.0",
  "device": { "model": "n0110", "firmware": "24" },
  "app": { "name": "jeu.nwa", "crc32": "7400335b" },
  "entries": [
    { "request": { "cmd": "key", "key": "ok", "action": "press" } },
    { "request": { "cmd": "run", "frames": 30 }, "expect": { "stop": "frame", "instructions": 161 } },
    { "request": { "cmd": "screen", "rgba": false, "rgb565": false }, "expect": { "crc32": "50eba01c" } }
  ]
}
```

## Champs

| Champ | Rôle |
|---|---|
| `nrp` | **Obligatoire.** Version du format : `1`. |
| `device` | Le modèle et le firmware de l'appareil, et le temps (`clock`, `speed`, `mhz`) quand ils ne sont pas ceux par défaut. |
| `app` | L'application attendue : `name`, `crc32` (de son `.nwa`), et `data_crc32` si elle avait des données externes, `flash` si elle avait des [données auxiliaires en flash](../reference/donnees-flash.md). Le `.nwa` lui-même n'est pas dans le fichier. |
| `entries` | **Obligatoire.** Les requêtes, dans l'ordre. |

Chaque entrée a une `request` (une requête du [protocole de session](../reference/protocole-session.md)) et, le plus souvent, un `expect`
avec ce qui a été observé :

| Observation | Contenu de `expect` |
|---|---|
| `run`, `step`, `step_source`… | `stop` (le motif d'arrêt) et `instructions` |
| `screen` | `crc32` de l'écran |
| `stdout` | `text_crc32` du texte écrit par l'application |

Seules les requêtes qui changent ce que l'application voit ou fait sont enregistrées (touches, temps, exécution, écriture de
mémoire ou de fichiers, points d'arrêt…), avec l'écran et la sortie texte comme points de contrôle. Lire la mémoire ou les registres
n'est pas enregistré.

## Enregistrer et rejouer

| Où | Comment |
|---|---|
| `nwasabi-cli run` | `run jeu.nwa --press ok --frames 30 --record jeu.nrp` enregistre cette exécution |
| `nwasabi-cli session` | `session --record jeu.nrp` enregistre les requêtes que l'on envoie, jusqu'à la fin de l'entrée |
| `nwasabi-cli replay` | `replay jeu.nrp jeu.nwa` rejoue et compare ; `--data`, `--flash` donnent ce qui accompagnait l'application, `--screenshot` garde l'écran final |

Le code de sortie de `replay` est 0 si tout concorde, **4** si une observation diffère (chaque différence est affichée avec son rang,
ce qui avait été enregistré et ce qui a été rejoué), 1 si le fichier est illisible ou si le chargement échoue. Une application, des
données ou des données auxiliaires qui ne sont pas ceux de l'enregistrement sont signalés par un avertissement.
