# Fichiers `.nfd` — Nwasabi Flash Data

Un fichier **NFD** contient des **octets à placer à des adresses de la flash externe**, à côté de l'application. Certaines applications
ne tiennent pas dans leur fichier `.nwa` : elles lisent, à une adresse fixe, un gros programme ou des données que l'on flashe à part avec
un outil USB (c'est le cas de [KhiCAS](../guide/khicas.md)). Un `.nfd` rassemble ces morceaux, chacun avec son adresse. C'est du JSON,
extension `.nfd`, avec les contenus en base64.

## Exemple

```json
{
  "nfd": 1,
  "active_slot": "a",
  "segments": [
    { "name": "khia", "address": "0x90260000", "content": "AAAA…" },
    { "name": "khi120ab.tar", "address": "0x90400000", "content": "AAAA…" }
  ]
}
```

## Champs

| Champ | Rôle |
|---|---|
| `nfd` | **Obligatoire.** Version du format : `1`. |
| `active_slot` | Facultatif, `"a"` ou `"b"` : le slot du firmware que l'appareil doit exécuter. À utiliser quand les segments occupent l'autre slot (voir plus bas). |
| `segments` | **Obligatoire.** La liste des morceaux, au plus 64. |

Chaque segment :

| Champ | Rôle |
|---|---|
| `address` | Adresse du premier octet, dans la flash externe (`0x90000000` à `0x907fffff`). Un nombre, ou un texte `"0x90400000"`. |
| `content` | Les octets, en base64. Au moins un. |
| `name` | Facultatif : nom indicatif (en général celui du fichier d'origine). |

Un segment doit tenir entièrement dans la flash externe, et deux segments ne peuvent pas se recouvrir.

## Le firmware et les slots

La flash externe de 8 Mio est coupée en deux **slots** de 4 Mio : l'un contient le firmware (Epsilon) et les applications installées après
lui, l'autre est libre ou contient un autre firmware. Un appareil réel peut exécuter l'un ou l'autre : sur les appareils relevés, c'est
parfois le slot A (`0x90000000`), parfois le slot B (`0x90400000`).

Un segment ne peut **pas recouvrir** le firmware ni les applications installées : il est refusé avec l'adresse en cause. Un programme
placé à `0x90400000` demande donc que le firmware soit dans le slot A : c'est le rôle de `active_slot`, que le chargement applique
avant de construire l'appareil. Les adresses de la zone des applications dépendent du modèle et du slot (voir
[Modèles et mémoire](../reference/modeles-memoire.md)).

## Ce que fait l'application des segments

Les segments sont de la flash comme une autre : lisible et **exécutable** à leur adresse. Une application peut aussi effacer et
reprogrammer la flash qui n'appartient ni au firmware ni aux applications (appels système 30 et 32 : voir
[Appels système](../reference/appels-systeme.md)), par exemple pour garder des scripts. Ces écritures ne sont pas retenues dans le `.nfd` :
elles font partie de l'état de l'appareil, qu'un fichier [`.nds`](./nds.md) enregistre.

## Fabriquer et charger un `.nfd`

| Où | Comment |
|---|---|
| `nwasabi-cli` | `flash pack -o sortie.nfd [--active-slot a] 0x90400000=programme.tar …` rassemble des fichiers ; `flash list fichier.nfd` les décrit ; `run app.nwa --flash fichier.nfd` charge avant d'exécuter. |
| Émulateur web | `…/emulator/?nwa=…&flash=https://exemple.org/fichier.nfd` (CORS comme pour `?nwa=`) ; ou glisser le fichier sur la page : l'application est relancée avec ses octets. |
| Serveur `nwasabi` | `nwasabi --nwa app.nwa --flash fichier.nfd` : le fichier est vérifié au démarrage, relu quand il change. |
| Session JSON | `{"cmd":"load","nwa":…,"nfd":…}` charge l'application avec ses octets ; `{"cmd":"flash_load","nfd":…}` en ajoute à l'appareil ; `{"cmd":"flash_export"}` rend ceux qui sont placés. |
| `.nds` | la partie `app` d'un [état enregistré](./nds.md) garde les segments (`app.flash`). |

## Règles de chargement

- Le document est **entièrement vérifié avant** d'écrire quoi que ce soit (adresses, recouvrements) ; une erreur est signalée avec
  son emplacement (`segments[1] : …`) et rien n'est modifié.
- Les segments s'ajoutent à ceux déjà placés ; un segment à la même adresse qu'un autre le remplace.
- Une application installée ensuite ne peut pas recouvrir un segment déjà placé.
