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

## Les deux slots de la flash

La flash externe est coupée en deux **slots** de 4 Mio : le slot A à `0x9000_0000`, le slot B à `0x9040_0000`. Chacun peut contenir
un firmware, suivi de la zone des applications externes (qui finit à `0x903f_0000` pour le slot A, `0x907f_0000` pour le B). L'appareil
en exécute un seul, le **slot actif** (réglage `active_slot`) : les applications s'installent dans la zone de ce slot-là.

Début de la zone des applications, selon la disposition du firmware :

| Disposition | N0110 | N0115 | N0120 |
|---|---|---|---|
| `24` (appareils relevés) | `0x9057_0000` (slot B actif) | `0x9017_0000` (slot A actif) | `0x9059_0000` (slot B actif) |
| `26.4.0` | `0x9020_0000` (slot A) | `0x9020_0000` (slot A) | `0x9018_0000` (slot A) |

Avec l'autre slot actif, les adresses sont décalées de `0x40_0000`. Un programme placé en flash à part (voir [`.nfd`](../format/nfd.md))
ne peut pas recouvrir le firmware du slot actif ni les applications installées.

## Dans la SRAM

La disposition est celle du système :

- le tampon de stockage (`0xA800` octets, 42 Kio) se trouve à `SRAM + 0x657c` ; c'est là que vivent les fichiers
  ([formats](../format/nfs.md)) ;
- la mémoire des applications va de `SRAM + 0x118a4` à `SRAM + 0x37000` ;
- le haut de la pile du processus est à `SRAM + 0x3eff8`.

La commande de session
`regions` les renvoie. Toute lecture ou écriture hors d'une zone déclarée provoque une faute d'accès
([registres de faute](../debug/index.md#registres-de-faute)).
