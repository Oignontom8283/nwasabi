---
title: "La mémoire émulée"
description: "Le module memory : zones à pages paresseuses, droits d'accès, chemin rapide des accès, suivi des lignes lues, écrites et exécutées, datation des modifications et instantanés."
---

# La mémoire émulée

`Memory` est ce que le processeur voit à travers le trait `cpu::Bus`, et ce que l'hôte inspecte. Elle n'est qu'un **espace d'adressage de 32 bits** découpé en
**zones**, sans aucun périphérique : tout ce qui ressemble à un registre matériel est simulé par du contenu mémoire (voir
[Appareil](./appareil.md)).

## Zones

Une zone (`RegionSpec`) a un nom, une base, une taille, des droits (`Perms { read, write, execute }`), une nature (`RegionKind` : `Flash`, `Ram` ou
`Reserved`) et un octet de remplissage. Les droits usuels sont `READ_EXECUTE` (flash), `READ_WRITE_EXECUTE` (RAM), `READ_ONLY` (registres de faute)
et `NONE` (SRAM du noyau). `add_region` refuse (`MemoryError::BadRegion`) une zone de taille nulle, dont la base ou la taille ne sont pas des multiples de 4 Kio ou qui dépasse 4 Gio, et (`Overlap`) une zone qui en recouvre une autre.

- **Pages paresseuses** : une zone n'alloue de mémoire que pour les pages de 4 Kio (`PAGE_SIZE`) qu'on touche. Une flash de 8 Mio vide ne coûte rien ; ce
  qu'on lit dans une page non allouée vaut l'octet de remplissage (`0xFF` pour la flash, la valeur d'`DeviceOptions::ram_fill` pour la RAM).
  `allocated_pages(zone)` mesure la consommation réelle.
- **Recherche de la zone** : `find(address, len)` essaie d'abord les **deux dernières zones utilisées** (`last`, `previous`) : un programme alterne
  surtout entre sa pile ou son tas (RAM) et ses constantes (flash), et le plus récent est essayé en premier. Sinon, recherche linéaire.
- **Deux sortes d'accès** : ceux **du programme** (par `Bus` : vérifient les droits, marquent les lignes, déclenchent les points d'observation) et ceux **de
  l'hôte** (`read_bytes`, `write_bytes`, `read_u32`, `fill`… : ignorent les droits, ne marquent rien, ne comptent pas comme une activité du programme).
  Seule l'hôte fabrique l'image de l'appareil ou en lit le contenu sans fausser les mesures.

## Chemin d'un accès du programme

```text
read_access<N>(adresse, fetch):
    zone = find(adresse, N)                  sinon BusFault::Unmapped
    droit = fetch ? execute : read           sinon BusFault::AccessDenied
    octets = zone.read_small::<N>(offset)
    marquer les lignes (EXECUTE si fetch, sinon READ)
    si pas fetch et un point d'observation existe : watches.check(...)
write_access<N>(adresse, octets):
    zone = find(...)                         droit write sinon AccessDenied
    tick += 1
    si la ligne est marquée EXECUTE : code_epoch += 1      ← invalide le cache d'instructions
    écrire ; marquer WRITE ; dater la ligne avec tick
    points d'observation
```

`read_block` et `write_block` (utilisés par les appels système : pixels, textes, fichiers) ont un **chemin rapide** : un seul bloc recopié page par page
quand tout le bloc tient dans une zone qui l'autorise **et** qu'aucun point d'observation n'existe ; sinon on retombe sur l'accès octet par octet, qui produit
exactement les mêmes fautes et les mêmes observations.

## Suivi des lignes

Chaque zone garde, pour chaque **ligne de 32 octets** (`LINE_SIZE`), un octet d'indicateurs : `READ` (1), `WRITE` (2), `EXECUTE` (4). Ils servent à :

- la carte d'utilisation de la mémoire (`usage` : tas et pile, plus haute adresse écrite du tas, plus basse de la pile, `access_extent`) ;
- la détection du **code modifié** (`code_epoch`) ;
- l'inspection des octets « touchés » dans l'éditeur de mémoire de la page ;
- les cartes d'accès enregistrées dans un état `.nds` (`access_map`, `set_access_map`).

`clear_access` efface les indicateurs (à chaque relance). Le suivi se fait au grain de la ligne : la mesure de la pile est donc exacte à 32 octets près.

## Datation et changements

Un compteur `tick` augmente à chaque écriture. Chaque ligne écrite garde la date de sa dernière écriture (le vecteur `stamps` n'existe qu'à la première écriture
de la zone). `change_cursor()` rend le compteur actuel, et `changes_since(cursor)` rend les plages `ChangedRange` modifiées depuis, **fusionnées par zone** et triées
par adresse. C'est ainsi que la page ne relit que ce qui a changé. `flash_changed_since` détecte une écriture en flash (fichiers écrits par l'application) : l'historique en
tire que ce qui précède n'est plus rejouable.

## Instantanés

`snapshot_volatile()` copie les pages allouées de **toutes les zones qui ne sont pas de la flash** (`VolatileImage`, `bytes()` en donne le poids) ;
`restore_volatile` les remet et **vide le cache d'instructions** (`code_epoch += 1`), car de la RAM exécutable a pu changer. Les indicateurs d'accès ne sont pas
restaurés. Ces instantanés sont la base des points de reprise de l'[historique](../debogage/historique.md).

## Points d'observation

`Watchpoints` surveille des plages `[start, start+len)` en lecture, écriture ou les deux (`WatchKind`). `check` est appelé par chaque accès du programme quand au
moins un point existe ; un accès qui recouvre la plage même en partie déclenche. Un déclenchement (`WatchHit`) garde l'identifiant, l'adresse, la largeur (1, 2 ou 4),
le sens, la valeur lue ou écrite et l'adresse de l'instruction (`pc`, connue tant qu'un point existe : l'émulateur la pose avant chaque instruction). Les déclenchements
s'accumulent dans une file bornée à **4096** (`MAX_HITS`, les plus anciens perdus et comptés dans `dropped`). Un point avec `stop` fixe un `stop_hit` que l'émulateur
lit après l'instruction : l'exécution s'arrête **juste après** l'accès. Un rejeu de l'historique **suspend** les points sans les retirer.

Dès qu'un point d'observation existe, le chemin rapide des blocs est désactivé et l'émulateur exécute [instruction par instruction](../execution/index.md).

## Accès exclusifs

`allow_exclusive_accesses(false)` (le défaut) fait refuser `ldrex`/`strex` en RAM (`FaultKind::ExclusiveUnsupported`) : sur la calculatrice, la RAM est partageable et sans moniteur
global. La flash accepte les accès exclusifs.
