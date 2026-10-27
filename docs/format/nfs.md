# Fichiers `.nfs` — Nwasabi File System

Un fichier **NFS** contient les **fichiers de la calculatrice** (scripts Python, réglages…), ceux que
l'on voit dans l'application « Python » et que les applications externes lisent et écrivent. C'est du
JSON, extension `.nfs`. Les contenus binaires sont encodés en base64 : le fichier ne contient que du texte.

Il décrit des fichiers, pas l'image brute du tampon de la calculatrice : un `.nfs` se recharge donc sur
n'importe quel modèle et n'importe quelle disposition de firmware.

## Exemple

```json
{
  "nfs": 1,
  "capacity": 43008,
  "files": [
    { "name": "main.py", "content": "cHJpbnQoMSk=" },
    { "name": "scores.py", "content": "eCA9IDQy" }
  ]
}
```

## Champs

| Champ | Rôle |
|---|---|
| `nfs` | **Obligatoire.** Version du format : `1`. |
| `capacity` | Taille du tampon de l'appareil d'où vient l'export, en octets. Informatif : à la lecture, c'est la place réelle de l'appareil qui compte. |
| `files` | **Obligatoire.** La liste des fichiers, dans l'ordre du tampon. |

Chaque fichier :

| Champ | Rôle |
|---|---|
| `name` | Nom de la forme `nom.extension`, sans caractère nul. Un nom ne peut apparaître qu'une fois. |
| `content` | Le corps du fichier (octets, encodés en base64 dans le JSON). Au plus 65 535 octets. |

Pour un script Python (`.py`), le corps commence par un octet « importé automatiquement » (0 ou 1)
suivi du texte : c'est le corps tel que la calculatrice le stocke, sans transformation.

## Charger un `.nfs`

| Où | Comment |
|---|---|
| Émulateur web | `…/emulator/?nfs=https://exemple.org/fichiers.nfs` (CORS comme pour `?nwa=`) ; onglet « Fichiers » : importer, exporter. |
| Serveur `nwasabi` | `nwasabi --nwa jeu.nwa --nfs fichiers.nfs` : le fichier est vérifié au démarrage. |
| `nwasabi-cli` | `run jeu.nwa --nfs fichiers.nfs` charge avant d'exécuter, `--save-nfs sortie.nfs` enregistre après. `fs --image tampon.bin export sortie.nfs` et `import entree.nfs [--keep]` travaillent sur une image brute. |
| Session JSON | `{"cmd":"fs_export"}` rend `{"nfs": …}` ; `{"cmd":"fs_import","nfs":…,"replace":true}` charge. |

## Règles de chargement

- Le document est **entièrement vérifié avant** la première écriture : une erreur est signalée avec
  son emplacement (`files[1] : nom invalide…`) et rien n'est modifié.
- Par défaut (`replace`), les fichiers existants sont **tous supprimés** avant le chargement ; avec
  « garder les fichiers existants », seuls ceux du même nom sont remplacés.
- Si la place manque en cours de chargement, **le tampon est remis dans son état d'avant**.
