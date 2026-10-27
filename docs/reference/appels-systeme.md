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
