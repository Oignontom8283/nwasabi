# Exécuter KhiCAS

[KhiCAS](https://www-fourier.univ-grenoble-alpes.fr/~parisse/numworks/khicasnw.html) est le logiciel de calcul formel de Bernard Parisse
(le moteur Giac/Xcas) pour les NumWorks. C'est l'application externe la plus volumineuse qui existe, et elle ne tient pas dans un seul
fichier `.nwa` : Nwasabi l'exécute, à condition de lui donner ses morceaux. Cette page explique comment elle est faite et comment la
lancer.

::: warning Ce que Nwasabi ne fournit pas
KhiCAS est un logiciel distinct, sous sa propre licence (GPL). Nwasabi n'en embarque aucun fichier : on les télécharge depuis la
page de son auteur.
:::

## Comment KhiCAS est installé sur une vraie calculatrice

Le système officiel limite les applications externes à environ 2,5 Mio, et KhiCAS en fait près de 4. Il est donc livré en **deux
parties**, que l'outil d'installation place à des endroits différents de la flash :

| Fichier | Rôle | Adresse en flash |
|---|---|---|
| `khicas.nwa` | un **lanceur** de quelques kilo-octets, installé comme n'importe quelle application externe | après le firmware (zone des applications) |
| `khi110ab.tar` ou `khi120ab.tar` | le programme KhiCAS lui-même (le code), sous forme d'archive tar | `0x90400000` (le slot B) |
| `khi110a` ou `khi120a` | les données en lecture seule de KhiCAS | `0x90260000` |

Le lanceur lit le point d'entrée de KhiCAS à `0x90400200` (juste après l'en-tête de 512 octets de l'archive tar), crée le tas dans la RAM
des applications, puis lui donne la main en lui passant une table de fonctions (écran, clavier, temps, effacement et écriture de la
flash…). Les fichiers `110` conviennent au N0110 et au N0115, les `120` au N0120 : essayer l'autre jeu de fichiers arrête l'application sur une faute d'accès mémoire.

Le slot B étant occupé par KhiCAS, le firmware officiel doit être dans le **slot A**.

## Les lancer dans Nwasabi

Les morceaux à placer en flash se donnent dans un fichier [`.nfd`](../format/nfd.md), qui dit aussi que le firmware doit être dans le
slot A. On le fabrique avec `nwasabi-cli`, à partir des fichiers téléchargés :

```sh
nwasabi-cli flash pack -o khicas.nfd --active-slot a \
  0x90260000=khi110a \
  0x90400000=khi110ab.tar
```

(Pour un N0120 : `khi120a` et `khi120ab.tar`.) `nwasabi-cli flash list khicas.nfd` vérifie le résultat.

Puis, au choix :

```sh
# le serveur : la page s'ouvre avec KhiCAS lancé
nwasabi --nwa khicas.nwa --flash khicas.nfd --model n0110

# ou la ligne de commande, sans navigateur
nwasabi-cli run khicas.nwa --model n0110 --flash khicas.nfd --screenshot ecran.png
```

Dans l'émulateur web, le modèle se choisit dans les réglages ; on glisse `khicas.nwa` puis `khicas.nfd` sur la page (l'application est
relancée avec ses octets en flash), ou on ouvre `…/emulator/?nwa=…&flash=…` avec des adresses accessibles (CORS).

KhiCAS demande d'abord la langue (OK : français), puis affiche un message d'accueil et le choix de l'interpréteur (OK : Xcas) avant
d'arriver à la session de calcul. Les touches `shift` et `alpha` de la calculatrice fonctionnent : KhiCAS gère lui-même leur état.

## Ce que Nwasabi fait pour lui

- Les **octets en flash** de `khicas.nfd` sont lisibles et exécutables à leurs adresses.
- Le **slot actif** est celui que demande le fichier.
- Les **appels système supplémentaires** du lanceur sont gérés : état de `shift` et d'`alpha` (lecture et écriture), effacement d'un
  secteur de flash et écriture dans la flash. D'après son auteur, KhiCAS s'en sert pour garder des scripts et des sessions à
  `0x90200000`, hors du firmware et des applications installées. Voir [Appels système](../reference/appels-systeme.md).

## Limites

- Les fichiers à utiliser sont ceux de la page de l'auteur : une autre version de KhiCAS peut attendre d'autres adresses ou d'autres
  appels système.
