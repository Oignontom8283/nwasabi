---
title: "Format .nmk (mapping des touches)"
description: "Spécification du format .nmk : liaisons du clavier de l'ordinateur, accords de touches et gestes tactiles vers les touches de la calculatrice."
---

# Fichiers de mapping `.nmk`

Un fichier **NMK** (*Nwasabi Mapping Key*) décrit comment les entrées de l'ordinateur ou de l'écran
tactile deviennent des touches de la calculatrice : `W`, `A`, `S`, `D` pour les flèches d'un jeu, un
balayage vers la droite pour la flèche droite, un appui long ou deux doigts pour une autre touche…

C'est du JSON, extension `.nmk`. Il est chargé par l'émulateur web (`?nmk=…`), par le serveur
`nwasabi` (`--nmk`) et par l'outil `nwasabi-cli` (`--nmk`), et il est utilisable depuis `@nwasabi/js`.

## Charger un mapping

| Où | Comment |
|---|---|
| Émulateur web | `…/emulator/?nmk=https://exemple.org/jeu.nmk` : l'adresse peut être relative ou absolue ; un autre site doit autoriser la lecture (CORS), comme pour `?nwa=`. L'onglet « Touches » montre le mapping en vigueur et permet d'ouvrir un fichier ou de revenir au défaut. |
| Serveur `nwasabi` | `nwasabi --nwa jeu.nwa --nmk jeu.nmk` : le fichier est vérifié au démarrage (le serveur refuse de démarrer s'il est invalide) puis donné à la page. |
| `nwasabi-cli` | `nwasabi-cli run jeu.nwa --nmk jeu.nmk --press KeyW` : `--press` accepte alors une touche de l'ordinateur (`KeyW`, `key:e`). `nwasabi-cli keys --nmk jeu.nmk` affiche le mapping complet, `--json` pour une sortie lisible par un programme. |
| `@nwasabi/js` | `player.setKeyMap(KeyMap.parse(texte))` ; voir le README du paquet. |

Un fichier invalide est refusé avec l'endroit de l'erreur (`touch.rules[0].while_held : réservé à long_press`).

## Exemple

```json
{
  "nmk": 1,
  "name": "Jeu de plateforme",
  "keyboard": {
    "KeyW": "up",
    "KeyA": "left",
    "KeyS": "down",
    "KeyD": "right",
    "Space": "ok",
    "KeyQ": ["shift", "alpha"],
    "key:e": null
  },
  "touch": {
    "long_press_ms": 400,
    "rules": [
      { "gesture": "long_press", "key": "ok", "while_held": true },
      { "gesture": "tap", "fingers": 2, "key": "back" }
    ]
  }
}
```

## Champs

| Champ | Rôle |
|---|---|
| `nmk` | **Obligatoire.** Version du format : `1`. |
| `name` | Nom du mapping (affiché dans l'interface). |
| `extends` | `"default"` (défaut) : le fichier complète les liaisons par défaut ; `"none"` : il part de zéro. |
| `keyboard` | Liaisons du clavier de l'ordinateur. |
| `touch` | Réglages et règles des gestes tactiles. |

Les champs inconnus sont refusés : une faute de frappe se voit tout de suite.

## `keyboard`

Chaque clé désigne une touche de l'ordinateur ; chaque valeur est :

- le nom d'une touche de la calculatrice (`"up"`, `"ok"`, `"left_parenthesis"`… la liste donnée par
  `nwasabi-cli keys`) ;
- une liste de noms : les touches sont enfoncées **ensemble** (`["shift", "alpha"]`) ;
- `null` : retire la liaison par défaut de cette touche.

Une clé s'écrit de deux façons :

| Forme | Exemple | Signification |
|---|---|---|
| `Code` | `"KeyW"`, `"ArrowUp"`, `"Digit1"`, `"Space"` | position physique de la touche (`KeyboardEvent.code`) : `KeyW` est au même endroit sur un clavier AZERTY ou QWERTY. À préférer pour les jeux. |
| `key:Valeur` | `"key:e"`, `"key:+"`, `"key:Enter"` | caractère produit (`KeyboardEvent.key`), selon la disposition du clavier. |

Si une touche correspond à la fois à une liaison par code et à une liaison par valeur, le **code** gagne.
Plusieurs touches de l'ordinateur peuvent viser la même touche de la calculatrice : elle reste enfoncée
tant que l'une d'elles est maintenue.

## `touch`

Réglages (tous facultatifs) :

| Champ | Défaut | Rôle |
|---|---|---|
| `swipe_distance` | `40` | distance minimale d'un balayage, en pixels |
| `long_press_ms` | `500` | durée d'un appui long |
| `double_tap_ms` | `300` | délai maximal entre deux appuis d'un double appui |
| `press_ms` | `80` | durée pendant laquelle la touche reste enfoncée pour un geste ponctuel |

`rules` est une liste de règles `{ "gesture", "fingers", "key", "while_held" }` :

- `gesture` : `tap`, `double_tap`, `long_press`, `swipe_left`, `swipe_right`, `swipe_up`, `swipe_down`, `u_turn` ;
- `fingers` : nombre de doigts, de 1 à 3 (défaut 1) ;
- `key` : comme pour le clavier (nom, liste, ou `null` pour retirer une règle par défaut) ;
- `while_held` : seulement pour `long_press` ; la touche reste enfoncée tant que le doigt reste posé.

Le geste `u_turn` (demi-tour) est une courbe du doigt qui tourne de la moitié d'un cercle, dans un sens ou
dans l'autre, comme un « U » ou un « C » tracé d'un trait. Il faut que le doigt tourne d'au moins 150 degrés
et d'au plus 280, toujours du même côté, sur une longueur d'au moins une fois et demie `swipe_distance`.
Un quart de cercle, une ligne droite, un zigzag ou un cercle entier n'en sont pas.

Par défaut, à un doigt :

| Geste | Touche |
|---|---|
| appui du doigt | `ok` |
| demi-tour | `back` |
| balayage à gauche, à droite, en haut, en bas | la flèche correspondante |

Un clic de souris n'est jamais un appui, un double appui ni un appui long : on clique sur l'écran pour lui donner le focus. Les gestes tracés (balayage, demi-tour) fonctionnent aussi à la souris.

Une règle du fichier remplace la règle par défaut de même `gesture` et même `fingers` ; `"key": null` la retire.
