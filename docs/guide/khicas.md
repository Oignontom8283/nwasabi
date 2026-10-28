# Exécuter KhiCAS

[KhiCAS](https://www-fourier.univ-grenoble-alpes.fr/~parisse/numworks/khicasnw.html) est le logiciel de calcul formel de Bernard Parisse
(le moteur Giac/Xcas) pour les NumWorks. À notre connaissance, c'est l'application externe la plus volumineuse qui existe : elle ne
tient pas dans un seul `.nwa`, mais dans un lanceur et des [données auxiliaires en flash](./donnees-flash.md). Nwasabi l'exécute si on lui
donne ses morceaux.

::: warning Ce que Nwasabi ne fournit pas
KhiCAS est un logiciel distinct, sous sa propre licence (GPL). Nwasabi n'en embarque aucun fichier : on les télécharge depuis la
page de son auteur.
:::

## Les fichiers

| Fichier | Rôle | Adresse en flash |
|---|---|---|
| `khicas.nwa` | le lanceur | zone des applications |
| `khi110ab.tar` ou `khi120ab.tar` | le programme KhiCAS (une archive tar : en-tête de 512 octets, puis le code) | `0x90400000` |
| `khi110a` ou `khi120a` | les données en lecture seule de KhiCAS (un fichier binaire, sans extension) | `0x90260000` |

Les fichiers `110` sont pour le N0110 et le N0115, les `120` pour le N0120. Le programme occupant le slot B, le firmware doit être dans le
slot A. Pour le détail du fonctionnement (point d'entrée, tas, appels système du lanceur), voir
[Données auxiliaires en flash](./donnees-flash.md) et le manuel de l'auteur.

## Les lancer

On rassemble les deux fichiers de données dans un `.nfd`, qui demande aussi le slot A :

```sh
nwasabi-cli flash pack -o khicas.nfd --active-slot a \
  0x90260000=khi110a \
  0x90400000=khi110ab.tar
```

Puis, au choix :

```sh
# le serveur : la page s'ouvre avec KhiCAS lancé
nwasabi --nwa khicas.nwa --flash khicas.nfd --model n0110

# la ligne de commande, sans navigateur
nwasabi-cli run khicas.nwa --model n0110 --flash khicas.nfd --screenshot ecran.png
```

Dans l'émulateur web : `…/emulator/?nwa=…&flash=…&model=n0110` (adresses accessibles, CORS), ou glisser `khicas.nwa` puis `khicas.nfd` sur la
page après avoir choisi le modèle.

## Pour aller plus loin

KhiCAS est un exemple de [données auxiliaires en flash](./donnees-flash.md) : cette page explique le principe en général (la carte de la
flash, les slots, ce que l'application peut faire des données) et donne les détails techniques qu'on n'a pas repris ici.
