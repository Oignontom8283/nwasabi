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

## Commandes et raccourcis

**Ctrl + K** ouvre la palette de commandes : on tape quelques lettres d'une action (charger, pause, capture, onglets…) et Entrée la
lance.

| Raccourci | Action |
|---|---|
| Ctrl + K | palette de commandes |
| Ctrl + S, Ctrl + O | sauvegarde rapide de l'état dans le navigateur, et chargement |
| Ctrl + R | recharge les fichiers du serveur qui ont changé (sans changement, la page se recharge comme d'habitude) |
| Ctrl + Espace, Ctrl + . | pause ou reprise, avancer d'une image |
| Ctrl + Maj + Espace | relance l'application |
| Ctrl + E, Ctrl + Maj + E, Ctrl + G | capture de l'écran, copie de l'écran, enregistrement GIF |
| Ctrl + Entrée, Ctrl + B, Ctrl + J | plein écran, écran large, ouvrir ou fermer le panneau |
| ? | liste de tous les raccourcis |
| F5, F10, F11, Maj + F11 | reprendre ou pause, pas par-dessus, pas, sortir (onglet [Débogage](./debug/)) |

Dans la barre latérale, le bouton « Image » avance d'une image en pause, « Copier » met l'écran dans le presse-papiers et « GIF »
enregistre l'écran en image animée jusqu'au prochain clic. Le menu du haut garde les dernières applications ouvertes (si la
conservation des fichiers est acceptée). Chaque application peut avoir ses propres touches : dans l'onglet Touches, « Retenir pour
cette application » les associe à son nom. Dans les Réglages, « Effet de rétroéclairage » assombrit l'écran quand la luminosité
baisse.

## Installer l'émulateur

L'émulateur est une application web installable : le navigateur propose de l'installer (icône dans la barre d'adresse), et une fois
visité il fonctionne **hors ligne**. Sur téléphone ou tablette en hauteur, la calculatrice prend toute la largeur et le panneau
d'outils la recouvre.

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
