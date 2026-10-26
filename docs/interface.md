# L'interface de l'émulateur web

L'émulateur est dans le navigateur, à l'adresse `/emulator/` du site, ou servi par `nwasabi` (le serveur local).

## Charger des fichiers

Glissez un fichier sur la page, ou utilisez les boutons de la barre du haut. L'extension décide de ce qui est chargé :

| Extension | Contenu | Format |
|---|---|---|
| `.nwa` | l'application | — |
| `.nmk` | les touches (clavier et gestes) | [`nmk`](./format/nmk.md) |
| `.nfs` | les fichiers de la calculatrice | [`nfs`](./format/nfs.md) |
| `.nds` | un état d'émulation à reprendre | [`nds`](./format/nds.md) |

## Paramètres de l'adresse

Tout peut être donné dans l'adresse. Les adresses de fichiers sont absolues ou relatives à la page ; un autre site
doit autoriser leur lecture (CORS).

| Paramètre | Rôle |
|---|---|
| `nwa=` | application à lancer |
| `data=` | données externes de l'application |
| `nmk=` | mapping des touches |
| `nfs=` | fichiers de la calculatrice |
| `nds=` | état à reprendre (il contient son application : il remplace `nwa=`) |
| `lang=` | langue de l'interface : `fr` ou `en` (sinon le dernier choix, puis la langue du navigateur) |

Exemple : `…/emulator/?nwa=https://exemple.org/jeu.nwa&nmk=https://exemple.org/jeu.nmk&lang=fr`.

Avec le serveur : `nwasabi --nwa jeu.nwa --nmk jeu.nmk --nfs fichiers.nfs --nds etat.nds`.

## L'interface

- **La calculatrice** garde les proportions et la disposition d'une vraie. Un clic (ou un doigt) sur une touche l'enfonce ;
  les touches liées à autre chose que le clavier par défaut sont surlignées dans l'onglet « Touches ».
- **Plein écran** (bouton de la petite barre) : seul l'écran reste. **Espace** quitte le plein écran (il n'est alors jamais
  donné à l'application), de même qu'un bouton qui apparaît en haut au milieu quand la souris y passe, et Échap.
- **Petite barre d'infos** à droite : état, images, temps, vitesse, capture. L'engrenage ouvre le grand panneau ; la flèche
  la masque (un petit onglet la ramène).
- **Grand panneau** : se redimensionne à la souris (ou aux flèches quand sa poignée a le focus). Il décale la calculatrice,
  qui reste centrée dans l'espace restant. Onglets :
  - **Fichiers** : explorateur des fichiers de la calculatrice (clic droit : renommer, télécharger, supprimer), éditeur de
    scripts Python, import et export `.nfs` ;
  - **Mémoire** : enregistrement de l'état (`.nds`, avec cases à cocher), utilisation du tas et de la pile, carte de la
    mémoire (contenu, réel ou simulé, repères qui renvoient aux structures) et éditeur hexadécimal ;
  - **Structures** : les structures du firmware relues depuis la mémoire ;
  - **Touches** : le mapping, modifiable depuis la calculatrice ou les tableaux, avec import et export `.nmk` ;
  - **Réglages** : vitesse du processeur, identité et matériel de l'appareil ;
  - **Trace** : registres du processeur, appels système, sortie standard.
- Enregistrer l'état, exporter les fichiers et modifier la mémoire exigent la **pause**.

## Accessibilité

Toutes les actions se font au clavier (Tab, flèches dans les listes et les onglets, Entrée ou Espace sur une touche de la
calculatrice qui a le focus), les zones sont étiquetées pour les lecteurs d'écran, les couleurs suivent le thème clair ou
sombre du système (réglable en haut à droite), et les animations respectent « réduire les animations ».
