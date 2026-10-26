# Fichiers de mapping `.nmk`

Un fichier **NMK** (*Nwasabi Mapping Key*) décrit comment les entrées de l'ordinateur ou de l'écran
tactile deviennent des touches de la calculatrice : `W`, `A`, `S`, `D` pour les flèches d'un jeu, un
balayage vers la droite pour la flèche droite, un appui long ou deux doigts pour une autre touche…

C'est du JSON, extension `.nmk`. Il est chargé par l'émulateur web (`?nmk=…`), par le serveur
`nwasabi` (`--nmk`) et par l'outil `nwasabi-cli` (`--nmk`), et il est utilisable depuis `@nwasabi/js`.

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

- `gesture` : `tap`, `double_tap`, `long_press`, `swipe_left`, `swipe_right`, `swipe_up`, `swipe_down` ;
- `fingers` : nombre de doigts, de 1 à 3 (défaut 1) ;
- `key` : comme pour le clavier (nom, liste, ou `null` pour retirer une règle par défaut) ;
- `while_held` : seulement pour `long_press` ; la touche reste enfoncée tant que le doigt reste posé.

Par défaut, un balayage à un doigt vers la gauche, la droite, le haut ou le bas appuie sur la flèche
correspondante. Une règle du fichier remplace la règle par défaut de même `gesture` et même `fingers`.
