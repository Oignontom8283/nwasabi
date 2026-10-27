# Formats de fichiers de Nwasabi

Nwasabi définit quelques formats de fichiers, tous en **JSON** (les contenus binaires y sont encodés en base64),
chacun avec un champ de version obligatoire. Les fichiers invalides sont refusés avec l'endroit de l'erreur.

| Extension | Nom | Contenu |
|---|---|---|
| [`.nmk`](./nmk.md) | Nwasabi Mapping Key | liaisons du clavier de l'ordinateur et des gestes tactiles vers les touches de la calculatrice |
| [`.nfs`](./nfs.md) | Nwasabi File System | les fichiers de la calculatrice (scripts Python, réglages…) |
| [`.nds`](./nds.md) | Nwasabi Dump State | l'état d'une émulation en pause, à reprendre plus tard |

Chaque format peut être chargé par un paramètre de l'adresse de l'émulateur web (`?nmk=`, `?nfs=`, `?nds=`),
une option du serveur `nwasabi` et de l'outil `nwasabi-cli`, comme `?nwa=` pour l'application.
