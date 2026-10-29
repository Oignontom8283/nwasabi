---
title: "La session"
description: "Le protocole JSON de Nwasabi vu de l'intérieur : cycle d'une requête, découpage de run en tranches, classement des commandes, débogage du code source, pas à la ligne et budgets."
---

# La session

`Session` (`src/session/`) est **le** point d'entrée : un appareil émulé (`Emulator`), un historique facultatif et les réglages de la conversation. Une requête est un objet JSON `{"cmd": "…", …}` avec un `"id"` facultatif renvoyé tel quel ; la réponse est
`{"ok": true, …}` ou `{"ok": false, "error": "…"}`. C'est le même contrat pour `nwasabi-cli session`, le serveur, la page web, `@nwasabi/js` et le serveur GDB, qui n'ont qu'à transporter ces objets. Les commandes elles-mêmes sont décrites dans la
[référence du protocole](../../reference/protocole-session.md) ; cette page dit comment la session les traite.

La session ne touche **ni au disque ni à l'horloge** : les fichiers entrent et sortent en base64 (en hexadécimal pour la mémoire), le temps est donné par `advance`. Les objets JSON sont écrits avec leurs clés **triées** (`json.rs`, sur `serde_json`) : la
sortie est déterministe, les résultats se comparent tels quels.

## Cycle d'une requête

```rust
fn handle(&mut self, request: &Value) -> Value {
    let result = self.dispatch(request);                         // « cmd » → méthode
    if result.is_ok() && self.history.is_some() {
        match history::effect_of(cmd) {                          // voir « Remonter le temps »
            Effect::Input if !tap => self.history_record(request),
            Effect::Reset | Effect::Input => self.history_reset(),
            Effect::None | Effect::Forward => {}
        }
    }
    // { "ok": true, … } ou { "ok": false, "error": … }, avec l'id de la requête
}
```

`Reply = Result<Value, String>` : les erreurs sont des **messages en clair**, en français, avec la raison (« aucun appareil : utiliser « new » ou « load » », « mode inconnu : … (into, over, out) »). La commande `help` liste toutes les commandes ; les
commandes du débogage du code source sont dans `source.rs::COMMANDS`.

## `run`

`run` ne confie jamais tout le budget à l'émulateur d'un coup :

- budget par défaut **10 000 000** instructions (`DEFAULT_INSTRUCTIONS`), `instructions` le change ; `frames` s'arrête après autant d'images ;
- tranches de **1 000 000** (`CHUNK`), **plus courtes si un historique tourne** (`history_slice()`), pour ne pas manquer l'heure d'un point de reprise ; après chaque tranche, `history_tick` prend un point de reprise s'il est dû ;
- `Frame` incrémente le compteur d'images, `Sleep` et `Budget` continuent (sauf **avec l'horloge hôte**, où une attente ne finit que si l'hôte fait passer du temps : on lui rend la main), tout autre arrêt termine ;
- avant d'exécuter vers l'avant, `history_fork` coupe un éventuel futur enregistré (voir [Historique](./historique.md)).

La réponse porte `stop` (description de l'arrêt, avec l'adresse de l'instruction fautive pour une faute), `frames`, `instructions` et `time_ms`.

`key` avec `action: "tap"` exécute **2 000 000 instructions** (`instructions` le change) entre l'appui et le relâchement : c'est un appui d'une durée qui laisse l'application réagir ; `press` et `release` sont des événements isolés, enregistrés pour le rejeu.

## Le débogage du code source : `source.rs`

Ces commandes s'appuient sur `DebugInfo` (`Session::debug_of` rend l'émulateur et le `DebugInfo` partagé, `Arc`) :

| Commande | Rôle |
|---|---|
| `debug_info` | fonctions, fichiers, présence d'informations ; |
| `line_at`, `source_lines` | adresse → ligne ; texte des fichiers (donné par l'hôte ou lu par le serveur : voir ci-dessous) |
| `break_line`, `unbreak_line`, `break_function` | points d'arrêt par ligne (`fichier:ligne`, la ligne sans code glisse à la suivante qui en a) et par fonction (**après le prologue** : `DebugInfo::after_prologue`) |
| `frames` | la pile : fonction, fichier, ligne, adresse de retour, paramètres |
| `locals`, `globals` | variables visibles dans le cadre choisi, valeurs dépliables |
| `eval`, `assign`, `ptype` | expressions ([Expressions](./expressions.md)), modification, description d'un type |
| `step_source` | `mode` `into`, `over` ou `out` |
| `export_elf` | l'ELF lié, pour GDB |

### Pas à la ligne : `step_source`

La session réalise le pas par des **pas d'instruction** et des **points d'arrêt temporaires**, avec un budget de **20 000 000** instructions (`STEP_BUDGET`) :

- **`out`** : prend le cadre appelant (pile exacte à 2 niveaux), pose un point d'arrêt temporaire à l'**adresse de retour**, et exécute jusqu'à y arriver avec `sp ≥ CFA du cadre quitté` : le test de la pile gère la **récursion** (un point d'arrêt atteint par un appel plus profond n'arrête pas) ;
- **`into` et `over`** : exécute instruction après instruction. Quand l'instruction est un **appel** (`bl`, `blx`, `is_call_at`) : `over` exécute l'appel jusqu'à son retour (adresse `lr`, `sp ≥ sp_avant`) ; `into` entre dans la fonction **si elle a des lignes**
  (`function_at(now)`), jusqu'à la première ligne après le prologue (point d'arrêt temporaire), et sinon l'exécute en entier comme `over` ;
- le pas **s'arrête** sur une instruction qui est un **début d'instruction de source** (`is_stmt` d'une ligne exactement à l'adresse) et soit change de ligne, soit change de fonction, soit change de cadre (CFA différente) : c'est la règle qui distingue un retour dans l'appelant d'une
  boucle sur la même ligne ;
- une attente (`WaitEvent`) s'arrête avec `outcome = "waiting"`, un `Sleep` fait passer le temps virtuel, `outcome = "budget"` si les 20 millions ne suffisent pas.

### Le texte des sources

Le `.nwa` ne contient que les **noms de fichiers** et les numéros de lignes. `source_lines` en demande le texte à l'hôte : la page garde les fichiers donnés par l'utilisateur (choix ou glisser-déposer, rattachés **par la fin de leur nom**) ; avec le serveur, `--src dossier` fait lire
les fichiers au serveur à chaque chargement (`/__nwasabi/source`), qui ne sert que des fichiers de ce dossier choisis par la **fin de leur chemin DWARF** — **jamais un chemin que la requête donnerait**.

## Autres blocs de la session

- **Appareil** : `new`, `load` (avec `nwa`, `data`, `nfd`, `model`, `firmware`, `clock`, `speed`, `mhz`), `launch`, `dump`/`restore` ([`.nds`](../../reference/formats/nds.md)), `speed`, `device_name`, `fcc_id`, `serial_number`, `settings_get`/`settings_set`, `exam_mode`… (via `Settings`) ;
- **Mémoire** : `memory_read` (au plus 1 Mio : `MAX_TRANSFER`), `memory_write`, `changes`, `regions`, `usage`, `watch`/`unwatch`/`watch_list`/`watch_hits` ;
- **Exécution et observation** : `run`, `step`, `break`/`unbreak`/`breaks`, `cpu`, `cpu_set`, `disasm`, `profile`, `backtrace`, `symbol`, `symbols`, `svc_stats`, `svc_log`, `stdout`, `screen`, `time`, `advance` ;
- **Fichiers** : `fs_*` ([Système de fichiers](../noyau/fichiers.md)), `flash_load`, `flash_export`.
