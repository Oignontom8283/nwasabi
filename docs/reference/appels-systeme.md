# Les appels système

Une application ne parle au système que par des appels système (`svc`). Voici ceux qui sont gérés. Le
[débogage](../debug/index.md#journal-des-appels-systeme) les journalise avec leurs arguments.

## Appels de l'EADK

Les numéros sont ceux de la bibliothèque officielle.

| N° | Fonction | Rôle |
|---|---|---|
| 1 | `eadk_backlight_brightness` | lit la luminosité |
| 2 | `eadk_backlight_set_brightness` | règle la luminosité |
| 18 | `eadk_display_pull_rect` | lit des pixels de l'écran |
| 19 | `eadk_display_push_rect` | écrit des pixels |
| 20 | `eadk_display_push_rect_uniform` | remplit un rectangle d'une couleur |
| 21 | `eadk_display_wait_for_vblank` | attend la prochaine image |
| 23 | `eadk_event_get` | prochain événement (touche, avec délai) |
| 33 | `eadk_keyboard_pop_state` | état du clavier, vidé |
| 34 | `eadk_keyboard_scan` | état du clavier |
| 45 | `eadk_random` | nombre aléatoire |
| 48 | `eadk_timing_millis` | millisecondes écoulées |
| 49 | `eadk_timing_msleep` | attente en millisecondes |
| 50 | `eadk_timing_usleep` | attente en microsecondes |

Les autres emplacements de la table officielle répondent `0` ou sont ignorés.

## Appels du noyau d'Epsilon utilisés par KhiCAS

Le lanceur de [KhiCAS](../guide/khicas.md) s'en sert, en plus de ceux de l'EADK (que le système officiel ne propose pas à une
application ordinaire).

| N° | Rôle |
|---|---|
| 26 | règle l'état de `shift` et d'`alpha` (r0 : 0 rien, 1 shift, 2 alpha, 3 shift et alpha, 4 alpha verrouillé, 5 shift et alpha verrouillé) |
| 28 | lit cet état (même codage) |
| 30 | efface un secteur de la flash externe (r0 : numéro du secteur ; rend 1 si fait, 0 si refusé) |
| 32 | écrit dans la flash externe (r0 : destination, r1 : source, r2 : longueur ; rend 1 si fait, 0 si refusé) |

Les secteurs sont numérotés comme dans le noyau : 0 à 7 de 4 Kio (`0x90000000` à `0x90008000`), 8 de 32 Kio, puis 9 et suivants de
64 Kio (`0x90010000` à `0x907f0000`). Écrire ne peut que passer des bits à 0 : seul l'effacement d'un secteur les remet à 1, comme
pour la vraie flash. Ces deux appels sont **refusés** (retour 0) sur le firmware et sur les applications installées ; le reste de la
flash externe est ouvert. Les écritures font partie de l'état de l'appareil (un [`.nds`](../format/nds.md) les garde) et remettent à
zéro l'[historique du débogage](../debug/index.md#remonter-le-temps).

## Appels facultatifs

Le système officiel ne les propose pas aux applications ; Nwasabi peut les fournir en option : `eadk_battery_is_charging` (3),
`eadk_battery_level` (4), `eadk_battery_voltage` (5), `eadk_usb_is_plugged` (52).

## Appels propres à Nwasabi

Ils servent à la bibliothèque C embarquée et ne se lient pas depuis l'application.

| N° | Rôle |
|---|---|
| `0x80` | affichage de texte à l'écran |
| `0x81` | `_sbrk` : agrandir le tas |
| `0x82` | `_write` : écrire du texte sur la sortie |
| `0x89` | fin de l'application (retour de `main`) |

## Appel inconnu

Avec le réglage `strict_svc`, un numéro qui n'est pas dans ces tables arrête l'application sur une faute, ce qui aide à repérer
une fonction non gérée.
