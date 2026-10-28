# L'émulateur web

L'émulateur est dans le navigateur, à l'adresse `/emulator/` du site, ou servi par `nwasabi` (le serveur local).

## Charger des fichiers

L'extension d'un fichier fourni à la page décide de ce qui est chargé :

| Extension | Contenu | Format |
|---|---|---|
| `.nwa` | l'application | — |
| `.nmk` | les touches (clavier et gestes) | [`nmk`](../format/nmk.md) |
| `.nfs` | les fichiers de la calculatrice | [`nfs`](../format/nfs.md) |
| `.nfd` | des [données auxiliaires en flash](../reference/donnees-flash.md) ; l'application est relancée | [`nfd`](../format/nfd.md) |
| `.nds` | un état d'émulation à reprendre | [`nds`](../format/nds.md) |

## Paramètres de l'adresse

Tout peut être donné dans l'adresse. Les adresses de fichiers sont absolues ou relatives à la page ; un autre site
doit autoriser leur lecture (CORS).

| Paramètre | Rôle |
|---|---|
| `nwa=` | application à lancer |
| `data=` | données externes de l'application |
| `nmk=` | mapping des touches |
| `nfs=` | fichiers de la calculatrice |
| `flash=` | données auxiliaires en flash de l'application |
| `model=` | modèle émulé : `n0110`, `n0115` ou `n0120` |
| `nds=` | état à reprendre (il contient son application : il remplace `nwa=`) |
| `lang=` | langue de l'interface : `fr` ou `en` (sinon le dernier choix, puis la langue du navigateur) |

Exemple : `…/emulator/?nwa=https://exemple.org/jeu.nwa&nmk=https://exemple.org/jeu.nmk&lang=fr`.

Avec le serveur : `nwasabi --nwa jeu.nwa --nmk jeu.nmk --nfs fichiers.nfs --flash morceaux.nfd --nds etat.nds`.

### Fichiers du serveur qui changent

Le serveur relit ses fichiers (`--nwa`, `--data`, `--nmk`, `--nfs`, `--flash`, `--nds`) à chaque requête et donne leur empreinte sur
`/__nwasabi/version`. La page la compare une fois par seconde : quand un fichier change sur le disque, la page propose de recharger (raccourci **Ctrl + R**). Un réglage (désactivé par défaut) permet de les recharger sans rien demander. Un fichier devenu invalide n'arrête pas le
serveur : la page affiche l'erreur.

## Texte propre à votre installation

Si vous compilez le site vous-même, vous pouvez y ajouter une fenêtre d'informations sans toucher aux
fichiers du dépôt, avec des variables d'environnement lues à la compilation :

| Variable | Rôle |
|---|---|
| `NWASABI_INFO` | le texte, en Markdown simple (gras, italique, liens, listes ; pas de tableaux ni de HTML) |
| `NWASABI_INFO_FILE` | ou le chemin d'un fichier Markdown |
| `NWASABI_INFO_TITLE` | le titre de la fenêtre (par défaut « Informations ») |

Exemple : `NWASABI_INFO_FILE=infos.md cargo build --release --features serve`. Sans texte, la fenêtre n'existe pas.
