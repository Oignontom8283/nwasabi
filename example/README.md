# example

Petite page web qui montre comment intégrer nwasabi dans un site : `index.html` est un seul
fichier, sans outil de construction. Elle charge le module WebAssembly de nwasabi, lit un fichier
`.nwa` choisi par l'utilisateur et dessine son écran dans un canvas.

Pour l'essayer, depuis n'importe quel dossier du dépôt :

```
just example-serve
```

Cela construit le module, copie `nwasabi.js` et `nwasabi.wasm` à côté de la page, puis sert le dossier
sur http://localhost:8000/. (Un module WebAssembly ne se charge pas depuis `file://`, il faut un
serveur.) N'importe quel serveur de fichiers statiques convient : si vous préférez le vôtre, lancez
`just example` puis servez le dossier `example/` (attention, `-d example` de Python est relatif au
dossier courant : depuis `example/` lui-même, c'est simplement `python3 -m http.server 8000`).
Un serveur qui ne donne pas le type `application/wasm` à `nwasabi.wasm` fonctionne aussi :
`nwasabi.js` recharge alors le module sans compilation en continu.

Pour l'intégrer ailleurs, il suffit de copier `index.html`, `nwasabi.js` et `nwasabi.wasm` sur
votre serveur. Toutes les commandes disponibles (fichiers, mémoire, réglages…) sont celles de la
commande `help` du protocole de session : voir `src/session/mod.rs`.

Ce dossier ne contient que la démonstration, aucun test.
