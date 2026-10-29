---
title: "La bibliothèque JavaScript"
description: "Utiliser @nwasabi/js et @nwasabi/wasm pour intégrer l'émulateur dans une page web ou un programme Node.js."
---

# La bibliothèque JavaScript

`@nwasabi/js` embarque l'émulateur (compilé en WebAssembly) dans une page web ou un programme JavaScript : le lecteur d'écran
et de touches de ce site est construit avec.

## Les classes

| Classe | Rôle |
|---|---|
| `Nwasabi` | charge le module WebAssembly et ouvre des sessions |
| `Player` | un appareil avec son écran, son clavier et sa boucle d'animation |
| `InputMapper`, `KeyMap` | traduit les touches de l'ordinateur (et les gestes) en touches de la calculatrice |
| `KEYS` | la liste des touches |
| `GestureRecognizer` | reconnaît les gestes tactiles |

## Le lecteur (`Player`)

| Méthode | Effet |
|---|---|
| `Player.create` | crée un lecteur |
| `load`, `restart`, `changeModel` | charge une application, relance, change de modèle |
| `attach`, `draw`, `framebuffer`, `rgba`, `onFrame` | branche l'écran (canvas) et lit les images |
| `start`, `stop`, `stepFrame`, `bump` | contrôle l'exécution |
| `key` | appuie sur une touche |
| `setSpeed`, `setCustomMhz` | vitesse |
| `setKeyMap` | clavier de l'ordinateur |
| `time` | temps écoulé |
| `call` | envoie une commande du [protocole de session](./protocole-session.md) |
| `dump`, `restore` | exporte ou restaure l'état |
| `runWithoutData` | lance l'application sans ses données externes |

`call` donne accès à tout ce que fait le protocole (mémoire, registres, points d'arrêt, fichiers…).
