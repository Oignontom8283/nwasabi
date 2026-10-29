---
title: "Clavier et événements"
description: "L'état des touches, la numérotation des événements par plan shift et alpha, la répétition d'appui long et la table des textes d'événements."
---

# Clavier et événements

Deux vues du clavier coexistent, comme dans l'EADK : l'**état** (`eadk_keyboard_scan`, `pop_state`) et les **événements** (`eadk_event_get`).

## L'état

`Keyboard::state()` est un `u64` : le bit `Key::bit()` d'une touche vaut 1 quand elle est enfoncée. Les 46 touches (`KEY_NAMES`) ont les numéros de l'API EADK publique (`eadk_key_*`), de 0, dans cet ordre : `left`, `up`, `down`, `right`, `ok`, `back`, `home`, `on_off`, `shift`, `alpha`, `xnt`, `var`, `toolbox`, `backspace`, `exp`, `ln`, `log`, `imaginary`, `comma`, `power`, `sine`,
`cosine`, `tangent`, `pi`, `sqrt`, `square`, `seven`, `eight`, `nine`, `left_parenthesis`, `right_parenthesis`, `four`, `five`, `six`, `multiplication`, `division`, `one`, `two`, `three`, `plus`, `minus`, `zero`, `dot`, `ee`, `ans`, `exe`.
`set_state` remplace l'état sans produire d'événement ; `press` et `release` font les deux.

## Les événements

Un événement est un numéro. Le clavier a 9 rangées de 6 touches = **54** événements par « plan » (`EVENTS_PER_PLANE`) ; le plan dépend de `shift` et d'`alpha` :

```text
événement = numéro_de_touche + 54 × (shift + 2 × alpha)       plans 0 à 3, donc 0 à 215
EVENT_NONE = 4 × 54 = 216                                      « rien » (délai écoulé)
```

`press(key)` met à jour les modificateurs **avant** de produire l'événement :

- `shift` bascule ; `alpha` bascule ; **`shift` suivi d'`alpha`** verrouille `alpha` (`alpha_lock`) et relâche `shift` ;
- `shift` et `alpha` produisent leur propre numéro de touche (plan 0) ;
- toute autre touche utilise le plan courant, puis relâche `shift` et, si `alpha` n'est pas verrouillé, `alpha` : un modificateur ne vaut que pour **une** touche ;
- la touche `Home`, avec `HomeBehavior::ExitApp` (le défaut, comme sur l'appareil), ne produit pas d'événement : elle pose `exit_requested`, et le noyau quitte l'application au prochain appel système.
  Avec `HomeBehavior::Deliver` (comme avec le contournement DFU), l'application la reçoit comme les autres.

La file d'événements garde au plus **64** événements en attente (`MAX_PENDING_EVENTS`) : le clavier réel n'en garde qu'un nombre limité. `shiftAlphaStatus` (appels 26 et 28) lit et impose les modificateurs :
`1` shift, `2` alpha, `4` alpha verrouillé, 3 et 5 les combinaisons avec shift.

## La répétition

Les événements des **flèches**, de `back`, de `backspace`, et des flèches avec `shift` se répètent tant que la touche reste enfoncée (`is_repeatable`, relevé dans le noyau d'Epsilon 26.4.0). Le premier délai est de
**299 ms**, les suivants de **50 ms** (`REPEAT_FIRST_MS`, `REPEAT_NEXT_MS`). Une nouvelle touche arrête la répétition de la précédente ; relâcher la touche répétée aussi. `Kernel::next_event(now)` rend le plus
ancien événement en file, sinon la répétition si son délai est écoulé, et compte les répétitions dans `repeat_count` (`long_press_counter`, appel 55).

## Texte des événements

`eadk_event_copy_text` (appel 22) rend le texte qu'insère un événement (`sin(`, `×`, les chiffres, les lettres…). `event_text.rs` est une table de **216** entrées indexée par le numéro d'événement ; `None` : événement non défini, `Some("")` :
défini sans texte ; le caractère `U+0011` marque la position du curseur après l'insertion (`sin(` puis `)`). Les valeurs sont celles de l'Epsilon 26.4.0, identiques sur les trois modèles. `is_defined` (appel 24) dit si un événement
existe : toutes les entrées de la table, plus les événements spéciaux.

## Gestes et fichiers de mapping

Le noyau ne connaît que les **touches de la calculatrice**. Le lien entre le clavier de l'ordinateur ou les gestes tactiles et ces touches est dans `keymap/` (module de règles) et dans `@nwasabi/js` (`input.ts`,
`gestures.ts` : reconnaissance des gestes au plus près des événements du navigateur). Voir le format [`.nmk`](../../reference/formats/nmk.md).
