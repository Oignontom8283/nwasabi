# Modèles et carte mémoire

## Les modèles

| Modèle | Processeur | SRAM (début) | Flash des applications (début) |
|---|---|---|---|
| N0110 | Cortex-M7, 192 MHz | `0x2000_0000` | `0x9020_0000` |
| N0115 | Cortex-M7, 192 MHz | `0x2000_0000` | `0x9020_0000` |
| N0120 | Cortex-M7, 540 MHz | `0x2400_0000` | `0x9018_0000` |

Le modèle se choisit dans les Réglages, avec `--model` en ligne de commande ou par l'option `model` du
[protocole de session](./protocole-session.md). La vitesse peut suivre le modèle, une fréquence choisie, ou être illimitée.

## Les dispositions de firmware

- **`24`** (par défaut) : la disposition mesurée sur des appareils réels (N0110 en 24.11.0, N0115 en 24.3.0, N0120 en 23.2.6).
- **`26.4.0`** : la disposition du système officiel récent. Certaines applications qui devinent le modèle à partir de la mémoire se
  trompent sur N0110 et N0115 avec cette disposition.

## La carte mémoire

| Zone | Début | Taille | Contenu |
|---|---|---|---|
| Flash externe | `0x9000_0000` | 8 Mio | système et applications installées |
| SRAM | début du modèle | `0x40000` (256 Kio) | pile, tas, données de l'application |
| Noyau | dans la SRAM | `0x1000` | structures du système, inaccessibles à l'application |

Dans la SRAM, la disposition est celle du système :

- le tampon de stockage (`0xA800` octets, 42 Kio) se trouve à `SRAM + 0x657c` ; c'est là que vivent les fichiers
  ([formats](../format/nfs.md)) ;
- la mémoire des applications va de `SRAM + 0x118a4` à `SRAM + 0x37000` ;
- le haut de la pile du processus est à `SRAM + 0x3eff8`.

La commande de session
`regions` les renvoie. Toute lecture ou écriture hors d'une zone déclarée provoque une faute d'accès
([registres de faute](../debug/index.md#registres-de-faute)).
