---
title: "Format .nfd (données en flash)"
description: "Spécification du format .nfd : morceaux d'octets placés à des adresses de la flash externe, à côté d'une application."
---

# Fichiers `.nfd` — Nwasabi Flash Data

Un fichier **NFD** contient les [**données auxiliaires en flash**](../donnees-flash.md) d'une application : des **octets à placer à des
adresses de la flash externe**, à côté de l'application (c'est le cas de [KhiCAS](../../guide/khicas.md)). C'est du JSON, extension `.nfd`, avec
les contenus en base64.

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

## Slots et recouvrements

Un segment ne peut **pas recouvrir** le firmware ni les applications installées : il est refusé avec l'adresse en cause. Pour mettre
un programme à `0x90400000`, il faut donc que le firmware soit dans le slot A : c'est le rôle de `active_slot`, que le chargement
applique avant de construire l'appareil. Voir [Données auxiliaires en flash](../donnees-flash.md#ou-placer-les-donnees) et
[Modèles et mémoire](../modeles-memoire.md#les-deux-slots-de-la-flash).

## Fabriquer et charger un `.nfd`

| Où | Comment |
|---|---|
| `nwasabi-cli` | `flash pack -o sortie.nfd [--active-slot a] 0x90400000=programme.tar …` rassemble des fichiers ; `flash list fichier.nfd` les décrit ; `run app.nwa --flash fichier.nfd` charge avant d'exécuter. |
| Émulateur web | `…/emulator/?nwa=…&flash=https://exemple.org/fichier.nfd` (CORS comme pour `?nwa=`) ; ou glisser le fichier sur la page : l'application est relancée avec ses données. |
| Serveur `nwasabi` | `nwasabi --nwa app.nwa --flash fichier.nfd` : le fichier est vérifié au démarrage, relu quand il change. |
| Session JSON | `{"cmd":"load","nwa":…,"nfd":…}` charge l'application avec ses données ; `{"cmd":"flash_load","nfd":…}` en ajoute à l'appareil ; `{"cmd":"flash_export"}` rend ceux qui sont placés. |
| `.nds` | la partie `app` d'un [état enregistré](./nds.md) garde les segments (`app.flash`). |

## Règles de chargement

- Le document est **entièrement vérifié avant** d'écrire quoi que ce soit (adresses, recouvrements) ; une erreur est signalée avec
  son emplacement (`segments[1] : …`) et rien n'est modifié.
- Les segments s'ajoutent à ceux déjà placés ; un segment à la même adresse qu'un autre le remplace.
- Une application installée ensuite ne peut pas recouvrir un segment déjà placé.
