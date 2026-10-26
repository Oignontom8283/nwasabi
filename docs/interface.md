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

### Fichiers du serveur qui changent

Le serveur relit ses fichiers (`--nwa`, `--data`, `--nmk`, `--nfs`, `--nds`) à chaque requête et donne leur empreinte sur
`/__nwasabi/version`. La page la compare une fois par seconde : quand un fichier change sur le disque, un bouton
« Recharger » apparaît dans la barre du haut (raccourci **Ctrl + R**). Le réglage « Recharger automatiquement les fichiers du
serveur » (onglet Réglages, désactivé par défaut) les recharge sans rien demander. Un fichier devenu invalide n'arrête pas le
serveur : la page affiche l'erreur.

## Texte propre à votre installation

Si vous compilez le site vous-même, vous pouvez y ajouter une fenêtre « i » (à côté de « Relancer ») sans toucher aux
fichiers du dépôt, avec des variables d'environnement lues à la compilation :

| Variable | Rôle |
|---|---|
| `NWASABI_INFO` | le texte, en Markdown simple (gras, italique, liens, listes ; pas de tableaux ni de HTML) |
| `NWASABI_INFO_FILE` | ou le chemin d'un fichier Markdown |
| `NWASABI_INFO_TITLE` | le titre de la fenêtre (par défaut « Informations ») |

Exemple : `NWASABI_INFO_FILE=infos.md cargo build --release --features serve`. Sans texte, le bouton n'apparaît pas.

## L'interface

- **La calculatrice** garde les proportions et la disposition d'une vraie. Un clic (ou un doigt) sur une touche l'enfonce ;
  dans l'onglet « Touches », les touches qui ont une liaison sont surlignées.
- **Barre du haut** : menu à trois traits (charger une application, des données, un mapping, des fichiers ou un état),
  modèle et firmware, pause et relance, nom du fichier au centre (avec un bouton « Charger un .nwa » à côté tant qu'aucune
  application n'est chargée), puis agrandissement de l'écran, langue, thème, fenêtre « À propos » (licence, indépendance
  de l'outil vis-à-vis de ce qu'on y exécute) et lien vers le dépôt.
- **Écran vide** : glisser un fichier au-dessus de la zone de dépôt la met en évidence ; un clic dessus ouvre le sélecteur de fichier.
- **Agrandir** (bouton de la barre du haut ou de la petite barre) : seul l'écran reste, le plus grand possible dans la page,
  avec un cadre ; il s'adapte au panneau ouvert à côté. Ce n'est pas le plein écran.
- **Plein écran** (bouton de la petite barre) : seul l'écran reste. **Espace** quitte le plein écran (il n'est alors jamais
  donné à l'application), de même qu'un bouton qui apparaît en haut au milieu quand la souris y passe, et Échap.
- **Petite barre d'infos** à droite : état, images, temps, vitesse, capture. Le bouton « Outils » ouvre le grand panneau ;
  la flèche la masque (un petit onglet la ramène).
- **Grand panneau** : se redimensionne à la souris (ou aux flèches quand sa poignée a le focus). Il décale la calculatrice,
  qui reste centrée dans l'espace restant. Onglets :
  - **Fichiers** : explorateur des fichiers de la calculatrice (création et renommage dans la liste même ; clic droit :
    renommer, copier la taille ou le nom, télécharger, supprimer ; Ctrl ou Maj + clic, ou un rectangle dans l'espace libre,
    pour en choisir plusieurs ; Suppr supprime après confirmation, Maj + Suppr sans), éditeur de scripts Python, éditeur
    hexadécimal pour les fichiers dont le format n'est pas reconnu, « Tout effacer », import et export `.nfs` ;
  - **Mémoire** : enregistrement de l'état (`.nds`, avec cases à cocher), utilisation du tas et de la pile, carte de la
    mémoire (contenu, réel ou simulé, infobulle qui suit la souris, repères qui renvoient aux structures) et éditeur hexadécimal ;
  - **Structures** : les structures du firmware relues depuis la mémoire ;
  - **Touches** : le mapping, modifiable depuis la calculatrice ou les tableaux, avec import et export `.nmk`, et les seuils
    de reconnaissance des gestes ;
  - **Réglages** : conserver les fichiers de la calculatrice dans le navigateur (voir ci-dessous), rechargement
    automatique des fichiers du serveur, vitesse du processeur, identité et matériel de l'appareil ;
  - **Trace** : registres du processeur (avec leur rôle) et drapeaux, appels système, sortie standard.
- Enregistrer l'état, exporter les fichiers et modifier la mémoire exigent la **pause**.
- Les confirmations et saisies s'ouvrent dans des fenêtres flottantes, déplaçables et redimensionnables, comme la fenêtre
  « Licence et conditions d'utilisation ».

## Conserver les fichiers dans le navigateur

À la première visite, le site demande s'il peut conserver les fichiers de la calculatrice dans ce navigateur ; la réponse se
change à tout moment dans l'onglet Réglages. Si c'est accepté, les fichiers sont copiés (au format
[`nfs`](./format/nfs.md)) dans les données du site à chaque modification, et remis à chaque lancement ou relance ; le
navigateur peut demander l'autorisation de les garder durablement. Ils sont partagés par tout ce qui tourne sur ce site, dans
ce navigateur : le bouton « Effacer les fichiers conservés » les supprime.

## Performances

L'émulation tourne dans un Web Worker, à côté de la page : une application qui calcule sans attendre occupe un cœur du
processeur sans jamais figer l'interface. Le Worker dessine l'écran lui-même (`OffscreenCanvas`) et utilise toute l'image
de 16 ms pour émuler ; la page lui envoie les touches et les requêtes, et reçoit les changements d'état. Il faut donc un
navigateur qui sait dessiner un `OffscreenCanvas` dans un Worker et le SIMD WebAssembly (Chrome 91, Firefox 105, Edge 91,
Safari 16.4 ou plus récent).

## Accessibilité

Toutes les actions se font au clavier (Tab, flèches dans les listes et les onglets, Entrée ou Espace sur une touche de la
calculatrice qui a le focus), les zones sont étiquetées pour les lecteurs d'écran, les couleurs suivent le thème clair ou
sombre du système (réglable en haut à droite), et les animations respectent « réduire les animations ».
