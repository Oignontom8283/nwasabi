# Nwasabi Pixels

La police du texte à l'écran des applications (`draw_string`). Elle est dessinée pour nwasabi :
elle rappelle celle d'Epsilon sans en être une copie.

## Licence

MIT, voir [NwasabiPixels-MIT.txt](./NwasabiPixels-MIT.txt). Elle couvre le script de ce dossier et
les fichiers qu'il produit (`src/kernel/font/data.rs`) : ensemble, ils forment la police.

## Utilisation

```sh
python3 tools/font/generate.py
```

Python 3 suffit, rien n'est à installer. Le script réécrit `src/kernel/font/data.rs`, le fichier
qui contient les glyphes ; le reste du projet n'en dépend pas autrement.

## Modifier un glyphe

Chaque caractère est une petite fonction du script, repérée par `@glyph("a")`. Elle renvoie des
traits (`line`, `dot`, `stem`, `bar`…) posés sur des repères de la taille de police (`m.L`, `m.R`,
`m.X`, `m.K`…, décrits dans `Metrics`), ou un dessin pixel par pixel avec `art(...)`. Il y a deux
tailles : 7×14 pixels et 10×18 pixels. Après un changement, relancer le script, puis lancer les
tests ; les captures de référence du corpus se régénèrent avec `just update-golden <dossier>`.
