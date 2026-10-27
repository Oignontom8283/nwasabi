# Ressources graphiques

| Fichier | Origine | Licence |
| --- | --- | --- |
| `logo/logo.svg`, `logo/logo-bordered.svg` | Créés pour nwasabi | Ceux du dépôt (AGPL-3.0) |
| `icons/floppy.svg` | Créé pour nwasabi | Ceux du dépôt (AGPL-3.0) |
| `icons/github.svg` | Octicons `mark-github` (1) | MIT |
| Tous les autres fichiers de `icons/` (dont `burger`, `home`, `back` et `power`, lus aussi pour dessiner les touches de la calculatrice) | Créés pour nwasabi (dessin en traits, 24 × 24) | Ceux du dépôt (AGPL-3.0) |

1. [Octicons](https://github.com/primer/octicons), de GitHub, sous licence MIT. Le fichier d'origine a seulement reçu l'attribut `fill="currentColor"`, pour prendre la couleur du texte. « GitHub » et son logo sont des marques de GitHub, Inc. : l'icône ne sert qu'à renvoyer vers le dépôt.

Les icônes sont chargées par l'interface web (`web/src/components/Icon.vue`), qui les lit dans `icons/` ; le logo est lu par `Logo.vue` et la page de l'émulateur (icône de l'onglet), les deux logos aussi par la documentation (copiés par `just docs-emulator`).
