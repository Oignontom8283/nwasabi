# tests/golden

Résultats de référence (« golden ») utilisés par les tests : captures d'écran, sommes de
contrôle de la mémoire, sorties JSON. Ils sont produits par la version native de nwasabi,
puis comparés à ceux de la version WebAssembly (voir `tests/wasm/`).

## Fichiers

- `link_images.tsv` : pour chaque application du corpus, la taille et la somme de contrôle (CRC-32) de
  l'image produite par l'éditeur de liens, et ses adresses principales. Utilisé par
  `tests/link_corpus.rs`. Comme le corpus n'est pas distribué avec le dépôt, ce test s'exécute avec
  `just test-corpus <dossier>` ; `just update-golden <dossier>` régénère le fichier après un
  changement voulu.

Ces résultats ont été confrontés à l'éditeur de liens GNU (`arm-none-eabi-ld`) avec le même script de
link : les images sont identiques octet pour octet, à la fusion des chaînes de caractères près
(nwasabi ne fusionne pas les chaînes identiques, ce qui n'a aucun effet sur le fonctionnement).
