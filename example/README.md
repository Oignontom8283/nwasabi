# example

Petite page web qui montre comment intégrer nwasabi dans un site : `index.html` est un seul
fichier, sans outil de construction. Elle charge le module WebAssembly de nwasabi, lit un fichier
`.nwa` choisi par l'utilisateur et dessine son écran dans un canvas.

Pour l'essayer :

```
just example          # construit le module et copie nwasabi.js et nwasabi.wasm à côté de la page
python3 -m http.server -d example 8000
```

puis ouvrir http://localhost:8000/. (Un module WebAssembly ne se charge pas depuis `file://`.)

Pour l'intégrer ailleurs, il suffit de copier `index.html`, `nwasabi.js` et `nwasabi.wasm` sur
votre serveur. Toutes les commandes disponibles (fichiers, mémoire, réglages…) sont celles de la
commande `help` du protocole de session : voir `src/session/mod.rs`.

Ce dossier ne contient que la démonstration, aucun test.
